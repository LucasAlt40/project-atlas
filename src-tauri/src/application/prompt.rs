use crate::domain::agent::Agent;
use crate::domain::personality::PersonalityProfile;
use crate::domain::project::ProjectContext;
use crate::domain::task::Task;

/// Rules Atlas applies to every agent until real permissions exist.
const ATLAS_RULES: &str = "You are running inside Atlas in read-only mode. Do not modify files \
or run commands that change anything. Answer with text only.";

/// For an execution that may edit files: it works in an isolated worktree and its policy allows
/// writing there. Says exactly that, and what is still not allowed.
const ATLAS_RULES_EDITING: &str = "You are running inside Atlas, in an isolated Git worktree of \
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
    pub context: String,
    pub instruction: String,
}

impl Prompt {
    /// The whole prompt as one text, with clearly labelled sections.
    pub fn combined(&self) -> String {
        let harness = self.harness.as_ref().map_or_else(String::new, |text| {
            let title = if self.task_aware {
                "TASK CONTEXT"
            } else {
                "PROJECT HARNESS"
            };
            format!("{title}\n\n{text}\n\n")
        });
        format!(
            "SYSTEM / PERSONALITY\n\n{}\n\n{harness}PROJECT CONTEXT\n\n{}\n\nUSER INSTRUCTION\n\n{}\n",
            self.system, self.context, self.instruction
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
        let rules = if can_edit {
            ATLAS_RULES_EDITING
        } else {
            ATLAS_RULES
        };
        let system = format!(
            "{}\n\n{rules}\n\n{LIVE_NARRATION}\n\n{PLAN_RULE}",
            personality.system_instructions.trim()
        );

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
        if !standing.is_empty() {
            instruction.push_str("Agent instructions:\n");
            instruction.push_str(standing);
            instruction.push_str("\n\n");
        }
        instruction.push_str("Task:\n");
        instruction.push_str(task.description.trim());

        Prompt {
            system,
            harness: harness.map(str::to_owned),
            task_aware,
            context,
            instruction,
        }
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
