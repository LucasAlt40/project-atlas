use std::fmt::Write;

use crate::application::memory::proposal::RULE as MEMORY_RULE;
use crate::domain::agent::Agent;
use crate::domain::optimization::{BriefLayout, PromptBreakdown, SectionKind, TextSize};
use crate::domain::personality::PersonalityProfile;
use crate::domain::project::{ProjectContext, ProjectNote};
use crate::domain::task::Task;

/// Rules Atlas applies to every agent until real permissions exist.
pub(crate) const ATLAS_RULES: &str =
    "You are running inside Atlas in read-only mode. Do not modify files \
or run commands that change anything. Answer with text only.";

/// For an execution that may edit files: it works in an isolated worktree and its policy allows
/// writing there. Says exactly that, and what is still not allowed.
pub(crate) const ATLAS_RULES_EDITING: &str =
    "You are running inside Atlas, in an isolated Git worktree of \
the project (your working directory). You may create and edit files in it with your file tools. \
You cannot run shell commands or use the network, and you must not touch anything outside your \
working directory. What you write stays in the worktree: it is saved there, handed to the next \
steps of your workflow and reviewed by the user before it reaches the project.";

/// As [`ATLAS_RULES_EDITING`], for a runtime whose tools *are* commands (Codex, `OpenCode`,
/// Gemini): it reads and edits files by running them inside its own sandbox, so telling it it has
/// no shell leaves it with nothing to read the project with. What stays forbidden is what the
/// sandbox and the policy forbid.
pub(crate) const ATLAS_RULES_EDITING_COMMANDS: &str =
    "You are running inside Atlas, in an isolated Git worktree of \
the project (your working directory). You may read, create and edit files in it with the tools \
your runtime gives you, which for you include running commands in its sandbox. Use them for the \
project's files only: do not use the network, do not install or download anything, and do not \
touch anything outside your working directory. What you write stays in the worktree: it is saved \
there, handed to the next steps of your workflow and reviewed by the user before it reaches the \
project.";

/// Applied to every agent, whatever its personality (built-in or custom), so the live response
/// shows each step as it happens. It only shapes the real-time response: the final answer is
/// the concluding message, which must stand on its own.
const LIVE_NARRATION: &str = "Narrate your work as you go. Before each step (reading a file, searching, \
running a tool, reaching a conclusion) write one short sentence saying what you are about to do and \
why, and after it say briefly what you found. Keep these updates short. When you are done, write \
the complete final answer as a last message that stands on its own, without relying on the updates. \
Your working directory path can contain spaces: always quote paths in shell commands.";

/// Applied to every agent, implicitly: a plan is a document the person reads and decides on, so
/// it is never only chat. Atlas shows the message in full (and in the notification), and the
/// file is what stays with the project.
const PLAN_RULE: &str = "PLANS ARE MANDATORY DOCUMENTS. Whenever you propose, are asked for, or \
need approval of a plan, a design or an analysis before implementing, you MUST (1) write it in \
full in your message as Markdown (a title, goals, steps, files to change, risks, how it will be \
verified), because Atlas shows that message to the person, highlighted, to read and approve, and \
(2) if you can create files, also save it as `docs/plans/<short-name>.md` in your working \
directory. Never reduce a plan to a one-line summary or ask for approval without it.";

/// The three separate concerns of a prompt. Providers that support a system prompt can
/// send `system` on its own; the others receive [`Prompt::combined`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub system: String,
    /// What the project's Harness says, already rendered by `HarnessContextBuilder`: the part
    /// the task needs (a Task Context) or, as a fallback, all of it. Context only: it sits below
    /// the Atlas rules in `system` and cannot grant anything. `None` if there is none.
    pub harness: Option<String>,
    /// The Harness text was chosen for this task (a Task Context), not the whole project's.
    pub task_aware: bool,
    /// The skills selected for the task, with the notice that frames them (see
    /// `optimization::skills`). Guidance only: it sits below the Atlas rules and cannot grant
    /// anything. `None` if no skill was selected.
    pub skills: Option<String>,
    /// The notes from earlier work recovered for the task (`application::memory`), with the notice
    /// that frames them: hints that grant nothing. `None` when none were recovered (the prompt is
    /// then byte-identical to one built before memories existed).
    pub memories: Option<String>,
    /// The rules that apply to this execution (`application::rules`), with the notice that frames
    /// them: authoritative guidance that grants no permission. `None` when none apply (the prompt
    /// is then byte-identical to one built before rules existed).
    pub rules: Option<String>,
    pub context: String,
    pub instruction: String,
}

