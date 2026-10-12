//! Workflow templates and the selector that picks one for a task. No model is involved: the same
//! task text always gives the same template. A future planner would produce a [`Workflow`] the
//! same way these do, and the engine would run it unchanged.
//!
//! Two kinds of template exist. The **built-in** ones are code, so a person's edits can never
//! overwrite them. The ones a person **saves** (`CustomTemplate`) are copies of a workflow's steps
//! and routes. Both answer the same question before anything runs: which steps, which agents,
//! which tools, what comes out ([`TemplateInfo`]).

use serde::Serialize;

use super::template_text as text;
use crate::domain::mcp::ToolSelection;
use crate::domain::workflow::{
    AgentNode, Condition, CustomTemplate, EndNode, EndOutcome, ExecutionPolicy, FailurePolicy,
    LoopPolicy, NodeKind, RetryPolicy, SuggestionTarget, ToolCapability, ToolSuggestion, Workflow,
    WorkflowEdge, WorkflowMode, WorkflowNode, WorkflowStatus,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateKind {
    /// Ships with Atlas; read-only.
    Builtin,
    /// Saved by the person; theirs to change or delete.
    Custom,
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

/// One step of a template, as the person reads it before starting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepInfo {
    /// The node's id: the same in every workflow made from the template.
    pub id: String,
    pub label: String,
    /// The role a built-in step is looked up by; a saved template remembers the agent instead.
    pub role: Option<Role>,
    pub agent_id: Option<String>,
    pub instructions: String,
    pub completion_criteria: String,
    /// The step may not change files (the node's own restriction).
    pub read_only: bool,
    /// A step that can run again, bounded by this many passes.
    pub max_iterations: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    pub id: String,
    pub kind: TemplateKind,
    pub category: TemplateCategory,
    /// The template Automatic mode proposes for its category.
    pub recommended: bool,
    /// One of the three Atlas leads with (Implement, Debug & fix, Review & audit).
    pub featured: bool,
    pub name: String,
    pub description: String,
    pub goal: String,
    pub roles: Vec<Role>,
    pub steps: Vec<StepInfo>,
    pub outputs: Vec<String>,
    /// No step may change files.
    pub read_only: bool,
    pub tools: Vec<ToolSuggestion>,
    /// The built-in template a saved one started from.
    pub based_on: Option<String>,
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
    /// The role each agent step was built for (built-in templates only).
    pub step_roles: Vec<(String, Role)>,
    pub tools: Vec<ToolSuggestion>,
}

struct Entry {
    id: &'static str,
    category: TemplateCategory,
    recommended: bool,
    featured: bool,
    name: &'static str,
    description: &'static str,
}

const ENTRIES: [Entry; 8] = [
    Entry {
        id: "software_feature",
        category: TemplateCategory::SoftwareFeature,
        recommended: true,
        featured: true,
        name: "Implement",
        description: "Plans, implements, reviews and checks a change, in depth proportional to the task.",
    },
    Entry {
        id: "feature_basic",
        category: TemplateCategory::SoftwareFeature,
        recommended: false,
        featured: false,
        name: "Quick change",
        description: "Architect, then Developer, then QA: the quick way for a small change.",
    },
    Entry {
        id: "feature_with_validation",
        category: TemplateCategory::SoftwareFeature,
        recommended: false,
        featured: false,
        name: "Feature with validation",
        description: "Architect, Developer, Architecture & Code Validator, then QA.",
    },
    Entry {
        id: "feature_with_bug_fix_loop",
        category: TemplateCategory::SoftwareFeature,
        recommended: false,
        featured: false,
        name: "Feature with fix loop",
        description: "Architect, Developer and QA; when QA fails a Bug Fixer repairs and QA runs again.",
    },
    Entry {
        id: "full_development",
        category: TemplateCategory::SoftwareFeature,
        recommended: false,
        featured: false,
        name: "Full-stack development",
        description: "Architect, Backend, Frontend, validator and QA with both Bug Fixer loops.",
    },
    Entry {
        id: "bug_fix",
        category: TemplateCategory::BugFix,
        recommended: true,
        featured: true,
        name: "Debug & fix",
        description: "Finds the root cause from evidence, fixes it minimally with a regression test, then verifies.",
    },
    Entry {
        id: "refactoring",
        category: TemplateCategory::Refactoring,
        recommended: true,
        featured: false,
        name: "Refactor",
        description: "Architect plans, Developer refactors, the validator checks the architecture, QA checks behaviour.",
    },
    Entry {
        id: "code_review",
        category: TemplateCategory::CodeReview,
        recommended: true,
        featured: true,
        name: "Review & audit",
        description: "Audits code and architecture and reports findings by severity. Analysis only: no file is changed.",
    },
];

fn goal_and_outputs(id: &str, fallback: &str) -> (String, Vec<String>) {
    let owned = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect();
    match id {
        "software_feature" => (text::IMPL_GOAL.to_owned(), owned(&text::IMPL_OUTPUTS)),
        "bug_fix" => (text::DEBUG_GOAL.to_owned(), owned(&text::DEBUG_OUTPUTS)),
        "code_review" => (text::REVIEW_GOAL.to_owned(), owned(&text::REVIEW_OUTPUTS)),
        _ => (fallback.to_owned(), Vec::new()),
    }
}

fn info_of(entry: &Entry) -> TemplateInfo {
    let built = instantiate(entry.id, "", "", None, &[], WorkflowMode::Automatic, 0)
        .expect("every entry builds");
    let (goal, outputs) = goal_and_outputs(entry.id, entry.description);
    let mut roles: Vec<Role> = Vec::new();
    for (_, role) in &built.step_roles {
        if !roles.contains(role) {
            roles.push(*role);
        }
    }
    let steps = steps_of(&built.workflow.nodes, |id| {
        built
            .step_roles
            .iter()
            .find(|(step, _)| step == id)
            .map(|(_, role)| *role)
    });
    TemplateInfo {
        id: entry.id.to_owned(),
        kind: TemplateKind::Builtin,
        category: entry.category,
        recommended: entry.recommended,
        featured: entry.featured,
        name: entry.name.to_owned(),
        description: entry.description.to_owned(),
        goal,
        roles,
        read_only: steps.iter().all(|s| s.read_only),
        steps,
        outputs,
        tools: built.tools,
        based_on: None,
    }
}

fn steps_of(nodes: &[WorkflowNode], role_of: impl Fn(&str) -> Option<Role>) -> Vec<StepInfo> {
    nodes
        .iter()
        .filter_map(|node| {
            let NodeKind::Agent(step) = &node.kind else {
                return None;
            };
            Some(StepInfo {
                id: node.id.clone(),
                label: step.label.clone(),
                role: role_of(&node.id),
                agent_id: Some(step.agent_id.clone()).filter(|id| !id.is_empty()),
                instructions: step.instructions.clone(),
                completion_criteria: step.completion_criteria.clone(),
                read_only: step.execution_policy.read_only,
                max_iterations: node.loop_policy.as_ref().map(|l| l.max_iterations),
            })
        })
        .collect()
}

/// A saved template as the person reads it.
pub fn info_of_custom(custom: &CustomTemplate) -> TemplateInfo {
    let steps = steps_of(&custom.nodes, |_| None);
    let category = custom
        .based_on
        .as_deref()
        .and_then(|id| ENTRIES.iter().find(|e| e.id == id))
        .map_or(TemplateCategory::SoftwareFeature, |e| e.category);
    TemplateInfo {
        id: custom.id.clone(),
        kind: TemplateKind::Custom,
        category,
        recommended: false,
        featured: false,
        name: custom.name.clone(),
        description: custom.description.clone(),
        goal: custom.description.clone(),
        roles: Vec::new(),
        read_only: !steps.is_empty() && steps.iter().all(|s| s.read_only),
        steps,
        outputs: Vec::new(),
        tools: custom.tools.clone(),
        based_on: custom.based_on.clone(),
    }
}

/// The built-in templates, the three featured ones first.
pub fn list() -> Vec<TemplateInfo> {
    let mut all: Vec<TemplateInfo> = ENTRIES.iter().map(info_of).collect();
    all.sort_by_key(|info| !info.featured); // stable: the order of the table within each group
    all
}

pub fn find(id: &str) -> Option<TemplateInfo> {
    ENTRIES.iter().find(|e| e.id == id).map(info_of)
}

/// Words that say the task is a small, local change.
const SMALL_TASK_WORDS: [&str; 10] = [
    "typo",
    "rename",
    "renomear",
    "tweak",
    "small change",
    "pequena",
    "pequeno",
    "one-line",
    "copy change",
    "ajuste de texto",
];

/// What a task says it is, by words: each category has stems (a word that starts with one counts)
/// and exact words (short ones that would match the middle of other words).
struct Cues {
    category: TemplateCategory,
    stems: &'static [&'static str],
    exact: &'static [&'static str],
    /// Phrases, matched anywhere in the text.
    phrases: &'static [&'static str],
}

