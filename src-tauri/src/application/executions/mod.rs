mod guardrails;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Deserialize;

use super::agents::AgentService;
use super::errors::{AppError, ErrorCode};
use super::harness::context::{HarnessContextBuilder, TaskContextLoad};
use super::interaction::{should_pause, InteractionDetector, InteractionSignals, LayeredDetector};
use super::optimization::skills::{SelectionInput, SkillService};
use super::optimization::{self, OptimizationFlags, PreRuntime, RuntimeProbe};
use super::personalities::PersonalityService;
use super::process::ExecutionScope;
use super::prompt::PromptBuilder;
use super::runtimes::{RuntimeError, RuntimeEvent, RuntimeOutput, RuntimeRegistry, RuntimeRequest};
use super::security::{AuditLog, PolicyResolver};
use super::sessions::SessionRegistry;
use super::support::now_ms;
use super::workspace::WorkspaceService;
use super::worktree::{RunOutcome, WorktreeError, WorktreeService};
use crate::domain::execution::{
    Execution, ExecutionEvent, ExecutionEventKind, ExecutionFailure, ExecutionRecord,
    ExecutionStatus,
};
use crate::domain::guardrail::ReviewAnswer;
use crate::domain::interaction::InteractionDetection;
use crate::domain::optimization::BriefParts;
use crate::domain::security::{Permission, ToolAccess};
use crate::domain::task::{Task, TaskStatus};
use crate::domain::task_context::{ContextMode, ContextRecord};
use crate::domain::terminal::UserAction;
use crate::domain::worktree::Validation;

/// Port: receives progress while an execution runs. The Tauri adapter in `commands/`
/// forwards these to the webview; tests collect them.
pub trait ExecutionObserver: Send + Sync {
    fn on_event(&self, event: &ExecutionEvent);
}

/// Whether an execution may edit files, and what stops it when it may not. The runtime's
/// capability and the agent's permission are different things and are told apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditAccess {
    Allowed,
    /// The agent's policy does not allow writing files.
    PolicyDenied,
    /// The agent may write, but not outside an isolated worktree, and it has none.
    NotIsolated,
    /// The agent may write, but its runtime cannot be launched with file-editing tools.
    RuntimeCannotEdit,
}

/// What to run: which agent, in which workspace (whose project folder is the working
/// directory), and the task. `task_id` is chosen by the caller.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAgentRequest {
    pub task_id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub description: String,
}

/// How a workflow step differs from a message: where its Task Context comes from, the worktree it
/// shares, and whether an answer that is a request for a person pauses it instead of ending it.
#[derive(Debug, Clone, Default)]
pub struct StepOptions<'a> {
    /// The text the Task Context is selected for, when it differs from the instruction itself.
    pub context_query: Option<&'a str>,
    /// The primary worktree of the workflow run the step works in.
    pub shared_worktree: Option<&'a str>,
    /// The parts of a workflow step's brief, for measuring the prompt and, when the Context
    /// Engine is on, for removing what the brief says twice.
    pub brief_parts: Option<&'a BriefParts>,
    /// What a person answered when a guardrail asked about this step's context: the only thing
    /// that settles that question.
    pub review: Option<ReviewAnswer>,
    /// Look at the answer for a request for a person. Only workflow steps ask for this: in a
    /// conversation the person simply replies.
    pub detect_interaction: bool,
}

/// The request was not acceptable, so no execution was created. Runtime problems are not
/// errors here: they produce a *failed* execution that is recorded like any other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    EmptyTask,
    UnknownAgent(String),
    UnknownPersonality(String),
    UnknownRuntime(String),
    UnknownWorkspace(String),
}