impl Prompt {
    fn rules_section(&self) -> String {
        self.rules
            .as_ref()
            .map_or_else(String::new, |text| format!("RULES\n\n{text}\n\n"))
    }

    /// Everything after the system and rules sections, labelled.
    fn rest(&self) -> String {
        let harness = self.harness.as_ref().map_or_else(String::new, |text| {
            let title = if self.task_aware {
                "TASK CONTEXT"
            } else {
                "PROJECT HARNESS"
            };
            format!("{title}\n\n{text}\n\n")
        });
        let skills = self
            .skills
            .as_ref()
            .map_or_else(String::new, |text| format!("SKILLS\n\n{text}\n\n"));
        let memories = self.memories.as_ref().map_or_else(String::new, |text| {
            format!("NOTES FROM EARLIER WORK\n\n{text}\n\n")
        });
        format!(
            "{harness}{skills}{memories}PROJECT CONTEXT\n\n{}\n\nUSER INSTRUCTION\n\n{}\n",
            self.context, self.instruction
        )
    }

    /// The whole prompt as one text, with clearly labelled sections. This is how every runtime
    /// that has no system-prompt channel of its own receives it.
    pub fn combined(&self) -> String {
        format!(
            "SYSTEM / PERSONALITY\n\n{}\n\n{}{}",
            self.system,
            self.rules_section(),
            self.rest()
        )
    }

    /// What goes on a runtime's system-prompt channel, when it has one: Atlas's own instructions
    /// and the rules. Both are authoritative; everything else (project knowledge, skills, the
    /// task) is not, and stays in the body.
    pub fn system_channel_text(&self) -> String {
        format!("{}\n\n{}", self.system, self.rules_section())
            .trim_end()
            .to_owned()
    }

    /// The body that goes with [`Self::system_channel_text`].
    pub fn body_without_system(&self) -> String {
        self.rest()
    }
}

/// How big each part of a prompt is, taken from the very strings the builder joined (so it is
/// exact and nothing is parsed back out of the prompt). Observability only: the prompt does not
/// depend on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptLayout {
    personality: TextSize,
    atlas_rules: TextSize,
    live_narration: TextSize,
    plan_rule: TextSize,
    /// The rule that lets an agent propose notes; empty when proposals are off.
    memory_rule: TextSize,
    project_context: TextSize,
    agent_instructions: TextSize,
    /// The task text, trimmed, as it follows `Task:` in the instruction.
    task: TextSize,
}

impl PromptLayout {
    /// Splits `prompt` (which this layout was taken from, and which `combined` is the text of)
    /// into measured sections. `brief` says how
    /// the task text divides when it is a workflow step's brief; without it the whole task text
    /// is [`SectionKind::Task`].
    pub fn breakdown(
        &self,
        prompt: &Prompt,
        combined: &str,
        brief: Option<&BriefLayout>,
    ) -> PromptBreakdown {
        let whole = TextSize::of(combined);
        let (harness_kind, harness_size) =
            prompt
                .harness
                .as_deref()
                .map_or((SectionKind::Harness, TextSize::default()), |text| {
                    let kind = if prompt.task_aware {
                        SectionKind::TaskContext
                    } else {
                        SectionKind::Harness
                    };
                    (kind, TextSize::of(text))
                });
        let brief = brief.copied().unwrap_or_default();
        // The brief is a part of the task text: take it out of there. A trailing newline the
        // builder trimmed off is not counted twice (`saturating_sub`).
        let task = self
            .task
            .saturating_sub(brief.workflow_context)
            .saturating_sub(brief.handoff)
            .saturating_sub(brief.protocols);
        PromptBreakdown::new(
            whole,
            [
                (SectionKind::Personality, self.personality),
                (SectionKind::AtlasRules, self.atlas_rules),
                (SectionKind::LiveNarration, self.live_narration),
                (SectionKind::PlanRule, self.plan_rule),
                (harness_kind, harness_size),
                (
                    SectionKind::Rules,
                    prompt
                        .rules
                        .as_deref()
                        .map_or_else(TextSize::default, TextSize::of),
                ),
                (
                    SectionKind::Skills,
                    prompt
                        .skills
                        .as_deref()
                        .map_or_else(TextSize::default, TextSize::of),
                ),
                (SectionKind::Memory, {
                    let notes = prompt
                        .memories
                        .as_deref()
                        .map_or_else(TextSize::default, TextSize::of);
                    TextSize {
                        bytes: notes.bytes + self.memory_rule.bytes,
                        chars: notes.chars + self.memory_rule.chars,
                    }
                }),
                (SectionKind::ProjectContext, self.project_context),
                (SectionKind::AgentInstructions, self.agent_instructions),
                (SectionKind::BriefWorkflowContext, brief.workflow_context),
                (SectionKind::BriefHandoff, brief.handoff),
                (SectionKind::BriefProtocols, brief.protocols),
                (SectionKind::Task, task),
            ],
        )
    }
}

