//! Deterministic workflow templates and the selector that picks one for a task. No model is
//! involved: the same task text always gives the same template. A future planner would produce a
//! [`Workflow`] the same way these do, and the engine would run it unchanged.

use serde::Serialize;

use crate::domain::workflow::{
    AgentNode, Condition, EndNode, EndOutcome, ExecutionPolicy, FailurePolicy, LoopPolicy,
    NodeKind, RetryPolicy, Workflow, WorkflowEdge, WorkflowMode, WorkflowNode, WorkflowStatus,
};

/// The kinds of work Automatic mode chooses between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateCategory {
    SoftwareFeature,
    BugFix,
    Refactoring,
    CodeReview,
}

/// What a step is for. Each role is looked up among the agents by personality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Architect,
    Developer,
    Validator,
    Qa,
    BugFixer,
}

impl Role {
    pub fn personality_id(self) -> &'static str {
        match self {
            Self::Architect => "architect",
            Self::Developer => "developer",
            Self::Validator => "architecture-validator",
            Self::Qa => "qa",
            Self::BugFixer => "bug-fixer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    pub id: &'static str,
    pub category: TemplateCategory,
    /// The template Automatic mode proposes for its category.
    pub recommended: bool,
    pub name: &'static str,
    pub description: &'static str,
    pub roles: Vec<Role>,
}

/// An agent a template may use.
#[derive(Debug, Clone)]
pub struct AgentChoice {
    pub id: String,
    pub personality_id: String,
    /// Placed in the workspace: preferred over agents that are only in the catalog.
    pub placed: bool,
    /// The outcomes its result contract declares (none for a general agent).
    pub outcomes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instantiated {
    pub workflow: Workflow,
    /// Roles no agent could be found for: their nodes have no agent yet and the user must pick
    /// one. Atlas never creates an agent on its own.
    pub missing_roles: Vec<Role>,
}

const IDS: [(&str, TemplateCategory, bool, &str, &str); 8] = [
    (
        "software_feature",
        TemplateCategory::SoftwareFeature,
        true,
        "Software Feature Development",
        "Architect, Developer, a validator and QA, each with a Bug Fixer loop to repair what they find.",
    ),
    (
        "feature_basic",
        TemplateCategory::SoftwareFeature,
        false,
        "Software Feature",
        "Architect, then Developer, then QA.",
    ),
    (
        "feature_with_validation",
        TemplateCategory::SoftwareFeature,
        false,
        "Feature with Validation",
        "Architect, Developer, Architecture & Code Validator, then QA.",
    ),
    (
        "feature_with_bug_fix_loop",
        TemplateCategory::SoftwareFeature,
        false,
        "Feature with Bug Fix Loop",
        "Architect, Developer and QA; when QA fails a Bug Fixer repairs and QA runs again.",
    ),
    (
        "full_development",
        TemplateCategory::SoftwareFeature,
        false,
        "Full Development",
        "Architect, Backend, Frontend, validator and QA with both Bug Fixer loops.",
    ),
    (
        "bug_fix",
        TemplateCategory::BugFix,
        true,
        "Bug Fix",
        "A Bug Fixer finds the cause and fixes it; QA checks, and sends it back if it still fails.",
    ),
    (
        "refactoring",
        TemplateCategory::Refactoring,
        true,
        "Refactoring",
        "Architect plans, Developer refactors, the validator checks the architecture, QA checks behaviour.",
    ),
    (
        "code_review",
        TemplateCategory::CodeReview,
        true,
        "Code Review",
        "The Architecture & Code Validator reviews the code, read-only.",
    ),
];

fn roles_of(id: &str) -> Vec<Role> {
    use Role::{Architect, BugFixer, Developer, Qa, Validator};
    match id {
        "feature_basic" => vec![Architect, Developer, Qa],
        "feature_with_validation" => vec![Architect, Developer, Validator, Qa],
        "feature_with_bug_fix_loop" => vec![Architect, Developer, Qa, BugFixer],
        "software_feature" | "full_development" | "refactoring" => {
            vec![Architect, Developer, Validator, BugFixer, Qa]
        }
        "bug_fix" => vec![BugFixer, Qa],
        "code_review" => vec![Validator],
        _ => vec![],
    }
}

pub fn list() -> Vec<TemplateInfo> {
    IDS.iter()
        .map(
            |(id, category, recommended, name, description)| TemplateInfo {
                id,
                category: *category,
                recommended: *recommended,
                name,
                description,
                roles: roles_of(id),
            },
        )
        .collect()
}

pub fn find(id: &str) -> Option<TemplateInfo> {
    list().into_iter().find(|t| t.id == id)
}

/// The template Automatic mode proposes for a task: by what the task says it is, with the
/// feature template as the fallback.
pub fn select_for_task(task: &str) -> &'static str {
    let task = task.to_lowercase();
    let mentions = |words: &[&str]| words.iter().any(|w| task.contains(w));
    let category = if mentions(&[
        "refactor",
        "refator",
        "reorganiz",
        "cleanup",
        "clean up",
        "limpar",
    ]) {
        TemplateCategory::Refactoring
    } else if mentions(&["review", "revis", "audit", "auditar"]) {
        TemplateCategory::CodeReview
    } else if mentions(&[
        "bug",
        "fix",
        "error",
        "erro",
        "falha",
        "corrig",
        "correç",
        "correc",
        "crash",
        "broken",
        "quebrad",
        "exception",
        "regress",
    ]) {
        TemplateCategory::BugFix
    } else {
        TemplateCategory::SoftwareFeature
    };
    IDS.iter()
        .find(|(_, c, recommended, ..)| *c == category && *recommended)
        .map_or("software_feature", |(id, ..)| id)
}

// ---- building -----------------------------------------------------------------------------------

struct Builder<'a> {
    agents: &'a [AgentChoice],
    nodes: Vec<WorkflowNode>,
    edges: Vec<WorkflowEdge>,
    missing: Vec<Role>,
}

