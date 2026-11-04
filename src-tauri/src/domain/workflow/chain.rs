//! A task that changes several projects of a workspace (ADR 0043). A run still works in exactly one
//! project, with one worktree, one review and one merge (ADR 0033): a task for the frontend and the
//! backend is one run per project the person marked, in order, each started when the one before it
//! completed. The runs talk to each other through this record: each one leaves the next a handoff
//! (what it did, what each later project probably has to do, or that a project needs nothing), and
//! a project the runs before it said needs nothing is skipped.

use serde::{Deserialize, Serialize};

use super::WorkflowExecutionStatus;

/// How many changed files of an earlier run the next one is told about.
pub const MAX_CHAIN_FILES: usize = 40;
/// How much of an earlier run's diff the next one is given.
pub const MAX_CHAIN_DIFF_CHARS: usize = 6_000;
/// How much of an earlier run's last summary the next one is given.
pub const MAX_CHAIN_SUMMARY_CHARS: usize = 1_200;
/// How many of an earlier run's steps the next one is told about.
pub const MAX_CHAIN_STEPS: usize = 12;

/// A project of the chain, as it was named when the chain began.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainProject {
    pub id: String,
    pub name: String,
}

/// What a run said to the run of a later project: whether it needs anything, what was done that
/// matters to it, and what it probably has to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainNote {
    pub project_id: String,
    /// `false` only when the run said so: that project's run is then skipped.
    pub needed: bool,
    #[serde(default)]
    pub done: String,
    #[serde(default)]
    pub todo: Vec<String>,
}

/// One step of an earlier run, as the next run is told it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainStepSummary {
    pub label: String,
    pub summary: String,
}

/// What a run of the chain that came before this one left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainStep {
    pub project_id: String,
    pub execution_id: String,
    pub status: WorkflowExecutionStatus,
    /// The branch its changes are on, when it had a worktree.
    #[serde(default)]
    pub branch: Option<String>,
    /// The files it changed, by Git; at most [`MAX_CHAIN_FILES`].
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub files_changed: u32,
    /// The start of its diff, at most [`MAX_CHAIN_DIFF_CHARS`].
    #[serde(default)]
    pub diff: String,
    /// What its last step said it did, at most [`MAX_CHAIN_SUMMARY_CHARS`].
    #[serde(default)]
    pub summary: String,
    /// What each of its steps said it did, in the order they finished.
    #[serde(default)]
    pub steps: Vec<ChainStepSummary>,
    /// The decisions its steps recorded.
    #[serde(default)]
    pub decisions: Vec<String>,
    /// What it said to the later projects, one note per project at most.
    #[serde(default)]
    pub notes: Vec<ChainNote>,
}

/// A project of the chain whose run was not started: the run before it said it needs nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainSkip {
    pub project_id: String,
    /// The run whose word it was.
    pub by_execution_id: String,
    /// What that run said about it.
    #[serde(default)]
    pub reason: String,
}

/// The runs of one task over several projects. Every run of the chain carries it, with its own
/// `position`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectChain {
    /// The id of the chain's first run, shared by all of them.
    pub id: String,
    /// The projects the person marked, in the order they are worked in. At least two.
    pub projects: Vec<ChainProject>,
    /// Which of them this run works in.
    pub position: usize,
    /// Projects every run of the chain consults and none changes.
    #[serde(default)]
    pub read_project_ids: Vec<String>,
    /// The runs before this one, in order.
    #[serde(default)]
    pub earlier: Vec<ChainStep>,
    /// Projects skipped so far, because the run before said they need nothing.
    #[serde(default)]
    pub skipped: Vec<ChainSkip>,
    /// Set on a completed run when the run of the next project could not be started, so the
    /// chain ends there: the person sees why instead of a chain that silently goes no further.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped: Option<ChainStop>,
    /// The next project runs only once the person applied (or kept) this run's code, so it never
    /// builds on work that is then thrown away. Off: it runs as soon as this run completes.
    #[serde(default)]
    pub wait_for_delivery: bool,
}

/// Why a chain went no further than a completed run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainStop {
    /// The project whose run could not be started.
    pub project_id: String,
    /// The core's error code (`workflow_invalid`, `scope_invalid`…), worded by the UI.
    pub code: String,
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, String>,
}

/// Where a chain goes once a run of it completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advance {
    /// What the next run carries; `None` when no project is left to run.
    pub next: Option<ProjectChain>,
    /// The projects passed over on the way, each because a run said it needs nothing.
    pub skipped: Vec<ChainSkip>,
}

impl ProjectChain {
    pub fn current(&self) -> Option<&ChainProject> {
        self.projects.get(self.position)
    }

    pub fn next(&self) -> Option<&ChainProject> {
        self.projects.get(self.position + 1)
    }

    pub fn later(&self) -> &[ChainProject] {
        self.projects.get(self.position + 1..).unwrap_or_default()
    }

    /// Where the chain goes after this run, given what it left: to the next project that still
    /// needs something. The latest word about a project wins (this run's over the earlier ones');
    /// a project nobody spoke of runs, so a missing handoff never skips anything.
    pub fn advanced(&self, step: ChainStep) -> Advance {
        let mut earlier = self.earlier.clone();
        earlier.push(step);
        let mut skipped = Vec::new();
        let mut position = self.position + 1;
        while let Some(project) = self.projects.get(position) {
            let latest = earlier.iter().rev().find_map(|run| {
                run.notes
                    .iter()
                    .find(|n| n.project_id == project.id)
                    .map(|n| (run, n))
            });
            match latest {
                Some((run, note)) if !note.needed => {
                    skipped.push(ChainSkip {
                        project_id: project.id.clone(),
                        by_execution_id: run.execution_id.clone(),
                        reason: note.done.clone(),
                    });
                    position += 1;
                }
                _ => break,
            }
        }
        let mut all_skipped = self.skipped.clone();
        all_skipped.extend(skipped.iter().cloned());
        let next = (position < self.projects.len()).then(|| Self {
            id: self.id.clone(),
            projects: self.projects.clone(),
            position,
            read_project_ids: self.read_project_ids.clone(),
            earlier,
            skipped: all_skipped,
            stopped: None,
            wait_for_delivery: self.wait_for_delivery,
        });
        Advance { next, skipped }
    }