const CUES: [Cues; 4] = [
    Cues {
        category: TemplateCategory::Refactoring,
        stems: &[
            "refactor",
            "refator",
            "reorganiz",
            "cleanup",
            "limpar",
            "reestrutur",
            "restructur",
        ],
        exact: &[],
        phrases: &[
            "clean up",
            "divida tecnica",
            "tech debt",
            "simplificar o codigo",
        ],
    },
    Cues {
        category: TemplateCategory::CodeReview,
        stems: &["review", "revis", "audit", "avali", "inspecion", "fiscaliz"],
        exact: &[],
        phrases: &["analisar o codigo", "analyze the code", "code review"],
    },
    Cues {
        category: TemplateCategory::BugFix,
        stems: &[
            "bug",
            "fix",
            "error",
            "erro",
            "falha",
            "corrig",
            "correc",
            "crash",
            "broken",
            "quebrad",
            "quebra",
            "exception",
            "regress",
            "problem",
            "acontecendo",
            "happening",
            "travando",
            "errado",
            "incorret",
            "inesperad",
            "unexpected",
            "wrong",
            "stuck",
            "disappear",
            "sumindo",
        ],
        exact: &["issue", "issues", "falhou", "failed", "fails"],
        phrases: &[
            "nao funciona",
            "not working",
            "doesn't work",
            "does not work",
            "nao esta funcionando",
        ],
    },
    Cues {
        category: TemplateCategory::SoftwareFeature,
        stems: &[
            "implement",
            "adicion",
            "criar",
            "create",
            "feature",
            "funcionalidade",
            "construir",
            "permitir",
            "integrar",
            "integrate",
        ],
        exact: &["add", "new", "novo", "nova", "build"],
        phrases: &["suporte a", "support for", "nova funcionalidade"],
    },
];