impl<'a> Builder<'a> {
    fn new(agents: &'a [AgentChoice]) -> Self {
        Self {
            agents,
            nodes: Vec::new(),
            edges: Vec::new(),
            missing: Vec::new(),
        }
    }

    fn resolve(&mut self, role: Role) -> String {
        let personality = role.personality_id();
        let found = self
            .agents
            .iter()
            .filter(|a| a.personality_id == personality)
            .find(|a| a.placed)
            .or_else(|| self.agents.iter().find(|a| a.personality_id == personality));
        if let Some(agent) = found {
            return agent.id.clone();
        }
        if !self.missing.contains(&role) {
            self.missing.push(role);
        }
        String::new()
    }

    fn step(&mut self, id: &str, label: &str, role: Role, instructions: &str) -> &mut WorkflowNode {
        let agent_id = self.resolve(role);
        self.nodes.push(WorkflowNode {
            id: id.to_owned(),
            kind: NodeKind::Agent(AgentNode {
                agent_id,
                label: label.to_owned(),
                instructions: instructions.to_owned(),
                retry_policy: RetryPolicy::default(),
                failure_policy: FailurePolicy::StopWorkflow,
                execution_policy: ExecutionPolicy::default(),
            }),
            priority: 0,
            loop_policy: None,
            position: None,
        });
        self.nodes.last_mut().expect("just pushed")
    }

    fn looped(&mut self, id: &str, label: &str, role: Role, instructions: &str, loop_id: &str) {
        self.step(id, label, role, instructions).loop_policy = Some(LoopPolicy {
            loop_id: loop_id.to_owned(),
            max_iterations: 3,
        });
    }

    fn end(&mut self, id: &str, label: &str) {
        self.nodes.push(WorkflowNode {
            id: id.to_owned(),
            kind: NodeKind::End(EndNode {
                label: label.to_owned(),
                outcome: EndOutcome::Done,
            }),
            priority: 0,
            loop_policy: None,
            position: None,
        });
    }

