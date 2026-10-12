//! What an execution that looks at several projects is told of them.
//!
//! Each project keeps its own Harness in its own folder; the workspace has a context of its own
//! that relates them. Nothing is merged: the text an agent receives is the workspace context
//! followed by one clearly labelled fragment per project, each chosen for the task by the same
//! builder a single project uses, and all of them together within the one allocation the Harness
//! section already had. A project that cannot be given its share is named, not silently dropped.
//!
//! With a scope of one project and no workspace context this is exactly the single-project
//! context: the allocation is not divided.

use super::context::{HarnessBudget, HarnessContextBuilder, TaskContextLoad};
use crate::domain::scope::ProjectScope;
use crate::domain::task_context::{ContextMode, ContextRecord};
use crate::domain::workspace::WorkspaceProject;

/// The most of the allocation the workspace context may take.
const MAX_WORKSPACE_CHARS: usize = 1_200;
/// What labelling a section costs, counted against the allocation so the total holds.
const SECTION_OVERHEAD: usize = 220;
/// Below this a project's fragment is too thin to be worth sending: the project is only named.
const MIN_OTHER_CHARS: usize = 700;
/// The working project never gets less than this, whatever else is in scope.
const MIN_WORKING_CHARS: usize = 1_500;

/// The workspace's own context, as it is told.
#[derive(Debug, Clone, Copy)]
pub struct WorkspaceText<'a> {
    pub name: &'a str,
    pub text: &'a str,
}

/// What became of one project's part of the context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FragmentOutcome {
    /// Its Harness was read; `task_aware` is whether the part was chosen for the task.
    Included { task_aware: bool, characters: usize },
    /// The project has no Harness.
    Missing,
    /// `.atlas/` is there but unusable and was ignored.
    Invalid,
    /// There was no room: only its name and folder are told.
    NamedOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFragment {
    pub project_id: String,
    pub name: String,
    pub access: Access,
    pub outcome: FragmentOutcome,
}

/// The part a project plays in an execution, as the prompt names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Working,
    ReadOnly,
}

impl Access {
    pub fn label(self) -> &'static str {
        match self {
            Self::Working => "working project",
            Self::ReadOnly => "read-only",
        }
    }
}

/// What the builder produced for an execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeContext {
    /// `None` when there is nothing to tell: no workspace context and no project has a Harness.
    pub text: Option<String>,
    /// Whether the working project's part was chosen for the task (not the whole Harness).
    pub task_aware: bool,
    /// The working project's record, with the sizes of the others added in.
    pub record: Option<ContextRecord>,
    pub fragments: Vec<ProjectFragment>,
    /// Whether the workspace context was told, and whether it had to be shortened.
    pub workspace_told: bool,
    pub workspace_trimmed: bool,
}

/// How the allocation is divided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shares {
    pub workspace: usize,
    pub working: usize,
    /// How many of the other projects get a fragment, and how much each.
    pub others_included: usize,
    pub per_other: usize,
}

/// Divides `total` characters among the workspace context, the working project and `others`
/// other projects. The working project keeps at least 60% of what is left after the workspace
/// context, so adding projects to a scope never starves the one the agent works in.
pub fn plan(total: usize, workspace_present: bool, others: usize) -> Shares {
    let sections = 1 + others + usize::from(workspace_present);
    let after_labels = total
        .saturating_sub(SECTION_OVERHEAD * sections)
        .max(MIN_WORKING_CHARS.min(total));
    let workspace = if workspace_present {
        MAX_WORKSPACE_CHARS.min(after_labels / 5)
    } else {
        0
    };
    let rest = after_labels - workspace;
    if others == 0 {
        return Shares {
            workspace,
            working: rest,
            others_included: 0,
            per_other: 0,
        };
    }
    let pool = rest * 2 / 5;
    let others_included = others.min(pool / MIN_OTHER_CHARS);
    if others_included == 0 {
        return Shares {
            workspace,
            working: rest,
            others_included: 0,
            per_other: 0,
        };
    }
    let per_other = pool / others_included;
    Shares {
        workspace,
        working: rest - per_other * others_included,
        others_included,
        per_other,
    }
}

/// The context of an execution with this scope, for `task`, within `total` characters.
pub fn build(
    builder: &HarnessContextBuilder,
    scope: &ProjectScope,
    workspace: Option<WorkspaceText<'_>>,
    task: &str,
    total: usize,
) -> ScopeContext {
    let workspace = workspace.filter(|w| !w.text.trim().is_empty());
    let shares = plan(total, workspace.is_some(), scope.others.len());
    let mut sections: Vec<String> = Vec::new();
    let mut fragments = Vec::new();
    let mut workspace_trimmed = false;

    if let Some(workspace) = workspace {
        let (text, trimmed) = clip(workspace.text.trim(), shares.workspace);
        workspace_trimmed = trimmed;
        sections.push(format!(
            "WORKSPACE CONTEXT — {}\nHow the projects of this workspace relate. It names each \
             project's folder; the project sections below hold what is known of each project \
             itself.\n{text}",
            workspace.name
        ));
    }

    // The working project is always told first, then the others in the workspace's order.
    let mut record: Option<ContextRecord> = None;
    let mut task_aware = false;
    let ordered = std::iter::once((&scope.working, Access::Working))
        .chain(scope.others.iter().map(|other| (other, Access::ReadOnly)));
    // Projects past what the allocation can carry are named only.
    let mut told_others = 0;
    for (project, access) in ordered {
        let named_only = access != Access::Working && told_others >= shares.others_included;
        if access != Access::Working && !named_only {
            told_others += 1;
        }
        let (section, outcome) = if named_only {
            (
                Some(named_only_section(project, access)),
                FragmentOutcome::NamedOnly,
            )
        } else {
            let max_chars = if access == Access::Working {
                shares.working
            } else {
                shares.per_other
            };
            fragment(
                builder,
                project,
                access,
                task,
                max_chars,
                &mut record,
                &mut task_aware,
            )
        };
        if let Some(section) = section {
            sections.push(section);
        }
        fragments.push(ProjectFragment {
            project_id: project.id.clone(),
            name: project.name.clone(),
            access,
            outcome,
        });
    }

    let any_harness = fragments
        .iter()
        .any(|f| matches!(f.outcome, FragmentOutcome::Included { .. }));
    let text = (workspace.is_some() || any_harness).then(|| sections.join("\n\n"));
    ScopeContext {
        text,
        task_aware,
        record,
        fragments,
        workspace_told: workspace.is_some(),
        workspace_trimmed,
    }
}