/// What the agent is told of the integrations it was given. A model that is not told tends not to
/// reach for a tool nobody mentioned (shown with a real agent that never opened the browser it was
/// granted); it is not told to use them when the task does not need them.
fn integrations_note(names: &[String], unavailable: &[String]) -> String {
    use std::fmt::Write;

    let mut note = String::new();
    if !names.is_empty() {
        note = format!(
            "\nIntegrations (MCP servers) given to you for this task: {}.\nUse their tools when the \
task needs what they offer (opening or testing a page in a browser, reading or changing issues, \
and so on); do not use them otherwise.",
            names.join(", ")
        );
    }
    if !unavailable.is_empty() {
        // Said plainly, so a step that counted on a tool says it is missing instead of inventing
        // what the tool would have returned.
        let _ = write!(
            note,
            "\nIntegrations meant for this task that could not be given to you: {}. You do not \
have them: if you need what they offer, say so in your result instead of guessing it.",
            unavailable.join(", ")
        );
    }
    note
}

/// What a prompt is assembled from, borrowed: the only place a prompt is assembled.
#[allow(clippy::struct_excessive_bools)] // independent facts about one execution
pub struct PromptBuilder<'a> {
    pub personality: &'a PersonalityProfile,
    pub project: &'a ProjectContext,
    pub agent: &'a Agent,
    pub task: &'a Task,
    pub task_aware: bool,
    /// Whether the agent may edit files (see `RuntimeRequest::allow_edits`). The rule the prompt
    /// states is the one the runtime enforces: the prompt never says more than what the tools allow.
    pub can_edit: bool,
    /// The runtime's tools are commands it runs (see [`ATLAS_RULES_EDITING_COMMANDS`]).
    pub tools_are_commands: bool,
    pub harness: Option<&'a str>,
    pub skills: Option<&'a str>,
    /// The recovered memories, already rendered (see `application::memory`). `None`: none.
    pub memories: Option<&'a str>,
    /// The agent may propose notes for the next run (`optimization.memory`): the prompt says how.
    pub memory_proposals: bool,
    pub rules: Option<&'a str>,
    /// The names of the integrations (MCP servers) this step was given, to tell the agent. Empty:
    /// nothing is said.
    pub integrations: &'a [String],
    /// The names of integrations the step was granted and could not be given, with why in short.
    pub unavailable_integrations: &'a [String],
    /// The other projects of the workspace this execution may look at or change. Empty: the
    /// prompt is the single-project prompt, word for word.
    pub other_projects: &'a [ProjectNote],
}

/// What the agent is told about the projects it may reach besides its own: where they are, what
/// each is, and what it may do there. The limits are stated in the prompt and are the ones the
/// execution was given: nothing is claimed that was not granted.
fn other_projects_note(working: &ProjectContext, others: &[ProjectNote]) -> String {
    let mut note = format!(
        "\nYou work in {} (the working project). The workspace has other projects you may \
         reach:",
        working.name
    );
    for other in others {
        let _ = write!(note, "\n- {}", other.name);
        if let Some(role) = &other.role {
            let _ = write!(note, " ({role})");
        }
        let _ = write!(
            note,
            " at {} — read-only: consult it, never change it",
            other.path
        );
        if !other.mounted {
            note.push_str("; not opened for you in this run, so its files are not readable");
        }
        if !other.technologies.is_empty() {
            let _ = write!(note, "; {}", other.technologies.join(", "));
        }
    }
    note.push_str(
        "\nChange files only in the working project. Paths in the project contexts below belong \
         to the project they are listed under. If the task needs a change in another project, \
         say so in your result instead of making it.",
    );
    note
}