/// Lower case, without accents, so "correção" and "correcao" are the same word.
fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// How many of the category's cues the task carries, each counted once.
fn score(cues: &Cues, folded: &str, words: &[&str]) -> usize {
    let stems = cues
        .stems
        .iter()
        .filter(|stem| words.iter().any(|w| w.starts_with(**stem)))
        .count();
    let exact = cues.exact.iter().filter(|e| words.contains(e)).count();
    let phrases = cues.phrases.iter().filter(|p| folded.contains(**p)).count();
    stems + exact + phrases
}

/// The template Automatic mode proposes for a task, by what the task says it is: the category
/// whose cues it carries most wins (a tie goes to the more specific one: refactoring, then review,
/// then bug fix), and the feature template is the fallback. A short task that calls itself small
/// gets the quick feature template instead of the full one. Never a model: the same text always
/// gives the same template, and the person can always pick another.
pub fn select_for_task(task: &str) -> &'static str {
    let folded = fold(task);
    let words: Vec<&str> = folded
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .collect();
    let mut best: Option<(&Cues, usize)> = None;
    for cues in &CUES {
        let found = score(cues, &folded, &words);
        // Strictly more beats an earlier category, so ties keep the more specific one.
        if found > 0 && best.is_none_or(|(_, top)| found > top) {
            best = Some((cues, found));
        }
    }
    let category = best.map_or(TemplateCategory::SoftwareFeature, |(c, _)| c.category);
    if category == TemplateCategory::SoftwareFeature
        && task.chars().count() <= 140
        && SMALL_TASK_WORDS.iter().any(|w| folded.contains(w))
    {
        return "feature_basic";
    }
    ENTRIES
        .iter()
        .find(|e| e.category == category && e.recommended)
        .map_or("software_feature", |e| e.id)
}

// ---- building -----------------------------------------------------------------------------------

struct Builder<'a> {
    agents: &'a [AgentChoice],
    nodes: Vec<WorkflowNode>,
    edges: Vec<WorkflowEdge>,
    missing: Vec<Role>,
    roles: Vec<(String, Role)>,
    tools: Vec<ToolSuggestion>,
}