fn fragment(
    builder: &HarnessContextBuilder,
    project: &WorkspaceProject,
    access: Access,
    task: &str,
    max_chars: usize,
    record: &mut Option<ContextRecord>,
    task_aware: &mut bool,
) -> (Option<String>, FragmentOutcome) {
    match builder.build_for_task_within(&project.path, task, HarnessBudget { max_chars }) {
        TaskContextLoad::Ready(context) => {
            let aware = context.mode == ContextMode::TaskAware;
            let this = context.record();
            if access == Access::Working {
                *task_aware = aware;
                *record = Some(this.clone());
            } else if let Some(total) = record.as_mut() {
                total.total_harness_characters += this.total_harness_characters;
                total.selected_context_characters += this.selected_context_characters;
                total.selected_items += this.selected_items;
                total.omitted_items += this.omitted_items;
                total.stale_items += this.stale_items;
            }
            let characters = context.text.chars().count();
            (
                Some(format!(
                    "{}\n{}",
                    section_title(project, access),
                    context.text
                )),
                FragmentOutcome::Included {
                    task_aware: aware,
                    characters,
                },
            )
        }
        TaskContextLoad::Missing => (
            Some(format!(
                "{}\nThis project has no Harness yet: nothing has been analysed about it.",
                section_title(project, access)
            )),
            FragmentOutcome::Missing,
        ),
        TaskContextLoad::Invalid => (
            Some(format!(
                "{}\nThis project's Harness could not be read and was ignored.",
                section_title(project, access)
            )),
            FragmentOutcome::Invalid,
        ),
    }
}

fn section_title(project: &WorkspaceProject, access: Access) -> String {
    let role = project
        .role
        .and_then(|r| serde_json::to_value(r).ok())
        .and_then(|v| v.as_str().map(str::to_owned));
    format!(
        "PROJECT — {} ({}{}) at {}\nPaths cited in this section are relative to that folder.",
        project.name,
        role.map(|r| format!("{r}, ")).unwrap_or_default(),
        access.label(),
        project.path
    )
}

fn named_only_section(project: &WorkspaceProject, access: Access) -> String {
    format!(
        "PROJECT — {} ({}) at {}\nIts Harness is not included in this task's context.",
        project.name,
        access.label(),
        project.path
    )
}

/// At most `max` characters, cut at a line break when there is one near the end, saying so.
fn clip(text: &str, max: usize) -> (String, bool) {
    if text.chars().count() <= max {
        return (text.to_owned(), false);
    }
    let head: String = text.chars().take(max).collect();
    let cut = head
        .rfind('\n')
        .filter(|at| *at > head.len() / 2)
        .map_or(head.as_str(), |at| &head[..at]);
    (format!("{cut}\n… (shortened)"), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_project_and_no_workspace_context_keeps_the_whole_allocation() {
        let shares = plan(6_000, false, 0);

        assert_eq!(shares.working, 6_000 - SECTION_OVERHEAD);
        assert_eq!(shares.others_included, 0);
    }

    #[test]
    fn the_workspace_context_is_capped_and_the_working_project_keeps_most() {
        let shares = plan(6_000, true, 2);

        assert!(shares.workspace <= MAX_WORKSPACE_CHARS);
        assert_eq!(shares.others_included, 2);
        assert!(shares.working > shares.per_other * 2);
        let spent = shares.workspace + shares.working + shares.per_other * shares.others_included;
        assert!(spent + SECTION_OVERHEAD * 4 <= 6_000);
    }

    #[test]
    fn projects_past_what_the_allocation_carries_are_named_only() {
        let shares = plan(6_000, true, 12);

        assert!(shares.others_included > 0);
        assert!(shares.others_included < 12);
        assert!(shares.per_other >= MIN_OTHER_CHARS);
    }

    #[test]
    fn a_tiny_allocation_goes_to_the_working_project_alone() {
        let shares = plan(2_000, false, 3);

        assert_eq!(shares.others_included, 0);
        assert!(shares.working >= MIN_WORKING_CHARS.min(2_000));
    }

    #[test]
    fn a_long_text_is_cut_on_a_line_and_says_so() {
        let text = "line one\nline two\nline three is much longer than the others";

        let (cut, trimmed) = clip(text, 25);

        assert!(trimmed);
        assert!(cut.ends_with("(shortened)"));
        assert!(!cut.contains("three"));
        assert_eq!(clip("short", 25), ("short".to_owned(), false));
    }
}