impl From<&ExecutionError> for AppError {
    fn from(error: &ExecutionError) -> Self {
        let code = match error {
            ExecutionError::EmptyTask => ErrorCode::MessageEmpty,
            ExecutionError::UnknownAgent(_) => ErrorCode::AgentNotFound,
            ExecutionError::UnknownPersonality(_) => ErrorCode::PersonalityNotFound,
            ExecutionError::UnknownRuntime(_) => ErrorCode::RuntimeRequired,
            ExecutionError::UnknownWorkspace(_) => ErrorCode::WorkspaceNotFound,
        };
        AppError::new(code).with_detail(error.to_string())
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTask => write!(f, "The task description is empty"),
            Self::UnknownAgent(id) => write!(f, "Unknown agent: {id}"),
            Self::UnknownPersonality(id) => {
                write!(f, "The agent's personality no longer exists: {id}")
            }
            Self::UnknownRuntime(id) => write!(f, "The agent's runtime is unknown: {id}"),
            Self::UnknownWorkspace(id) => write!(f, "Unknown workspace: {id}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

/// Use case: run a task with an agent through its runtime. Knows nothing about any specific
/// runtime.
pub struct ExecutionService {
    agents: Arc<AgentService>,
    personalities: Arc<PersonalityService>,
    runtimes: Arc<RuntimeRegistry>,
    workspaces: Arc<WorkspaceService>,
    /// Where the process port records the permission decisions of each execution.
    audit: Arc<AuditLog>,
    /// The live processes of executions that run in a terminal. Asked, after a run ends badly,
    /// whether the user stopped it.
    sessions: Arc<SessionRegistry>,
    /// Where isolated executions get their worktree. Without it an agent that asks for
    /// isolation fails (it never falls back to the project's checkout).
    worktrees: Option<Arc<WorktreeService>>,
    /// Where the project's Harness is read from, at the project's own root. Without it prompts
    /// carry no Harness.
    harness: Option<Arc<HarnessContextBuilder>>,
    /// Where an agent's policy is read from, to tell whether its execution may edit files.
    /// Without it no execution ever gets file-editing tools.
    policies: Option<Arc<dyn PolicyResolver>>,
    /// Decides whether an answer is a request for a person (see [`StepOptions`]).
    interactions: Arc<dyn InteractionDetector>,
    /// Which parts of the Optimization Layer are on. Without it nothing is measured.
    optimization: Option<Arc<dyn OptimizationFlags>>,
    /// Where skills are found. Without it (or with `optimization.skills.enabled` off) no skill is
    /// ever selected.
    skills: Option<Arc<SkillService>>,
    next_id: AtomicU64,
}

impl ExecutionService {
    pub fn new(
        agents: Arc<AgentService>,
        personalities: Arc<PersonalityService>,
        runtimes: Arc<RuntimeRegistry>,
        workspaces: Arc<WorkspaceService>,
        audit: Arc<AuditLog>,
    ) -> Self {
        Self {
            agents,
            personalities,
            runtimes,
            workspaces,
            audit,
            sessions: Arc::default(),
            worktrees: None,
            harness: None,
            policies: None,
            interactions: Arc::new(LayeredDetector),
            optimization: None,
            skills: None,
            next_id: AtomicU64::new(1),
        }
    }

    /// Uses the registry the process runner publishes its sessions to (the default is an empty
    /// one: nothing is ever reported as stopped by the user).
    #[must_use]
    pub fn with_sessions(mut self, sessions: Arc<SessionRegistry>) -> Self {
        self.sessions = sessions;
        self
    }

    /// Runs agents that ask for it in their own Git worktree, created and judged by `worktrees`.
    #[must_use]
    pub fn with_worktrees(mut self, worktrees: Arc<WorktreeService>) -> Self {
        self.worktrees = Some(worktrees);
        self
    }

    /// Gives prompts the project's Harness context.
    #[must_use]
    pub fn with_harness(mut self, harness: Arc<HarnessContextBuilder>) -> Self {
        self.harness = Some(harness);
        self
    }

    /// Measures executions (what the prompt is made of, how long each stage took) while the flag
    /// says so. Measuring never changes the prompt, the runtime request or the policy.
    #[must_use]
    pub fn with_optimization(mut self, flags: Arc<dyn OptimizationFlags>) -> Self {
        self.optimization = Some(flags);
        self
    }

    /// Lets executions use the skills `skills` finds, while the flag says so.
    #[must_use]
    pub fn with_skills(mut self, skills: Arc<SkillService>) -> Self {
        self.skills = Some(skills);
        self
    }

    /// Lets an execution that works in an isolated worktree edit files when the agent's policy
    /// allows writing there (never otherwise, and never outside the worktree).
    #[must_use]
    pub fn with_policies(mut self, policies: Arc<dyn PolicyResolver>) -> Self {
        self.policies = Some(policies);
        self
    }

    /// Whether this execution may be given file-editing tools, and if not, why. Three things
    /// must all hold: the runtime can be launched with such tools (a capability of the runtime,
    /// never a rule about which runtime it is), the agent's own policy (workspace ceiling and
    /// permission profile) allows file writes, and the agent works in an isolated worktree (so
    /// an edit can never reach the project's checkout). Anything unknown is a no.
    fn edit_access(
        &self,
        execution: &Execution,
        agent: &crate::domain::agent::Agent,
        task_id: &str,
        runtime_can_edit: bool,
    ) -> EditAccess {
        let Some(policies) = &self.policies else {
            return EditAccess::PolicyDenied;
        };
        let scope = ExecutionScope {
            workspace_id: execution.workspace_id.clone(),
            agent_id: agent.id.clone(),
            execution_id: execution.id.clone(),
            task_id: task_id.to_owned(),
            runtime_access: ToolAccess::NONE,
            isolated: agent.worktree_isolation,
        };
        // Whether the policy would allow it, whatever the runtime can do.
        let policy_allows = policies
            .resolve(&scope)
            .is_ok_and(|resolved| resolved.agent_policy.filesystem.write == Permission::Allowed);
        Self::decide_edit(runtime_can_edit, policy_allows, agent.worktree_isolation)
    }

    /// The agent's resolved policies as text, for what an approval is bound to: the policy the
    /// workspace and the profile resolve to now, not the files they came from.
    fn policy_digest(
        &self,
        execution: &Execution,
        agent: &crate::domain::agent::Agent,
        task_id: &str,
    ) -> String {
        let Some(policies) = &self.policies else {
            return "none".to_owned();
        };
        let scope = ExecutionScope {
            workspace_id: execution.workspace_id.clone(),
            agent_id: agent.id.clone(),
            execution_id: execution.id.clone(),
            task_id: task_id.to_owned(),
            runtime_access: ToolAccess::NONE,
            isolated: agent.worktree_isolation,
        };
        match policies.resolve(&scope) {
            Ok(resolved) => format!(
                "{}|{}",
                serde_json::to_string(&resolved.agent_policy).unwrap_or_default(),
                serde_json::to_string(&resolved.policy).unwrap_or_default()
            ),
            Err(reason) => format!("unresolved:{}", reason.as_str()),
        }
    }

    fn decide_edit(runtime_can_edit: bool, policy_allows: bool, isolated: bool) -> EditAccess {
        if !policy_allows {
            EditAccess::PolicyDenied
        } else if !isolated {
            EditAccess::NotIsolated
        } else if !runtime_can_edit {
            EditAccess::RuntimeCannotEdit
        } else {
            EditAccess::Allowed
        }
    }

    /// Whether a step of this agent would edit files, decided before the step exists (so a
    /// workflow can tell a writing step from a reading one). `false` for anything unknown.
    pub fn would_write(&self, workspace_id: &str, agent_id: &str) -> bool {
        let (Some(policies), Some(agent)) = (&self.policies, self.agents.find(agent_id)) else {
            return false;
        };
        let Some(runtime) = self.runtimes.find(&agent.runtime_id) else {
            return false;
        };
        // The policy is the agent's own: it does not depend on a worktree that is not made yet.
        let scope = ExecutionScope {
            workspace_id: workspace_id.to_owned(),
            agent_id: agent.id.clone(),
            execution_id: String::new(),
            task_id: String::new(),
            runtime_access: ToolAccess::NONE,
            isolated: false,
        };
        let policy_allows = policies
            .resolve(&scope)
            .is_ok_and(|resolved| resolved.agent_policy.filesystem.write == Permission::Allowed);
        Self::decide_edit(
            runtime.info().capabilities.file_edit,
            policy_allows,
            agent.worktree_isolation,
        ) == EditAccess::Allowed
    }

    /// Whether the agent works in an isolated worktree.
    pub fn is_isolated(&self, agent_id: &str) -> bool {
        self.agents
            .find(agent_id)
            .is_some_and(|a| a.worktree_isolation)
    }

    /// Numbers new executions from `first`, so ids never repeat those stored by an earlier run.
    #[must_use]
    pub fn with_first_id(self, first: u64) -> Self {
        self.next_id.store(first, Ordering::Relaxed);
        self
    }

    /// An id for an execution that will be started with [`Self::run_with_id`], for callers
    /// that must tell the user which execution they started before it finishes.
    pub fn next_execution_id(&self) -> String {
        format!("exec-{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Builds the prompt, calls the agent's runtime and returns the outcome. Blocks until the
    /// runtime answers, so call it from a blocking context.
    ///
    /// # Errors
    ///
    /// Fails before any execution is created if the task is empty or the agent, its
    /// personality or runtime, or the workspace is unknown.
    pub fn run_with_id(
        &self,
        id: String,
        request: RunAgentRequest,
        observer: &dyn ExecutionObserver,
    ) -> Result<ExecutionRecord, ExecutionError> {
        self.run_for_context(id, request, None, None, observer)
    }

    /// Like [`Self::run_with_id`], for callers (workflow steps) whose instruction carries more
    /// than what the Harness context should be chosen for: `context_query` is the text the Task
    /// Context is selected for, when it differs from the instruction itself.
    ///
    /// # Errors
    ///
    /// As [`Self::run_with_id`].
    pub fn run_for_context(
        &self,
        id: String,
        request: RunAgentRequest,
        context_query: Option<&str>,
        shared_worktree: Option<&str>,
        observer: &dyn ExecutionObserver,
    ) -> Result<ExecutionRecord, ExecutionError> {
        let options = StepOptions {
            context_query,
            shared_worktree,
            brief_parts: None,
            review: None,
            detect_interaction: false,
        };
        self.run_step(id, request, options, observer)
    }

    /// [`Self::run_for_context`] with every way a workflow step differs from a message.
    ///
    /// # Errors
    ///
    /// As [`Self::run_with_id`].
    #[allow(clippy::too_many_lines)]
    pub fn run_step(
        &self,
        id: String,
        request: RunAgentRequest,
        options: StepOptions<'_>,
        observer: &dyn ExecutionObserver,
    ) -> Result<ExecutionRecord, ExecutionError> {
        let StepOptions {
            context_query,
            shared_worktree,
            brief_parts,
            review: review_answer,
            detect_interaction,
        } = options;
        // Read once: a run is measured entirely or not at all.
        let measuring = self
            .optimization
            .as_ref()
            .is_some_and(|flags| flags.metrics_enabled());
        let run_timer = measuring.then(std::time::Instant::now);
        let description = request.description.trim();
        if description.is_empty() {
            return Err(ExecutionError::EmptyTask);
        }
        let agent = self
            .agents
            .find(&request.agent_id)
            .ok_or_else(|| ExecutionError::UnknownAgent(request.agent_id.clone()))?;
        let personality = self
            .personalities
            .find(&agent.personality_id)
            .ok_or_else(|| ExecutionError::UnknownPersonality(agent.personality_id.clone()))?;
        let runtime = self
            .runtimes
            .find(&agent.runtime_id)
            .ok_or_else(|| ExecutionError::UnknownRuntime(agent.runtime_id.clone()))?;
        let project = self
            .workspaces
            .project_context(&request.workspace_id)
            .map_err(|_| ExecutionError::UnknownWorkspace(request.workspace_id.clone()))?;

        let mut task = Task {
            id: request.task_id,
            description: description.to_owned(),
            agent_id: agent.id.clone(),
            status: TaskStatus::Pending,
        };
        let runtime_name = runtime.info().name;
        // The prompt is only known once the working directory is: it tells the model where it
        // is working, and that must be the worktree, never the project's checkout.
        let mut execution = Execution::start(
            id,
            request.workspace_id.clone(),
            task.id.clone(),
            agent.runtime_id.clone(),
            agent.model_id.clone(),
            String::new(),
            now_ms(),
        );

        task.status = TaskStatus::Running;
        let emitter = Emitter {
            observer,
            execution_id: execution.id.clone(),
            workspace_id: request.workspace_id,
            task_id: task.id.clone(),
            agent_id: agent.id.clone(),
            logs: RefCell::new(vec![
                "Task created".to_owned(),
                format!(
                    "Prompt built from the {} personality and project context",
                    personality.name
                ),
            ]),
        };
        emitter.emit(ExecutionEventKind::Started, "Execution started".to_owned());

        // The Harness belongs to the project, not to a worktree: it is read at the project's own
        // root before the path is swapped for the worktree's, and reaches the agent as prompt
        // text only. The runtime and the worktree never see `.atlas/`. What the agent is told of
        // it is chosen for the task; the whole Harness context is only a recorded fallback.
        let context_timer = measuring.then(std::time::Instant::now);
        let (harness, task_aware, context) = self.load_harness(
            &emitter,
            &project.path,
            context_query.unwrap_or(description),
        );
        // With no Harness service there is nothing to build, so no time is claimed for it.
        let context_build_ms = self
            .harness
            .as_ref()
            .and_then(|_| optimization::elapsed_ms(context_timer));
        if measuring {
            emitter.announce(
                ExecutionEventKind::OptimizationContextBuilt,
                "Context built".to_owned(),
                Self::context_event_metadata(context.as_ref(), context_build_ms),
            );
        }
        execution.context = context;
        // Skills are read from the project's own folder like the Harness, before the path is
        // swapped for the worktree's. What the task calls for goes into the prompt; the rest is
        // never sent.
        let skill_plan = self
            .skills
            .as_ref()
            .filter(|_| {
                self.optimization
                    .as_ref()
                    .is_some_and(|flags| flags.skills_enabled())
            })
            .map(|skills| {
                let plan = skills.prepare(
                    &project.path,
                    &SelectionInput {
                        task: context_query.unwrap_or(description),
                        personality_id: &personality.id,
                        technologies: &project.technologies,
                    },
                );
                emitter.announce(
                    ExecutionEventKind::OptimizationSkillsSelected,
                    "Skills selected".to_owned(),
                    Self::skills_event_metadata(&plan),
                );
                plan
            });
        let mut project = project;
        let working_dir = if agent.worktree_isolation {
            match self.start_worktree(
                &emitter,
                &execution,
                &agent.id,
                &project.path,
                shared_worktree,
            ) {
                Ok(dir) => dir,
                Err(error) => {
                    return Ok(self.abandon(emitter, execution, task, &error));
                }
            }
        } else {
            project.path.clone().into()
        };
        project.path = working_dir.to_string_lossy().into_owned();
        let access = self.edit_access(
            &execution,
            &agent,
            &task.id,
            runtime.info().capabilities.file_edit,
        );
        if access == EditAccess::RuntimeCannotEdit {
            // Not a permission problem: the agent may write, this runtime cannot be asked to.
            emitter.log("The agent may edit files, but its runtime does not support file editing");
        }
        let can_edit = access == EditAccess::Allowed;
        let prompt_timer = measuring.then(std::time::Instant::now);
        // What the prompt is built from. The Context Engine and the guardrails rework these inputs
        // (never a built prompt) and the builder, still the only assembly, builds again.
        let mut inputs = guardrails::PromptInputs {
            harness: harness.clone(),
            skills: skill_plan
                .as_ref()
                .map(|plan| plan.blocks.clone())
                .unwrap_or_default(),
            description: task.description.clone(),
            parts: brief_parts.cloned(),
        };
        let build = |inputs: &guardrails::PromptInputs| {
            let reworked = Task {
                description: inputs.description.clone(),
                ..task.clone()
            };
            PromptBuilder::assemble(
                &personality,
                &project,
                inputs.harness.as_deref(),
                task_aware,
                &agent,
                &reworked,
                can_edit,
                inputs.skills_text().as_deref(),
            )
        };
        let (mut prompt, mut layout) = build(&inputs);
        let mut combined = prompt.combined();
        let mut context_engine = None;
        if let Some(flags) = self.optimization.as_ref().filter(|f| f.context_enabled()) {
            let outcome = optimization::context::optimize_prompt_inputs(
                &optimization::context::ContextInputs {
                    prompt: &prompt,
                    combined: &combined,
                    agent_instructions: &agent.instructions,
                    task_description: &task.description,
                    brief: brief_parts,
                    skills: &inputs.skills,
                    budget: optimization::context::ContextBudget {
                        max_tokens: flags.context_max_tokens(),
                        ..optimization::context::ContextBudget::default()
                    },
                },
            );
            if outcome.changed {
                inputs.harness.clone_from(&outcome.harness);
                inputs.skills.clone_from(&outcome.skill_blocks);
                inputs.description.clone_from(&outcome.task_description);
                if outcome.brief_parts.is_some() {
                    inputs.parts.clone_from(&outcome.brief_parts);
                }
                (prompt, layout) = build(&inputs);
                combined = prompt.combined();
            }
            let engine = outcome.metrics(&combined);
            Self::announce_context_engine(&emitter, &engine);
            context_engine = Some(engine);
        }
        // The guardrails look at the context as it is about to be sent: whole, coherent, free of
        // secrets that came from files and other agents. A person decides what needs deciding.
        let mut gate = guardrails::Gate::Proceed;
        let mut guard_summary = None;
        if self.guardrails_on() {
            let stage = self.guard_before_agent(
                &emitter,
                &execution,
                &guardrails::GuardContext {
                    prompt: &prompt,
                    agent_instructions: &agent.instructions,
                    inputs: &inputs,
                    engine: context_engine.as_ref(),
                    skills: skill_plan.as_ref().map(|plan| &plan.metrics),
                    record: execution.context.as_ref(),
                    step: detect_interaction,
                    answered: review_answer,
                    access,
                    binding: guardrails::Binding {
                        agent_id: agent.id.clone(),
                        runtime_id: agent.runtime_id.clone(),
                        model_id: agent.model_id.clone(),
                        capabilities: format!("{:?}", runtime.info().capabilities),
                        policy: self.policy_digest(&execution, &agent, &task.id),
                        worktree: format!(
                            "{}|isolated={}",
                            working_dir.to_string_lossy(),
                            agent.worktree_isolation
                        ),
                    },
                },
            );
            if let Some(clean) = stage.redacted {
                inputs = clean;
                (prompt, layout) = build(&inputs);
                combined = prompt.combined();
            }
            gate = stage.gate;
            guard_summary = Some((stage.metrics, stage.review));
        }
        let measured_parts = inputs.parts.clone();
        execution.prompt = combined;
        let prompt_build_ms = optimization::elapsed_ms(prompt_timer);
        let pre_runtime = measuring.then(|| {
            let measured = std::time::Instant::now();
            let breakdown = layout.breakdown(
                &prompt,
                &execution.prompt,
                measured_parts.as_ref().map(BriefParts::layout).as_ref(),
            );
            emitter.announce(
                ExecutionEventKind::OptimizationPromptBuilt,
                "Prompt built".to_owned(),
                optimization::prompt_event_metadata(&breakdown, prompt_build_ms),
            );
            PreRuntime {
                prompt: breakdown,
                context: optimization::context_metrics(execution.context.as_ref()),
                context_engine,
                skills: skill_plan.as_ref().map(|plan| plan.metrics.clone()),
                guardrails: guard_summary.as_ref().map(|(metrics, _)| *metrics),
                context_review: guard_summary.as_ref().map(|(_, review)| review.clone()),
                handoff_bytes: measured_parts.as_ref().map(|b| b.handoff.len() as u64),
                context_build_ms,
                prompt_build_ms,
                instrumentation_ms: optimization::elapsed_ms(Some(measured)).unwrap_or_default(),
            }
        });

        // The ids the process port uses to find the policy itself. Nothing here decides what
        // is permitted: the runtime only passes the scope on.
        let scope = ExecutionScope {
            workspace_id: execution.workspace_id.clone(),
            agent_id: agent.id.clone(),
            execution_id: execution.id.clone(),
            task_id: task.id.clone(),
            // What the runtime's tools can do *as launched for this execution*: file edits only
            // when they were granted above, which the guard checks against the policy again.
            runtime_access: if can_edit {
                ToolAccess {
                    filesystem_write: true,
                    ..ToolAccess::NONE
                }
            } else {
                runtime.info().capabilities.tool_access
            },
            isolated: agent.worktree_isolation,
        };
        let runtime_request = RuntimeRequest {
            model_id: agent.model_id.clone(),
            prompt,
            working_dir,
            scope,
            text_only: false,
            allow_edits: can_edit,
        };
        let mut observation = None;
        let outcome = match gate {
            guardrails::Gate::Deny(failure) => {
                // Not started: nothing ran, so there is nothing to conclude or to repeat.
                emitter.emit_with(
                    ExecutionEventKind::Failed,
                    failure.message.clone(),
                    [("failureKind".to_owned(), failure.kind.as_str().to_owned())].into(),
                );
                execution.fail(failure, now_ms());
                task.status = TaskStatus::Failed;
                None
            }
            guardrails::Gate::Ask(detection) => {
                // Paused before it started, waiting for a person: the answer is what lets it go on.
                let question = RuntimeOutput {
                    text: detection.question.clone(),
                    metadata: BTreeMap::new(),
                    usage: None,
                    quota: None,
                };
                Self::pause_for_input(&emitter, &mut execution, &mut task, question, detection);
                None
            }
            guardrails::Gate::Proceed => {
                let probe = measuring.then(RuntimeProbe::start);
                let result = runtime.execute(&runtime_request, &|stage| {
                    if let Some(probe) = &probe {
                        probe.observe(&stage);
                    }
                    emitter.runtime_event(stage, &runtime_name, &agent.model_id);
                });
                observation = probe.as_ref().map(RuntimeProbe::finish);
                match result {
                    Ok(output) if detect_interaction => {
                        match self.interaction_in(runtime.as_ref(), &output) {
                            Some(interaction) => {
                                Self::pause_for_input(
                                    &emitter,
                                    &mut execution,
                                    &mut task,
                                    output,
                                    interaction,
                                );
                                None
                            }
                            None => Some(Ok(output)),
                        }
                    }
                    other => Some(other),
                }
            }
        };
        if let Some(outcome) = outcome {
            self.conclude(outcome, &emitter, &mut execution, &mut task);
        }
        if let Some(pre) = pre_runtime {
            Self::record_metrics(
                &emitter,
                &mut execution,
                pre,
                observation.as_ref(),
                run_timer,
            );
        }
        if agent.worktree_isolation {
            if shared_worktree.is_some() {
                self.finish_shared_step(&emitter, &execution);
            } else {
                self.finalize_worktree(&emitter, &execution);
            }
        }
        self.attach_audit(&mut execution, emitter.logs.into_inner());

        Ok(ExecutionRecord { task, execution })
    }

    /// What the `optimization_skills_selected` event says.
    fn skills_event_metadata(
        plan: &crate::application::optimization::skills::SkillPlan,
    ) -> BTreeMap<String, String> {
        let m = &plan.metrics;
        BTreeMap::from([
            ("discovered".to_owned(), m.discovered.to_string()),
            ("usable".to_owned(), m.usable.to_string()),
            ("activated".to_owned(), m.activated.join(",")),
            (
                "reasons".to_owned(),
                plan.candidates
                    .iter()
                    .map(|c| format!("{}: {}", c.name, c.reasons.join("; ")))
                    .collect::<Vec<_>>()
                    .join(" | "),
            ),
            ("level2Tokens".to_owned(), m.level2_tokens.to_string()),
            ("level3Tokens".to_owned(), m.level3_tokens.to_string()),
            ("cacheHits".to_owned(), m.cache_hits.to_string()),
            ("cacheMisses".to_owned(), m.cache_misses.to_string()),
            ("tokenSource".to_owned(), "estimated".to_owned()),
        ])
    }

    /// Announces what the Context Engine did, and warns when the prompt is still over its budget.
    fn announce_context_engine(
        emitter: &Emitter<'_>,
        engine: &crate::domain::optimization::ContextEngineMetrics,
    ) {
        emitter.announce(
            ExecutionEventKind::OptimizationContextOptimized,
            "Context optimized".to_owned(),
            BTreeMap::from([
                (
                    "deduplicatedLines".to_owned(),
                    engine.deduplicated_lines.to_string(),
                ),
                (
                    "compressedItems".to_owned(),
                    engine.compressed_items.to_string(),
                ),
                ("omittedItems".to_owned(), engine.omitted_items.to_string()),
                ("savedBytes".to_owned(), engine.saved_bytes().to_string()),
                (
                    "rawEstimatedTokens".to_owned(),
                    engine.raw_estimated_tokens.to_string(),
                ),
                (
                    "finalEstimatedTokens".to_owned(),
                    engine.final_estimated_tokens.to_string(),
                ),
                ("tokenSource".to_owned(), "estimated".to_owned()),
            ]),
        );
        if let Some(over) = engine.over_budget {
            emitter.announce(
                ExecutionEventKind::OptimizationBudgetWarning,
                "Context over budget".to_owned(),
                BTreeMap::from([
                    ("budgetTokens".to_owned(), over.budget_tokens.to_string()),
                    (
                        "estimatedTokens".to_owned(),
                        over.estimated_tokens.to_string(),
                    ),
                    (
                        "requiredTokens".to_owned(),
                        over.required_tokens.to_string(),
                    ),
                ]),
            );
        }
    }

    /// What the `optimization_context_built` event says.
    fn context_event_metadata(
        record: Option<&ContextRecord>,
        build_ms: Option<f64>,
    ) -> BTreeMap<String, String> {
        let mut metadata = BTreeMap::new();
        if let Some(ms) = build_ms {
            metadata.insert("buildMs".to_owned(), ms.to_string());
        }
        match record {
            Some(record) => {
                metadata.insert(
                    "selectedItems".to_owned(),
                    record.selected_items.to_string(),
                );
                metadata.insert("omittedItems".to_owned(), record.omitted_items.to_string());
            }
            None => {
                metadata.insert("harness".to_owned(), "absent".to_owned());
            }
        }
        metadata
    }

    /// Saves the measurements with the execution and announces that they exist. Called once the
    /// runtime has answered; it reads the execution and writes only `execution.optimization`.
    fn record_metrics(
        emitter: &Emitter<'_>,
        execution: &mut Execution,
        pre: PreRuntime,
        observed: Option<&optimization::RuntimeObservation>,
        run_timer: Option<std::time::Instant>,
    ) {
        let measured = std::time::Instant::now();
        let total_ms = optimization::elapsed_ms(run_timer);
        let instrumentation_ms = optimization::elapsed_ms(Some(measured)).unwrap_or_default();
        let mut metrics = optimization::finish_metrics(pre, observed, total_ms, instrumentation_ms);
        let mut metadata = BTreeMap::new();
        if let Some(ms) = total_ms {
            metadata.insert("totalMs".to_owned(), ms.to_string());
        }
        if let Some(usage) = &execution.usage {
            let compared =
                crate::domain::optimization::PromptVersusRuntime::of(&metrics, Some(usage));
            if let Some(tokens) = compared.runtime_input_tokens {
                metadata.insert("runtimeInputTokens".to_owned(), tokens.to_string());
            }
            if let Some(tokens) = compared.runtime_output_tokens {
                metadata.insert("runtimeOutputTokens".to_owned(), tokens.to_string());
            }
        }
        let (exposed, extensions) = optimization::runtime_reported_surface(&execution.metadata);
        metrics.tools.exposed = exposed;
        metrics.extensions = extensions;
        execution.optimization = Some(metrics);
        emitter.announce(
            ExecutionEventKind::OptimizationMetricsRecorded,
            "Metrics recorded".to_owned(),
            metadata,
        );
    }

    /// Whether the answer is a request for a person worth pausing for: the runtime adapter's own
    /// signal first, then the generic detector. Below its confidence bar nothing pauses.
    fn interaction_in(
        &self,
        runtime: &dyn crate::application::runtimes::ModelRuntime,
        output: &RuntimeOutput,
    ) -> Option<InteractionDetection> {
        runtime
            .detect_interaction(output)
            .filter(should_pause)
            .or_else(|| {
                let found = self
                    .interactions
                    .detect(&InteractionSignals { text: &output.text });
                should_pause(&found).then_some(found)
            })
    }

    /// The agent stopped to ask a person: the execution is neither completed nor failed. Its
    /// answer is kept (it is what the person replies to), and no result exists yet.
    fn pause_for_input(
        emitter: &Emitter<'_>,
        execution: &mut Execution,
        task: &mut Task,
        output: RuntimeOutput,
        interaction: InteractionDetection,
    ) {
        let kind = interaction.kind.map_or("clarification", |k| k.as_str());
        emitter.emit_with(
            ExecutionEventKind::InteractionDetected,
            interaction.question.clone(),
            [
                ("interactionKind".to_owned(), kind.to_owned()),
                (
                    "source".to_owned(),
                    format!("{:?}", interaction.source).to_ascii_lowercase(),
                ),
                ("confidence".to_owned(), interaction.confidence.to_string()),
            ]
            .into(),
        );
        execution.wait_for_input(
            output.text,
            interaction,
            output.metadata,
            output.usage,
            output.quota,
            now_ms(),
        );
        // A task waiting for a person is still being worked on.
        task.status = TaskStatus::Running;
    }

    /// Creates the execution's worktree and announces it, or says why it cannot be: no worktree
    /// service, no Git repository, no base branch…
    fn start_worktree(
        &self,
        emitter: &Emitter<'_>,
        execution: &Execution,
        agent_id: &str,
        project_path: &str,
        shared_worktree: Option<&str>,
    ) -> Result<std::path::PathBuf, WorktreeError> {
        let worktrees = self
            .worktrees
            .as_ref()
            .ok_or(WorktreeError::GitUnavailable)?;
        let prepared = match shared_worktree {
            Some(primary) => {
                worktrees.attach(&execution.workspace_id, agent_id, &execution.id, primary)?
            }
            None => worktrees.prepare(
                &execution.workspace_id,
                agent_id,
                &execution.id,
                std::path::Path::new(project_path),
            )?,
        };
        emitter.emit_with(
            ExecutionEventKind::WorktreeCreated,
            format!("Isolated worktree on {}", prepared.worktree.branch_name),
            WorktreeService::event_metadata(&prepared.worktree),
        );
        Ok(prepared.working_dir)
    }

    /// The project's Harness as prompt text, chosen for the task, noting in the execution's log
    /// what became of it. Also says whether the text is a Task Context and how it was chosen.
    fn load_harness(
        &self,
        emitter: &Emitter<'_>,
        project_path: &str,
        task: &str,
    ) -> (Option<String>, bool, Option<ContextRecord>) {
        let note = |text: &str| emitter.logs.borrow_mut().push(text.to_owned());
        let Some(builder) = self.harness.as_ref() else {
            return (None, false, None);
        };
        match builder.build_for_task(project_path, task) {
            TaskContextLoad::Ready(context) => {
                note("Project Harness loaded");
                let record = context.record();
                let task_aware = context.mode == ContextMode::TaskAware;
                if task_aware {
                    note(&format!(
                        "Task context selected from the Harness: {} of {} characters, {} item(s) \
                         included, {} left out",
                        record.selected_context_characters,
                        record.total_harness_characters,
                        record.selected_items,
                        record.omitted_items
                    ));
                    if context.truncated {
                        note(&format!(
                            "Task context trimmed to its budget; omitted: {}",
                            context
                                .omitted
                                .iter()
                                .map(|g| format!("{} {}", g.count, g.area.label()))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                } else {
                    note(&format!(
                        "Task context could not be built ({}); the whole Harness context was used",
                        context.fallback_reason.as_deref().unwrap_or("unknown")
                    ));
                    if context.truncated {
                        note("Project Harness context trimmed to its budget");
                    }
                }
                (Some(context.text), task_aware, Some(record))
            }
            TaskContextLoad::Invalid => {
                note("Project Harness is invalid and was ignored");
                (None, false, None)
            }
            TaskContextLoad::Missing => (None, false, None),
        }
    }

    /// Ends an execution that could not get its worktree: failed, runtime never started. There
    /// is no fallback to the project's checkout.
    fn abandon(
        &self,
        emitter: Emitter<'_>,
        mut execution: Execution,
        mut task: Task,
        error: &WorktreeError,
    ) -> ExecutionRecord {
        let failure = ExecutionFailure {
            kind: error.failure_kind(),
            message: error.to_string(),
            details: matches!(error, WorktreeError::Git(_)).then(|| error.to_string()),
        };
        emitter.emit_with(
            ExecutionEventKind::Failed,
            failure.message.clone(),
            [("failureKind".to_owned(), failure.kind.as_str().to_owned())].into(),
        );
        execution.fail(failure, now_ms());
        task.status = TaskStatus::Failed;
        self.attach_audit(&mut execution, emitter.logs.into_inner());
        ExecutionRecord { task, execution }
    }

    /// A step of a workflow ended: what it left in the shared worktree is saved, and nothing
    /// is merged (the workflow's worktree is decided once, when the run ends).
    fn finish_shared_step(&self, emitter: &Emitter<'_>, execution: &Execution) {
        let Some(worktrees) = &self.worktrees else {
            return;
        };
        // A step waiting for a person keeps what it did in the shared worktree, exactly as a
        // finished one does: the answer starts it again from there.
        let outcome = if execution.status == ExecutionStatus::WaitingForInput {
            RunOutcome::Completed
        } else {
            RunOutcome::from(execution.status)
        };
        if let Some(lease) = worktrees.finish_step(&execution.id, outcome) {
            emitter.emit_with(
                ExecutionEventKind::WorktreeFinalized,
                "Step saved in the workflow's worktree".to_owned(),
                WorktreeService::event_metadata(&lease),
            );
        }
    }

    /// Looks at what the execution left in its worktree and, where the policy and the facts
    /// allow, merges it. Failed and cancelled executions keep their worktree untouched.
    fn finalize_worktree(&self, emitter: &Emitter<'_>, execution: &Execution) {
        let Some(worktrees) = &self.worktrees else {
            return;
        };
        if let Some(worktree) = worktrees.finalize(
            &execution.id,
            RunOutcome::from(execution.status),
            Validation::NotRun,
        ) {
            emitter.emit_with(
                ExecutionEventKind::WorktreeFinalized,
                format!(
                    "Worktree {:?}, merge {:?}",
                    worktree.status, worktree.merge_status
                ),
                WorktreeService::event_metadata(&worktree),
            );
        }
    }

    /// Ends the execution with what the runtime answered: completed, cancelled (the user
    /// stopped its process) or failed.
    fn conclude(
        &self,
        outcome: Result<RuntimeOutput, RuntimeError>,
        emitter: &Emitter<'_>,
        execution: &mut Execution,
        task: &mut Task,
    ) {
        match outcome {
            Ok(output) => {
                emitter.emit_with(
                    ExecutionEventKind::Completed,
                    "Execution completed".to_owned(),
                    output.metadata.clone(),
                );
                execution.complete(
                    output.text,
                    output.metadata,
                    output.usage,
                    output.quota,
                    now_ms(),
                );
                task.status = TaskStatus::Completed;
            }
            Err(_) if self.sessions.user_action(&execution.id).is_some() => {
                // The user stopped the process: the run ended as they asked, nothing failed.
                let action = self.sessions.user_action(&execution.id);
                emitter.emit_with(
                    ExecutionEventKind::Cancelled,
                    "Execution cancelled".to_owned(),
                    [(
                        "by".to_owned(),
                        match action {
                            Some(UserAction::Terminated) => "terminated",
                            _ => "interrupted",
                        }
                        .to_owned(),
                    )]
                    .into(),
                );
                execution.cancel(now_ms());
                task.status = TaskStatus::Cancelled;
            }
            Err(error) => {
                let failure = error.into_failure();
                emitter.emit_with(
                    ExecutionEventKind::Failed,
                    failure.message.clone(),
                    [("failureKind".to_owned(), failure.kind.as_str().to_owned())].into(),
                );
                execution.fail(failure, now_ms());
                task.status = TaskStatus::Failed;
            }
        }
    }

    /// Stores the audit trail of the execution (what the process port decided, in order) on its
    /// record, and mentions each decision in its log.
    fn attach_audit(&self, execution: &mut Execution, mut logs: Vec<String>) {
        execution.permission_events = self.audit.take(&execution.id);
        logs.extend(execution.permission_events.iter().map(|event| {
            format!(
                "Permission {:?}: {} ({:?})",
                event.decision, event.target, event.action
            )
        }));
        execution.logs = logs;
    }
}

/// Records each step in the execution's log and announces it to the observer, stamped with
/// the ids a listener needs to tell executions apart.
struct Emitter<'a> {
    observer: &'a dyn ExecutionObserver,
    execution_id: String,
    workspace_id: String,
    task_id: String,
    agent_id: String,
    logs: RefCell<Vec<String>>,
}

impl Emitter<'_> {
    /// Announces what a runtime reported. Answer text is streamed to the listener only: it is
    /// not logged, because the complete answer is recorded as the result. The names the UI needs
    /// to word a step in the user's language travel in `metadata`.
    fn runtime_event(&self, event: RuntimeEvent, runtime_name: &str, model_id: &str) {
        let named = |key: &str, value: &str| -> BTreeMap<String, String> {
            [(key.to_owned(), value.to_owned())].into()
        };
        match event {
            RuntimeEvent::Starting => self.emit_with(
                ExecutionEventKind::StartingRuntime,
                format!("Starting {runtime_name}"),
                named("runtime", runtime_name),
            ),
            RuntimeEvent::Sending => self.emit(
                ExecutionEventKind::SendingPrompt,
                "Sending prompt".to_owned(),
            ),
            RuntimeEvent::Waiting => self.emit_with(
                ExecutionEventKind::WaitingForModel,
                format!("Waiting for {model_id}"),
                named("model", model_id),
            ),
            RuntimeEvent::Output(text) => {
                self.announce(ExecutionEventKind::OutputChunk, text, BTreeMap::new());
            }
            RuntimeEvent::ToolStarted(tool) => self.emit_with(
                ExecutionEventKind::ToolStarted,
                format!("Using {tool}"),
                named("tool", &tool),
            ),
            RuntimeEvent::ToolCompleted(tool) => self.emit_with(
                ExecutionEventKind::ToolCompleted,
                format!("Finished {tool}"),
                named("tool", &tool),
            ),
        }
    }

    /// A line in the execution's log, with no event.
    fn log(&self, message: &str) {
        self.logs.borrow_mut().push(message.to_owned());
    }

    fn emit(&self, kind: ExecutionEventKind, message: String) {
        self.emit_with(kind, message, BTreeMap::new());
    }

    fn emit_with(
        &self,
        kind: ExecutionEventKind,
        message: String,
        metadata: BTreeMap<String, String>,
    ) {
        self.logs.borrow_mut().push(message.clone());
        self.announce(kind, message, metadata);
    }

    fn announce(
        &self,
        kind: ExecutionEventKind,
        message: String,
        metadata: BTreeMap<String, String>,
    ) {
        self.observer.on_event(&ExecutionEvent {
            execution_id: self.execution_id.clone(),
            workspace_id: self.workspace_id.clone(),
            task_id: self.task_id.clone(),
            agent_id: self.agent_id.clone(),
            kind,
            message,
            timestamp: now_ms(),
            metadata,
        });
    }
}

#[cfg(test)]
mod tests;