impl<'a> Builder<'a> {
    fn new(agents: &'a [AgentChoice]) -> Self {
        Self {
            agents,
            nodes: Vec::new(),
            edges: Vec::new(),
            missing: Vec::new(),
            roles: Vec::new(),
            tools: Vec::new(),
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
        self.roles.push((id.to_owned(), role));
        self.nodes.push(WorkflowNode {
            id: id.to_owned(),
            kind: NodeKind::Agent(AgentNode {
                agent_id,
                label: label.to_owned(),
                instructions: instructions.to_owned(),
                completion_criteria: String::new(),
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

    /// What must be true for the step to be done.
    fn done_when(&mut self, id: &str, criteria: &str) {
        if let Some(step) = self.agent_mut(id) {
            criteria.clone_into(&mut step.completion_criteria);
        }
    }

    /// The step may not change files, whatever its agent could do.
    fn read_only(&mut self, id: &str) {
        if let Some(step) = self.agent_mut(id) {
            step.execution_policy.read_only = true;
        }
    }

    fn agent_mut(&mut self, id: &str) -> Option<&mut AgentNode> {
        self.nodes
            .iter_mut()
            .find(|n| n.id == id)
            .and_then(|n| match &mut n.kind {
                NodeKind::Agent(step) => Some(step),
                _ => None,
            })
    }

    /// A tool the step is better off with, by kind.
    fn suggest(&mut self, node_id: &str, capability: ToolCapability, purpose: &str) {
        self.tools.push(ToolSuggestion {
            node_id: node_id.to_owned(),
            target: SuggestionTarget::Capability { capability },
            purpose: purpose.to_owned(),
        });
    }

    fn end(&mut self, id: &str, label: &str) {
        self.end_as(id, label, EndOutcome::Done);
    }

    fn end_as(&mut self, id: &str, label: &str, outcome: EndOutcome) {
        self.nodes.push(WorkflowNode {
            id: id.to_owned(),
            kind: NodeKind::End(EndNode {
                label: label.to_owned(),
                outcome,
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

    /// The outcomes the agent of a step declares.
    fn declared(&self, from: &str) -> Vec<String> {
        self.nodes
            .iter()
            .find(|n| n.id == from)
            .and_then(|n| n.agent_id())
            .and_then(|id| self.agents.iter().find(|a| a.id == id))
            .map(|a| a.outcomes.clone())
            .unwrap_or_default()
    }

    /// The two ways out of a step that judges: on to `ok_to`, or to `problem_to` to be corrected.
    /// Which outcomes those are comes from the contract of the agent the step got: the pair of
    /// the `validation` preset (`pass`/`fail`) or of the `review` preset (`approved`/
    /// `changes_requested`), whichever it declares in full. These pairs are Atlas's own presets,
    /// not a guess about an agent's wording; an agent declaring neither gets `pass`/`fail`, and
    /// validation then says what it lacks, since no pair can be chosen for it.
    fn gate(&mut self, from: &str, ok_to: &str, problem_to: &str) {
        let declared = self.declared(from);
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

    /// A step that fixes: on to `ok_to` when it says it fixed, to `blocked_to` when it says it
    /// could not. An agent that does not declare that pair (a general one) goes on to `ok_to`
    /// whatever it says, so the template is never born with a route its agent cannot take.
    fn fix_gate(&mut self, from: &str, ok_to: &str, blocked_to: &str) {
        let declared = self.declared(from);
        let has = |o: &str| declared.iter().any(|d| d.eq_ignore_ascii_case(o));
        if has("fixed") && has("blocked") {
            // The way out for a fixer that could not establish the cause. Made here, only when
            // something can lead to it: a node nothing reaches is a broken workflow.
            if !self.nodes.iter().any(|n| n.id == blocked_to) {
                self.end_as(blocked_to, "Inconclusive", EndOutcome::Failed);
            }
            self.on(from, ok_to, "fixed");
            self.on(from, blocked_to, "blocked");
        } else {
            self.then(from, ok_to);
        }
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

/// Builds the workflow of a built-in template for a workspace. `None` for an unknown template.
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
    let entry = ENTRIES.iter().find(|e| e.id == template_id)?;
    let mut b = Builder::new(agents);
    match template_id {
        "feature_basic" => {
            b.step(
                "architect",
                "Architect",
                Role::Architect,
                text::IMPL_ARCHITECT,
            );
            b.step(
                "developer",
                "Developer",
                Role::Developer,
                text::IMPL_DEVELOPER,
            );
            b.step("qa", "QA", Role::Qa, text::IMPL_QA);
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
            b.step("architect", "Architect", Role::Architect, text::ARCHITECT);
            b.step("developer", "Developer", Role::Developer, text::DEVELOPER);
            b.step(
                "validator",
                "Architecture & Code Validator",
                Role::Validator,
                text::VALIDATOR,
            );
            b.step("qa", "QA", Role::Qa, text::QA);
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
            b.step("architect", "Architect", Role::Architect, text::ARCHITECT);
            b.step("developer", "Developer", Role::Developer, text::DEVELOPER);
            b.looped("qa", "QA", Role::Qa, text::QA, "qa_bug_fix");
            b.step("bug-fixer", "Bug Fixer", Role::BugFixer, text::FIXER);
            b.end("done", "Done");
            b.then("architect", "developer");
            b.then("developer", "qa");
            b.gate("qa", "done", "bug-fixer");
            b.then("bug-fixer", "qa");
        }
        "software_feature" => {
            // The Implementer: analysis and a plan sized to the task, the change, a read-only review
            // that sends problems to a fixer, and a behaviour check that does the same.
            b.step(
                "architect",
                "Analyze and plan",
                Role::Architect,
                text::IMPL_ARCHITECT,
            );
            b.step(
                "developer",
                "Implement",
                Role::Developer,
                text::IMPL_DEVELOPER,
            );
            b.looped(
                "validator",
                "Review the change",
                Role::Validator,
                text::IMPL_VALIDATOR,
                "architecture_fix",
            );
            b.step(
                "bug-fixer-architecture",
                "Fix review findings",
                Role::BugFixer,
                text::IMPL_FIXER,
            );
            b.looped(
                "qa",
                "Check behaviour",
                Role::Qa,
                text::IMPL_QA,
                "qa_bug_fix",
            );
            b.step(
                "bug-fixer-qa",
                "Fix behaviour problems",
                Role::BugFixer,
                text::IMPL_FIXER,
            );
            b.end("done", "Done");
            for (id, criteria) in [
                ("architect", text::IMPL_ARCHITECT_DONE),
                ("developer", text::IMPL_DEVELOPER_DONE),
                ("validator", text::IMPL_VALIDATOR_DONE),
                ("bug-fixer-architecture", text::IMPL_FIXER_DONE),
                ("qa", text::IMPL_QA_DONE),
                ("bug-fixer-qa", text::IMPL_FIXER_DONE),
            ] {
                b.done_when(id, criteria);
            }
            b.read_only("validator");
            b.read_only("qa");
            b.suggest(
                "architect",
                ToolCapability::Issues,
                "Read the ticket the task refers to, when the design depends on it.",
            );
            b.suggest(
                "architect",
                ToolCapability::Docs,
                "Read the documentation of a library or API the design relies on.",
            );
            b.suggest(
                "developer",
                ToolCapability::Docs,
                "Check an API's documented behaviour before using it.",
            );
            b.suggest(
                "qa",
                ToolCapability::Browser,
                "Open the page to see the behaviour when the change is a web interface.",
            );
            b.then("architect", "developer");
            b.then("developer", "validator");
            b.gate("validator", "qa", "bug-fixer-architecture");
            b.then("bug-fixer-architecture", "validator");
            b.gate("qa", "done", "bug-fixer-qa");
            b.then("bug-fixer-qa", "qa");
        }
        "full_development" | "refactoring" => {
            let split = template_id == "full_development";
            b.step("architect", "Architect", Role::Architect, text::ARCHITECT);
            if split {
                b.step("backend", "Backend", Role::Developer, text::BACKEND);
                b.step("frontend", "Frontend", Role::Developer, text::FRONTEND);
            } else {
                b.step("developer", "Developer", Role::Developer, text::DEVELOPER);
            }
            b.looped(
                "validator",
                "Architecture & Code Validator",
                Role::Validator,
                text::VALIDATOR,
                "architecture_fix",
            );
            b.step(
                "bug-fixer-architecture",
                "Bug Fixer (architecture)",
                Role::BugFixer,
                text::FIXER,
            );
            b.looped("qa", "QA", Role::Qa, text::QA, "qa_bug_fix");
            b.step(
                "bug-fixer-qa",
                "Bug Fixer (behaviour)",
                Role::BugFixer,
                text::FIXER,
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
            // The Debugger: evidence and root cause first, then the smallest fix with a regression
            // test, then a read-only verification that writes the final report. A fixer that
            // reports it is blocked ends the run as inconclusive instead of guessing.
            b.step(
                "bug-fixer",
                "Investigate and fix",
                Role::BugFixer,
                text::DEBUG_FIXER,
            );
            b.looped(
                "qa",
                "Verify the fix",
                Role::Qa,
                text::DEBUG_VERIFY,
                "qa_bug_fix",
            );
            b.step(
                "bug-fixer-retry",
                "Investigate again and fix",
                Role::BugFixer,
                text::DEBUG_RETRY,
            );
            b.end("done", "Done");
            b.done_when("bug-fixer", text::DEBUG_FIXER_DONE);
            b.done_when("bug-fixer-retry", text::DEBUG_FIXER_DONE);
            b.done_when("qa", text::DEBUG_VERIFY_DONE);
            b.read_only("qa");
            b.suggest(
                "bug-fixer",
                ToolCapability::Browser,
                "Reproduce a web problem and read its console and network errors.",
            );
            b.suggest(
                "bug-fixer",
                ToolCapability::Issues,
                "Read the bug report and what was said about it.",
            );
            b.suggest(
                "bug-fixer",
                ToolCapability::Repo,
                "Look at recent changes and related reports in the repository's history.",
            );
            b.suggest(
                "bug-fixer",
                ToolCapability::Docs,
                "Check how a library is documented to behave before blaming it or the code.",
            );
            b.suggest(
                "qa",
                ToolCapability::Browser,
                "Load the page again to check the fixed behaviour.",
            );
            b.fix_gate("bug-fixer", "qa", "inconclusive");
            b.gate("qa", "done", "bug-fixer-retry");
            b.fix_gate("bug-fixer-retry", "qa", "inconclusive");
        }
        "code_review" => {
            // The Reviewer: one read-only audit. It changes nothing; a fix is a separate run the
            // person starts.
            b.step(
                "validator",
                "Review code and architecture",
                Role::Validator,
                text::REVIEW_AUDIT,
            );
            b.end("done", "Done");
            b.done_when("validator", text::REVIEW_AUDIT_DONE);
            b.read_only("validator");
            b.suggest(
                "validator",
                ToolCapability::Issues,
                "Read the ticket the change was meant to implement.",
            );
            b.suggest(
                "validator",
                ToolCapability::Docs,
                "Check that a library is used the way its documentation says.",
            );
            b.suggest(
                "validator",
                ToolCapability::Repo,
                "Read the history and the diff of the change under review.",
            );
            b.then("validator", "done");
        }
        _ => return None,
    }
    let Builder {
        nodes,
        edges,
        missing,
        roles,
        tools,
        ..
    } = b;
    Some(Instantiated {
        workflow: Workflow {
            id: workflow_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            name: name.map_or_else(|| entry.name.to_owned(), str::to_owned),
            description: entry.description.to_owned(),
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
        step_roles: roles,
        tools,
    })
}

/// Builds a workflow from a template a person saved. The steps, routes and agents are the ones it
/// was saved with; an agent that no longer exists shows up in validation like in any workflow.
pub fn instantiate_custom(
    custom: &CustomTemplate,
    workspace_id: &str,
    workflow_id: &str,
    name: Option<&str>,
    mode: WorkflowMode,
    now: u64,
) -> Workflow {
    Workflow {
        id: workflow_id.to_owned(),
        workspace_id: workspace_id.to_owned(),
        name: name.map_or_else(|| custom.name.clone(), str::to_owned),
        description: custom.description.clone(),
        mode,
        version: 1,
        status: WorkflowStatus::Draft,
        template_id: Some(custom.id.clone()),
        nodes: custom.nodes.clone(),
        edges: custom.edges.clone(),
        viewport: None,
        route_repairs: Vec::new(),
        created_at: now,
        updated_at: now,
    }
}

/// What a connection was granted to a step, as a suggestion a saved template can carry.
pub fn suggestion_of_grant(
    node_id: &str,
    connection: &str,
    tools: &ToolSelection,
) -> ToolSuggestion {
    ToolSuggestion {
        node_id: node_id.to_owned(),
        target: SuggestionTarget::Connection {
            name: connection.to_owned(),
            tools: tools.clone(),
        },
        purpose: String::new(),
    }
}