    fn then(&mut self, from: &str, to: &str) {
        self.edges.push(WorkflowEdge {
            id: format!("{from}->{to}"),
            source_node_id: from.to_owned(),
            target_node_id: to.to_owned(),
            condition: None,
            label: String::new(),
        });
    }

    /// The two ways out of a step that judges: on to `ok_to`, or to `problem_to` to be corrected.
    /// Which outcomes those are comes from the contract of the agent the step got: the pair of
    /// the `validation` preset (`pass`/`fail`) or of the `review` preset (`approved`/
    /// `changes_requested`), whichever it declares in full. These pairs are Atlas's own presets,
    /// not a guess about an agent's wording; an agent declaring neither gets `pass`/`fail`, and
    /// validation then says what it lacks, since no pair can be chosen for it.
    fn gate(&mut self, from: &str, ok_to: &str, problem_to: &str) {
        let declared: Vec<String> = self
            .nodes
            .iter()
            .find(|n| n.id == from)
            .and_then(|n| n.agent_id())
            .and_then(|id| self.agents.iter().find(|a| a.id == id))
            .map(|a| a.outcomes.clone())
            .unwrap_or_default();
        let has = |o: &str| declared.iter().any(|d| d.eq_ignore_ascii_case(o));
        let (ok, problem) = if has("pass") && has("fail") {
            ("pass", "fail")
        } else if has("approved") && has("changes_requested") {
            ("approved", "changes_requested")
        } else {
            ("pass", "fail")
        };
        self.on(from, ok_to, ok);
        self.on(from, problem_to, problem);
    }

    /// An edge taken when the source step ended with this *outcome*, one its agent's result
    /// contract declares. Routing never reads the execution's status or `result.status`.
    fn on(&mut self, from: &str, to: &str, outcome: &str) {
        self.edges.push(WorkflowEdge {
            id: format!("{from}->{to}:{outcome}"),
            source_node_id: from.to_owned(),
            target_node_id: to.to_owned(),
            condition: Some(Condition::equals("result.outcome", outcome)),
            label: outcome.to_owned(),
        });
    }
}

const ARCHITECT: &str = "Decide what must be built and how, within the project's architecture. \
Record the decisions others must follow and describe the contracts. Do not implement.";
const DEVELOPER: &str =
    "Implement the task following the architecture and decisions you were given. \
Keep the change focused and add tests for the behaviour you add.";
const BACKEND: &str =
    "Implement only the backend part of the task (APIs, services, data) described \
by the architecture. Do not change frontend code.";
const FRONTEND: &str =
    "Implement only the frontend part of the task (screens, state, calls to the \
backend contract) described by the architecture. Do not change backend code.";
const VALIDATOR: &str = "Check, read-only, that the implementation follows the architecture, the \
decisions, the project's conventions and its Harness. Report status pass, fail or warning with \
structured findings. Do not modify code.";
const QA: &str = "Check that the implementation works: run or review the tests, look for bugs and \
edge cases. Report status pass or fail, with the evidence for a failure.";
const FIXER: &str =
    "Investigate the failure reported in the reports you were given, find the root \
cause and fix only what is needed, adding or adjusting tests. Do not weaken validations to make \
tests pass. List the files you changed and any residual risk.";

