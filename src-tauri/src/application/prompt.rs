use crate::domain::agent::Agent;
use crate::domain::optimization::{BriefLayout, PromptBreakdown, SectionKind, TextSize};
use crate::domain::personality::PersonalityProfile;
use crate::domain::project::ProjectContext;
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
        format!(
            "{harness}{skills}PROJECT CONTEXT\n\n{}\n\nUSER INSTRUCTION\n\n{}\n",
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

/// The only place a prompt is assembled.
pub struct PromptBuilder;

impl PromptBuilder {
    #[cfg(test)]
    pub fn build(
        personality: &PersonalityProfile,
        project: &ProjectContext,
        harness: Option<&str>,
        task_aware: bool,
        agent: &Agent,
        task: &Task,
    ) -> Prompt {
        Self::build_with_access(
            personality,
            project,
            harness,
            task_aware,
            agent,
            task,
            false,
        )
    }

    /// As [`Self::build`], telling the agent whether it may edit files (see
    /// `RuntimeRequest::allow_edits`). The rule the prompt states is the one the runtime enforces:
    /// the prompt never says more than what the tools allow.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub fn build_with_access(
        personality: &PersonalityProfile,
        project: &ProjectContext,
        harness: Option<&str>,
        task_aware: bool,
        agent: &Agent,
        task: &Task,
        can_edit: bool,
    ) -> Prompt {
        Self::assemble(
            personality,
            project,
            harness,
            task_aware,
            agent,
            task,
            can_edit,
            None,
            None,
        )
        .0
    }

    /// [`Self::build_with_access`], also telling how big each part is. This is the one assembly:
    /// the prompt is identical whether or not the layout is used.
    #[allow(clippy::too_many_arguments)]
    pub fn assemble(
        personality: &PersonalityProfile,
        project: &ProjectContext,
        harness: Option<&str>,
        task_aware: bool,
        agent: &Agent,
        task: &Task,
        can_edit: bool,
        skills: Option<&str>,
        rules: Option<&str>,
    ) -> (Prompt, PromptLayout) {
        let atlas_rules = if can_edit {
            ATLAS_RULES_EDITING
        } else {
            ATLAS_RULES
        };
        let personality_text = personality.system_instructions.trim();
        let system =
            format!("{personality_text}\n\n{atlas_rules}\n\n{LIVE_NARRATION}\n\n{PLAN_RULE}");

        let mut context = format!("Project: {}\nPath: {}", project.name, project.path);
        if !project.technologies.is_empty() {
            context.push_str("\nDetected technologies:");
            for item in &project.technologies {
                context.push_str("\n- ");
                context.push_str(item);
            }
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
            project_context: TextSize::of(&context),
            agent_instructions,
            task: TextSize::of(task_text),
        };
        let prompt = Prompt {
            system,
            harness: harness.map(str::to_owned),
            task_aware,
            skills: skills.map(str::to_owned),
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
        PromptBuilder::build(
            &PersonalityProfile {
                id: "p".to_owned(),
                name: "Architect".to_owned(),
                description: String::new(),
                system_instructions: "You are an architect.".to_owned(),
                behavior: vec![],
                tags: vec![],
                source: PersonalitySource::Builtin,
                suggested_contract: crate::domain::result_contract::ResultContract::default(),
                suggested_permission_profile: "developer".to_owned(),
            },
            &ProjectContext {
                name: "Project Atlas".to_owned(),
                path: "/atlas".to_owned(),
                technologies: vec!["Tauri 2".to_owned(), "Rust".to_owned()],
            },
            None,
            false,
            &Agent {
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
            },
            &Task {
                id: "t".to_owned(),
                description: "Find three improvements".to_owned(),
                agent_id: "a".to_owned(),
                status: TaskStatus::Running,
            },
        )
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