impl PromptBuilder<'_> {
    /// The prompt, also telling how big each part is. The prompt is identical whether or not the
    /// layout is used.
    pub fn assemble(&self) -> (Prompt, PromptLayout) {
        let Self {
            personality,
            project,
            agent,
            task,
            task_aware,
            can_edit,
            tools_are_commands,
            harness,
            skills,
            memories,
            memory_proposals,
            rules,
            integrations,
            unavailable_integrations,
            other_projects,
        } = *self;
        let atlas_rules = if can_edit && tools_are_commands {
            ATLAS_RULES_EDITING_COMMANDS
        } else if can_edit {
            ATLAS_RULES_EDITING
        } else {
            ATLAS_RULES
        };
        let personality_text = personality.system_instructions.trim();
        let mut system =
            format!("{personality_text}\n\n{atlas_rules}\n\n{LIVE_NARRATION}\n\n{PLAN_RULE}");
        // Only when the person turned proposals on: otherwise the prompt is the one there was.
        let memory_rule = if memory_proposals {
            system.push_str("\n\n");
            system.push_str(MEMORY_RULE);
            TextSize::of(MEMORY_RULE)
        } else {
            TextSize::default()
        };

        let mut context = format!("Project: {}\nPath: {}", project.name, project.path);
        if !project.technologies.is_empty() {
            context.push_str("\nDetected technologies:");
            for item in &project.technologies {
                context.push_str("\n- ");
                context.push_str(item);
            }
        }

        if !other_projects.is_empty() {
            context.push_str(&other_projects_note(project, other_projects));
        }

        if !integrations.is_empty() || !unavailable_integrations.is_empty() {
            context.push_str(&integrations_note(integrations, unavailable_integrations));
        }

        let mut instruction = String::new();
        let standing = agent.instructions.trim();
        let mut agent_instructions = TextSize::default();
        if !standing.is_empty() {
            let before = instruction.len();
            instruction.push_str("Agent instructions:\n");
            instruction.push_str(standing);
            instruction.push_str("\n\n");
            agent_instructions = TextSize::of(&instruction[before..]);
        }
        instruction.push_str("Task:\n");
        let task_text = task.description.trim();
        instruction.push_str(task_text);

        let layout = PromptLayout {
            personality: TextSize::of(personality_text),
            atlas_rules: TextSize::of(atlas_rules),
            live_narration: TextSize::of(LIVE_NARRATION),
            plan_rule: TextSize::of(PLAN_RULE),
            memory_rule,
            project_context: TextSize::of(&context),
            agent_instructions,
            task: TextSize::of(task_text),
        };
        let prompt = Prompt {
            system,
            harness: harness.map(str::to_owned),
            task_aware,
            skills: skills.map(str::to_owned),
            memories: memories.map(str::to_owned),
            rules: rules.map(str::to_owned),
            context,
            instruction,
        };
        (prompt, layout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::personality::PersonalitySource;
    use crate::domain::task::TaskStatus;

    fn build(instructions: &str) -> Prompt {
        build_with(instructions, &[], &[])
    }

    fn build_with(instructions: &str, integrations: &[String], unavailable: &[String]) -> Prompt {
        build_rules(instructions, integrations, unavailable, false, false)
    }

    fn build_rules(
        instructions: &str,
        integrations: &[String],
        unavailable: &[String],
        can_edit: bool,
        tools_are_commands: bool,
    ) -> Prompt {
        let personality = PersonalityProfile {
            id: "p".to_owned(),
            name: "Architect".to_owned(),
            description: String::new(),
            system_instructions: "You are an architect.".to_owned(),
            behavior: vec![],
            tags: vec![],
            source: PersonalitySource::Builtin,
            suggested_contract: crate::domain::result_contract::ResultContract::default(),
            suggested_permission_profile: "developer".to_owned(),
        };
        let project = ProjectContext {
            name: "Project Atlas".to_owned(),
            path: "/atlas".to_owned(),
            technologies: vec!["Tauri 2".to_owned(), "Rust".to_owned()],
        };
        let agent = Agent {
            id: "a".to_owned(),
            name: "A".to_owned(),
            personality_id: "p".to_owned(),
            runtime_id: "x".to_owned(),
            model_id: "m".to_owned(),
            instructions: instructions.to_owned(),
            permission_profile_id: None,
            worktree_isolation: false,
            result_contract: crate::domain::result_contract::ResultContract::default(),
            created_at: 0,
        };
        let task = Task {
            id: "t".to_owned(),
            description: "Find three improvements".to_owned(),
            agent_id: "a".to_owned(),
            status: TaskStatus::Running,
        };
        PromptBuilder {
            personality: &personality,
            project: &project,
            agent: &agent,
            task: &task,
            task_aware: false,
            can_edit,
            tools_are_commands,
            harness: None,
            skills: None,
            memories: None,
            memory_proposals: false,
            rules: None,
            integrations,
            unavailable_integrations: unavailable,
            other_projects: &[],
        }
        .assemble()
        .0
    }

    #[test]
    fn the_agent_is_told_the_tools_it_has_and_the_integrations_it_does_not() {
        let given = ["jira (tools: getJiraIssue)".to_owned()];
        let missing = ["context7 (not connected)".to_owned()];

        let prompt = build_with("x", &given, &missing);

        assert!(prompt
            .context
            .contains("given to you for this task: jira (tools: getJiraIssue)."));
        assert!(prompt
            .context
            .contains("could not be given to you: context7 (not connected)"));
        assert!(prompt.context.contains("say so in your result"));
        // Nothing to say, nothing said.
        assert!(!build("x").context.contains("Integrations"));
        let only_missing = build_with("x", &[], &missing).context;
        assert!(!only_missing.contains("given to you for this task"));
        assert!(only_missing.contains("could not be given"));
    }

    #[test]
    fn a_runtime_that_works_through_commands_is_not_told_it_has_no_shell() {
        let file_tools = build_rules("x", &[], &[], true, false).system;
        let commands = build_rules("x", &[], &[], true, true).system;
        let read_only = build_rules("x", &[], &[], false, true).system;

        // A runtime with file tools of its own is told it cannot run commands.
        assert!(file_tools.contains("You cannot run shell commands"));
        // One whose tools are commands is told to use them for the project's files, and nothing else.
        assert!(!commands.contains("You cannot run shell commands"));
        assert!(commands.contains("running commands in its sandbox"));
        assert!(commands.contains("do not use the network"));
        assert!(commands.contains("outside your working directory"));
        // A step that may not edit is read-only whatever its runtime.
        assert!(read_only.contains("read-only mode"));
    }

    #[test]
    fn keeps_personality_context_and_instruction_separate() {
        let prompt = build("Be brief.");

        assert!(prompt.system.starts_with("You are an architect."));
        assert!(prompt.system.contains("read-only"));
        assert!(prompt.system.contains("Narrate your work"));
        // Every agent, whatever its personality or instructions, is held to the plan rule.
        assert!(prompt.system.contains("PLANS ARE MANDATORY DOCUMENTS"));
        assert!(prompt.system.contains("docs/plans/<short-name>.md"));
        assert_eq!(
            prompt.context,
            "Project: Project Atlas\nPath: /atlas\nDetected technologies:\n- Tauri 2\n- Rust"
        );
        assert_eq!(
            prompt.instruction,
            "Agent instructions:\nBe brief.\n\nTask:\nFind three improvements"
        );
    }

    #[test]
    fn omits_empty_agent_instructions_and_labels_combined_sections() {
        let prompt = build("  ");

        assert_eq!(prompt.instruction, "Task:\nFind three improvements");
        let combined = prompt.combined();
        let system = combined.find("SYSTEM / PERSONALITY").unwrap();
        let context = combined.find("PROJECT CONTEXT").unwrap();
        let instruction = combined.find("USER INSTRUCTION").unwrap();
        assert!(system < context && context < instruction);
    }

    #[test]
    fn the_harness_sits_between_the_atlas_rules_and_the_project_context() {
        let mut prompt = build("Be brief.");
        prompt.harness = Some("Project: Transport ERP".to_owned());

        let combined = prompt.combined();
        let rules = combined.find("read-only").unwrap();
        let harness = combined.find("PROJECT HARNESS").unwrap();
        let context = combined.find("PROJECT CONTEXT").unwrap();
        let task = combined.find("USER INSTRUCTION").unwrap();
        assert!(rules < harness && harness < context && context < task);
        assert!(combined.contains("Project: Transport ERP"));
        // Without a Harness there is no such section at all.
        assert!(!build("x").combined().contains("PROJECT HARNESS"));
    }

    #[test]
    fn a_task_context_takes_the_place_of_the_harness_under_its_own_title() {
        let mut prompt = build("Be brief.");
        prompt.harness = Some("This context was selected from the project's Harness".to_owned());
        prompt.task_aware = true;

        let combined = prompt.combined();
        let rules = combined.find("read-only").unwrap();
        let task_context = combined.find("TASK CONTEXT").unwrap();
        let task = combined.find("USER INSTRUCTION").unwrap();
        assert!(rules < task_context && task_context < task);
        // The whole Harness is not repeated under another title.
        assert!(!combined.contains("PROJECT HARNESS"));
    }
}