    /// The notes the earlier runs left for this run's project, oldest first.
    pub fn notes_for_current(&self) -> Vec<(&ChainStep, &ChainNote)> {
        let Some(current) = self.current() else {
            return Vec::new();
        };
        self.earlier
            .iter()
            .filter_map(|run| {
                run.notes
                    .iter()
                    .find(|n| n.project_id == current.id)
                    .map(|n| (run, n))
            })
            .collect()
    }

    /// What a run of the chain consults: the chain's other projects (the earlier ones as they are
    /// in the person's checkout, the later ones before they change) and the chain's own reads.
    pub fn consulted_ids(&self) -> Vec<String> {
        let current = self.current().map(|p| p.id.as_str());
        let mut ids: Vec<String> = self
            .projects
            .iter()
            .filter(|p| Some(p.id.as_str()) != current)
            .map(|p| p.id.clone())
            .collect();
        for id in &self.read_project_ids {
            if !ids.contains(id) && Some(id.as_str()) != current {
                ids.push(id.clone());
            }
        }
        ids
    }

    pub fn name_of<'a>(&'a self, id: &'a str) -> &'a str {
        self.projects
            .iter()
            .find(|p| p.id == id)
            .map_or(id, |p| p.name.as_str())
    }

    /// The project of the chain a step named in its handoff: by id, or by name ignoring case.
    pub fn project_named(&self, named: &str) -> Option<&ChainProject> {
        let named = named.trim();
        self.projects
            .iter()
            .find(|p| p.id == named || p.name.eq_ignore_ascii_case(named))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str, name: &str) -> ChainProject {
        ChainProject {
            id: id.to_owned(),
            name: name.to_owned(),
        }
    }

    fn chain() -> ProjectChain {
        ProjectChain {
            id: "wfx-1".to_owned(),
            projects: vec![
                project("back", "API"),
                project("front", "Web"),
                project("mobile", "App"),
            ],
            position: 0,
            read_project_ids: vec!["docs".to_owned(), "front".to_owned()],
            earlier: Vec::new(),
            skipped: Vec::new(),
            stopped: None,
            wait_for_delivery: false,
        }
    }

    fn step(notes: Vec<ChainNote>) -> ChainStep {
        ChainStep {
            project_id: "back".to_owned(),
            execution_id: "wfx-1".to_owned(),
            status: WorkflowExecutionStatus::Completed,
            branch: Some("atlas/wfx-1".to_owned()),
            files: vec!["src/api.rs".to_owned()],
            files_changed: 1,
            diff: String::new(),
            summary: "Added the endpoint".to_owned(),
            steps: Vec::new(),
            decisions: Vec::new(),
            notes,
        }
    }

    fn note(project: &str, needed: bool) -> ChainNote {
        ChainNote {
            project_id: project.to_owned(),
            needed,
            done: format!("about {project}"),
            todo: Vec::new(),
        }
    }

    #[test]
    fn a_run_consults_the_other_projects_of_the_chain_and_its_reads_once() {
        assert_eq!(chain().consulted_ids(), ["front", "mobile", "docs"]);
    }

    #[test]
    fn advancing_moves_to_the_next_project_and_keeps_what_the_run_left() {
        let advance = chain().advanced(step(vec![note("front", true)]));
        let next = advance.next.unwrap();
        assert_eq!(advance.skipped, Vec::<ChainSkip>::new());
        assert_eq!(next.position, 1);
        assert_eq!(next.current().unwrap().id, "front");
        assert_eq!(next.earlier.len(), 1);
        assert_eq!(next.notes_for_current().len(), 1);
        assert_eq!(next.consulted_ids(), ["back", "mobile", "docs"]);
    }

    #[test]
    fn a_project_the_run_said_needs_nothing_is_skipped_and_a_silent_one_runs() {
        let advance = chain().advanced(step(vec![note("front", false)]));

        let next = advance.next.unwrap();
        assert_eq!(next.current().unwrap().id, "mobile", "front was skipped");
        assert_eq!(advance.skipped.len(), 1);
        assert_eq!(advance.skipped[0].project_id, "front");
        assert_eq!(next.skipped, advance.skipped);
    }

    #[test]
    fn when_every_later_project_needs_nothing_the_chain_ends() {
        let advance = chain().advanced(step(vec![note("front", false), note("mobile", false)]));

        assert!(advance.next.is_none());
        assert_eq!(advance.skipped.len(), 2);
    }

    #[test]
    fn the_latest_word_about_a_project_wins() {
        let second = chain()
            .advanced(step(vec![note("mobile", false), note("front", true)]))
            .next
            .unwrap();
        let mut front = step(vec![note("mobile", true)]);
        front.project_id = "front".to_owned();
        front.execution_id = "wfx-2".to_owned();

        let third = second.advanced(front).next.unwrap();

        assert_eq!(
            third.current().unwrap().id,
            "mobile",
            "the front run asked for it"
        );
    }

    #[test]
    fn a_step_names_a_project_by_id_or_by_name() {
        let chain = chain();
        assert_eq!(chain.project_named("web").unwrap().id, "front");
        assert_eq!(chain.project_named("back").unwrap().id, "back");
        assert!(chain.project_named("nope").is_none());
    }
}