/// Builds the workflow of a template for a workspace. `None` for an unknown template.
#[allow(clippy::too_many_lines)]
pub fn instantiate(
    template_id: &str,
    workspace_id: &str,
    workflow_id: &str,
    name: Option<&str>,
    agents: &[AgentChoice],
    mode: WorkflowMode,
    now: u64,
) -> Option<Instantiated> {
    let info = find(template_id)?;
    let mut b = Builder::new(agents);
    match template_id {
        "feature_basic" => {
            b.step("architect", "Architect", Role::Architect, ARCHITECT);
            b.step("developer", "Developer", Role::Developer, DEVELOPER);
            b.step("qa", "QA", Role::Qa, QA);
            b.end("done", "Done");
            for (a, c) in [
                ("architect", "developer"),
                ("developer", "qa"),
                ("qa", "done"),
            ] {
                b.then(a, c);
            }
        }
        "feature_with_validation" => {
            b.step("architect", "Architect", Role::Architect, ARCHITECT);
            b.step("developer", "Developer", Role::Developer, DEVELOPER);
            b.step(
                "validator",
                "Architecture & Code Validator",
                Role::Validator,
                VALIDATOR,
            );
            b.step("qa", "QA", Role::Qa, QA);
            b.end("done", "Done");
            for (a, c) in [
                ("architect", "developer"),
                ("developer", "validator"),
                ("validator", "qa"),
                ("qa", "done"),
            ] {
                b.then(a, c);
            }
        }
        "feature_with_bug_fix_loop" => {
            b.step("architect", "Architect", Role::Architect, ARCHITECT);
            b.step("developer", "Developer", Role::Developer, DEVELOPER);
            b.looped("qa", "QA", Role::Qa, QA, "qa_bug_fix");
            b.step("bug-fixer", "Bug Fixer", Role::BugFixer, FIXER);
            b.end("done", "Done");
            b.then("architect", "developer");
            b.then("developer", "qa");
            b.gate("qa", "done", "bug-fixer");
            b.then("bug-fixer", "qa");
        }
        "software_feature" | "full_development" | "refactoring" => {
            let split = template_id == "full_development";
            b.step("architect", "Architect", Role::Architect, ARCHITECT);
            if split {
                b.step("backend", "Backend", Role::Developer, BACKEND);
                b.step("frontend", "Frontend", Role::Developer, FRONTEND);
            } else {
                b.step("developer", "Developer", Role::Developer, DEVELOPER);
            }
            b.looped(
                "validator",
                "Architecture & Code Validator",
                Role::Validator,
                VALIDATOR,
                "architecture_fix",
            );
            b.step(
                "bug-fixer-architecture",
                "Bug Fixer (architecture)",
                Role::BugFixer,
                FIXER,
            );
            b.looped("qa", "QA", Role::Qa, QA, "qa_bug_fix");
            b.step(
                "bug-fixer-qa",
                "Bug Fixer (behaviour)",
                Role::BugFixer,
                FIXER,
            );
            b.end("done", "Done");
            if split {
                b.then("architect", "backend");
                b.then("backend", "frontend");
                b.then("frontend", "validator");
            } else {
                b.then("architect", "developer");
                b.then("developer", "validator");
            }
            b.gate("validator", "qa", "bug-fixer-architecture");
            b.then("bug-fixer-architecture", "validator");
            b.gate("qa", "done", "bug-fixer-qa");
            b.then("bug-fixer-qa", "qa");
        }
        "bug_fix" => {
            b.step("bug-fixer", "Bug Fixer", Role::BugFixer, FIXER);
            b.looped("qa", "QA", Role::Qa, QA, "qa_bug_fix");
            b.step(
                "bug-fixer-retry",
                "Bug Fixer (again)",
                Role::BugFixer,
                FIXER,
            );
            b.end("done", "Done");
            b.then("bug-fixer", "qa");
            b.gate("qa", "done", "bug-fixer-retry");
            b.then("bug-fixer-retry", "qa");
        }
        "code_review" => {
            b.step(
                "validator",
                "Architecture & Code Validator",
                Role::Validator,
                VALIDATOR,
            );
            b.end("done", "Done");
            b.then("validator", "done");
        }
        _ => return None,
    }
    let Builder {
        nodes,
        edges,
        missing,
        ..
    } = b;
    Some(Instantiated {
        workflow: Workflow {
            id: workflow_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            name: name.map_or_else(|| info.name.to_owned(), str::to_owned),
            description: info.description.to_owned(),
            mode,
            version: 1,
            status: WorkflowStatus::Draft,
            template_id: Some(template_id.to_owned()),
            nodes,
            edges,
            viewport: None,
            route_repairs: Vec::new(),
            created_at: now,
            updated_at: now,
        },
        missing_roles: missing,
    })
}
