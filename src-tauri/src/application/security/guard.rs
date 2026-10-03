//! [`GuardedProcessRunner`]: the one place a process is allowed to start.
//!
//! Everything in the core that wants a process — runtime adapters probing or launching their CLI,
//! and (later) commands an agent asks for — reaches the OS only through this wrapper of the real
//! [`ProcessRunner`]. For each [`ProcessSpec`] it:
//!
//! 1. looks up the workspace, agent and policy from the ids in the spec's context (never from
//!    anything the webview sent),
//! 2. evaluates the request ([`super::evaluator`]),
//! 3. records the decision in the audit log,
//! 4. for "requires approval", parks the execution until the *user* answers through the
//!    approval UI (or time runs out, which rejects),
//! 5. only then passes the unchanged program + arguments to the inner runner, through the
//!    [`SandboxProvider`] hook.
//!
//! A denied or unapproved process is never spawned: the inner runner is not called.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::approvals::{Answer, ApprovalBroker, PendingApproval};
use super::audit::AuditLog;
use super::evaluator::{
    evaluate_agent_request, evaluate_probe, evaluate_runtime_launch, Evaluation,
};
use super::sandbox::SandboxProvider;
use super::service::{PolicyResolver, ResolvedScope};
use crate::application::process::{
    ExecutionScope, ProcessContext, ProcessError, ProcessEvent, ProcessOutput, ProcessRunner,
    ProcessSpec,
};
use crate::application::support::now_ms;
use crate::domain::security::{
    DecisionSource, PermissionAction, PermissionDecision, PermissionEvent, PermissionOutcome,
    Reason, SecurityPolicy,
};

/// How long a command waits for the user before it counts as rejected.
pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub struct GuardedProcessRunner {
    inner: Arc<dyn ProcessRunner>,
    resolver: Arc<dyn PolicyResolver>,
    broker: Arc<ApprovalBroker>,
    audit: Arc<AuditLog>,
    sandbox: Arc<dyn SandboxProvider>,
    /// Programs of the runtimes Atlas knows: the only ones it starts on its own behalf.
    known_programs: Vec<String>,
    approval_timeout: Duration,
}

impl GuardedProcessRunner {
    pub fn new(
        inner: Arc<dyn ProcessRunner>,
        resolver: Arc<dyn PolicyResolver>,
        broker: Arc<ApprovalBroker>,
        audit: Arc<AuditLog>,
        sandbox: Arc<dyn SandboxProvider>,
        known_programs: Vec<String>,
    ) -> Self {
        Self {
            inner,
            resolver,
            broker,
            audit,
            sandbox,
            known_programs,
            approval_timeout: APPROVAL_TIMEOUT,
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_approval_timeout(mut self, timeout: Duration) -> Self {
        self.approval_timeout = timeout;
        self
    }

    fn event(
        scope: &ExecutionScope,
        spec: &ProcessSpec,
        action: PermissionAction,
        decision: PermissionOutcome,
        source: DecisionSource,
        reason: Option<Reason>,
    ) -> PermissionEvent {
        PermissionEvent {
            timestamp: now_ms(),
            execution_id: scope.execution_id.clone(),
            workspace_id: scope.workspace_id.clone(),
            task_id: scope.task_id.clone(),
            agent_id: scope.agent_id.clone(),
            action,
            target: display_command(spec),
            cwd: spec.cwd.as_ref().map(|p| p.to_string_lossy().into_owned()),
            decision,
            source,
            reason,
            approval_id: None,
            notes: Vec::new(),
        }
    }

    fn record_denied(
        &self,
        scope: &ExecutionScope,
        spec: &ProcessSpec,
        action: PermissionAction,
        reason: Reason,
    ) -> ProcessError {
        self.audit.record(Self::event(
            scope,
            spec,
            action,
            PermissionOutcome::Denied,
            DecisionSource::Policy,
            Some(reason),
        ));
        ProcessError::PermissionDenied(reason)
    }

    /// Passes an approved request to the OS (through the sandbox hook), unchanged.
    fn start(
        &self,
        spec: &ProcessSpec,
        policy: &SecurityPolicy,
        scope: &ExecutionScope,
        action: PermissionAction,
        on_event: &dyn Fn(ProcessEvent),
    ) -> Result<ProcessOutput, ProcessError> {
        match self.sandbox.prepare(spec.clone(), policy) {
            Ok(prepared) => self.inner.run(&prepared, on_event),
            Err(_) => Err(self.record_denied(scope, spec, action, Reason::SandboxRefused)),
        }
    }

    fn resolve(
        &self,
        scope: &ExecutionScope,
        spec: &ProcessSpec,
        action: PermissionAction,
    ) -> Result<ResolvedScope, ProcessError> {
        self.resolver
            .resolve(scope)
            .map_err(|reason| self.record_denied(scope, spec, action, reason))
    }

    fn run_runtime(
        &self,
        scope: &ExecutionScope,
        spec: &ProcessSpec,
        on_event: &dyn Fn(ProcessEvent),
    ) -> Result<ProcessOutput, ProcessError> {
        let action = PermissionAction::LaunchRuntime;
        let resolved = self.resolve(scope, spec, action)?;
        let evaluation =
            evaluate_runtime_launch(&resolved.project_root, spec, &self.known_programs);
        if let Some(reason) = refusal(evaluation) {
            return Err(self.record_denied(scope, spec, action, reason));
        }
        // What the runtime's own tools can do beyond the policy is recorded, not hidden: Atlas
        // cannot narrow it from outside the process.
        let mut allowed = Self::event(
            scope,
            spec,
            action,
            PermissionOutcome::Allowed,
            DecisionSource::Policy,
            None,
        );
        allowed.notes = scope
            .runtime_access
            .exceeding(&resolved.agent_policy)
            .into_iter()
            .map(|capability| format!("runtime_exceeds_policy:{capability}"))
            .collect();
        self.audit.record(allowed);
        self.start(spec, &resolved.policy, scope, action, on_event)
    }

    fn run_agent_request(
        &self,
        scope: &ExecutionScope,
        spec: &ProcessSpec,
        on_event: &dyn Fn(ProcessEvent),
    ) -> Result<ProcessOutput, ProcessError> {
        let action = PermissionAction::RunProcess;
        let resolved = self.resolve(scope, spec, action)?;
        let evaluation = evaluate_agent_request(&resolved.policy, &resolved.project_root, spec);
        let policy = match evaluation.decision {
            PermissionDecision::Denied => {
                let reason = evaluation.reason.unwrap_or(Reason::ProcessesDenied);
                return Err(self.record_denied(scope, spec, action, reason));
            }
            PermissionDecision::Allowed => {
                self.audit.record(Self::event(
                    scope,
                    spec,
                    action,
                    PermissionOutcome::Allowed,
                    DecisionSource::Policy,
                    None,
                ));
                resolved.policy
            }
            PermissionDecision::RequiresApproval => {
                let reason = evaluation.reason.unwrap_or(Reason::ApprovalPolicy);
                self.await_approval(scope, spec, &resolved, reason)?
            }
        };
        self.start(spec, &policy, scope, action, on_event)
    }

    /// Parks the execution until the user answers. Returns the policy to start under.
    fn await_approval(
        &self,
        scope: &ExecutionScope,
        spec: &ProcessSpec,
        resolved: &ResolvedScope,
        reason: Reason,
    ) -> Result<SecurityPolicy, ProcessError> {
        let action = PermissionAction::RunProcess;
        let pending = self.broker.open(|id| PendingApproval {
            id,
            execution_id: scope.execution_id.clone(),
            workspace_id: scope.workspace_id.clone(),
            task_id: scope.task_id.clone(),
            agent_id: scope.agent_id.clone(),
            agent_name: resolved.agent_name.clone(),
            action,
            executable: spec.program.clone(),
            args: spec.args.clone(),
            cwd: spec.cwd.as_ref().map(|p| p.to_string_lossy().into_owned()),
            reason,
            requested_at: 0,
        });
        let with_approval = |outcome, source, reason| {
            let mut event = Self::event(scope, spec, action, outcome, source, reason);
            event.approval_id = Some(pending.id.clone());
            event
        };
        self.audit.record(with_approval(
            PermissionOutcome::ApprovalRequested,
            DecisionSource::Policy,
            Some(reason),
        ));

        // Nothing starts while this blocks.
        let answer = self.broker.wait(&pending.id, self.approval_timeout);
        let (outcome, source, why) = match answer {
            Answer::Approved => (PermissionOutcome::Approved, DecisionSource::User, None),
            Answer::Rejected => (
                PermissionOutcome::Rejected,
                DecisionSource::User,
                Some(Reason::ApprovalRejected),
            ),
            Answer::TimedOut => (
                PermissionOutcome::Rejected,
                DecisionSource::Timeout,
                Some(Reason::ApprovalTimeout),
            ),
        };
        self.audit.record(with_approval(outcome, source, why));
        if let Some(reason) = why {
            return Err(ProcessError::PermissionDenied(reason));
        }

        // The policy may have changed while the user was deciding; an approval only lifts
        // "requires approval", never a denial.
        let now = self.resolve(scope, spec, action)?;
        let evaluation = evaluate_agent_request(&now.policy, &now.project_root, spec);
        if evaluation.decision == PermissionDecision::Denied {
            let reason = evaluation.reason.unwrap_or(Reason::ProcessesDenied);
            return Err(self.record_denied(scope, spec, action, reason));
        }
        Ok(now.policy)
    }
}

fn refusal(evaluation: Evaluation) -> Option<Reason> {
    (evaluation.decision != PermissionDecision::Allowed)
        .then(|| evaluation.reason.unwrap_or(Reason::UnknownProgram))
}

/// The command as one line of text, for people to read. Never executed, never parsed.
pub fn display_command(spec: &ProcessSpec) -> String {
    let quote = |arg: &str| {
        if arg.is_empty() || arg.contains(char::is_whitespace) {
            format!("\"{arg}\"")
        } else {
            arg.to_owned()
        }
    };
    std::iter::once(spec.program.as_str())
        .chain(spec.args.iter().map(String::as_str))
        .map(quote)
        .collect::<Vec<_>>()
        .join(" ")
}

impl ProcessRunner for GuardedProcessRunner {
    fn locate(&self, program: &str) -> Option<PathBuf> {
        // Looking a program up starts nothing.
        self.inner.locate(program)
    }

    fn run(
        &self,
        spec: &ProcessSpec,
        on_event: &dyn Fn(ProcessEvent),
    ) -> Result<ProcessOutput, ProcessError> {
        match &spec.context {
            ProcessContext::Probe => match refusal(evaluate_probe(spec, &self.known_programs)) {
                Some(reason) => Err(ProcessError::PermissionDenied(reason)),
                None => self.inner.run(spec, on_event),
            },
            ProcessContext::Runtime(scope) => self.run_runtime(scope, spec, on_event),
            ProcessContext::AgentRequested(scope) => self.run_agent_request(scope, spec, on_event),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Mutex;
    use std::thread;

    use super::*;
    use crate::application::config::memory::MemoryStore;
    use crate::application::config::ConfigRepository;
    use crate::application::process::fake::{ok, FakeProcessRunner};
    use crate::application::security::sandbox::NoSandbox;
    use crate::application::security::service::SecurityService;
    use crate::application::security::testutil::TempDir;
    use crate::application::security::PermissionSink;
    use crate::domain::agent::Agent;
    use crate::domain::security::{Permission, ToolAccess};
    use crate::domain::workspace::{Workspace, WorkspaceLayout};

    #[derive(Default)]
    struct Collector(Mutex<Vec<PermissionEvent>>);

    impl PermissionSink for Collector {
        fn on_permission_event(&self, event: &PermissionEvent) {
            self.0.lock().unwrap().push(event.clone());
        }
    }

    struct World {
        _dir: TempDir,
        /// `ws-a` -> `<tmp>/a`, `ws-b` -> `<tmp>/b`.
        a: PathBuf,
        b: PathBuf,
        config: Arc<ConfigRepository>,
        broker: Arc<ApprovalBroker>,
        audit: Arc<AuditLog>,
        sink: Arc<Collector>,
        fake: Arc<FakeProcessRunner>,
        guard: Arc<GuardedProcessRunner>,
    }

    fn agent(id: &str, profile: Option<&str>) -> Agent {
        Agent {
            id: id.to_owned(),
            name: format!("Agent {id}"),
            personality_id: "p".to_owned(),
            runtime_id: "claude".to_owned(),
            model_id: "m".to_owned(),
            instructions: String::new(),
            permission_profile_id: profile.map(str::to_owned),
            worktree_isolation: false,
            created_at: 1,
        }
    }

    fn workspace(id: &str, path: &Path, security: SecurityPolicy) -> Workspace {
        Workspace {
            id: id.to_owned(),
            name: id.to_owned(),
            project_path: path.to_string_lossy().into_owned(),
            description: None,
            created_at: 1,
            updated_at: 1,
            layout: WorkspaceLayout {
                rows: 2,
                columns: 2,
                agent_placements: vec![],
            },
            security,
        }
    }

    use std::path::Path;

    fn world_with(ws_b_policy: SecurityPolicy) -> World {
        let dir = TempDir::new("guard");
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::create_dir_all(a.join("src")).unwrap();
        fs::create_dir_all(&b).unwrap();
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        config
            .modify(|c| {
                c.workspaces
                    .push(workspace("ws-a", &a, SecurityPolicy::developer()));
                c.workspaces.push(workspace("ws-b", &b, ws_b_policy));
                c.agents.push(agent("dev", Some("developer")));
                c.agents.push(agent("reader", Some("read_only")));
                c.agents.push(agent("unset", None));
                c.agents.push(agent("bogus", Some("root")));
                Ok(())
            })
            .unwrap();
        let broker = Arc::new(ApprovalBroker::new());
        let sink = Arc::new(Collector::default());
        let audit = Arc::new(AuditLog::with_sink(sink.clone()));
        let fake = Arc::new(FakeProcessRunner::new(
            &[
                "claude", "opencode", "npm", "git", "make", "curl", "sh", "cmd",
            ],
            |_| ok("done"),
        ));
        let guard = Arc::new(
            GuardedProcessRunner::new(
                fake.clone(),
                Arc::new(SecurityService::new(config.clone())),
                broker.clone(),
                audit.clone(),
                Arc::new(NoSandbox),
                vec!["claude".to_owned(), "opencode".to_owned()],
            )
            .with_approval_timeout(Duration::from_secs(10)),
        );
        World {
            _dir: dir,
            a,
            b,
            config,
            broker,
            audit,
            sink,
            fake,
            guard,
        }
    }

    fn world() -> World {
        world_with(SecurityPolicy::developer())
    }

    fn scope(workspace: &str, agent: &str, access: ToolAccess) -> ExecutionScope {
        ExecutionScope {
            workspace_id: workspace.to_owned(),
            agent_id: agent.to_owned(),
            execution_id: format!("exec-{workspace}-{agent}"),
            task_id: "task-1".to_owned(),
            runtime_access: access,
            isolated: false,
        }
    }

    const OPEN: ToolAccess = ToolAccess {
        filesystem_write: true,
        process_execution: true,
        network: true,
    };

    fn agent_request(
        scope: ExecutionScope,
        cwd: &Path,
        program: &str,
        args: &[&str],
    ) -> ProcessSpec {
        ProcessSpec {
            program: program.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            stdin: None,
            cwd: Some(cwd.to_path_buf()),
            env: Vec::new(),
            timeout: Duration::from_secs(5),
            context: ProcessContext::AgentRequested(scope),
            terminal: None,
        }
    }

    fn runtime_launch(scope: ExecutionScope, cwd: &Path) -> ProcessSpec {
        ProcessSpec {
            context: ProcessContext::Runtime(scope),
            ..agent_request(ExecutionScope::for_tests(), cwd, "claude", &["-p"])
        }
    }

    fn run(world: &World, spec: &ProcessSpec) -> Result<ProcessOutput, ProcessError> {
        world.guard.run(spec, &|_| {})
    }

    fn events(world: &World, execution_id: &str) -> Vec<(PermissionOutcome, Option<Reason>)> {
        world
            .sink
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.execution_id == execution_id)
            .map(|e| (e.decision, e.reason))
            .collect()
    }

    fn started(world: &World) -> usize {
        world.fake.calls.lock().unwrap().len()
    }

    fn wait_for_pending(world: &World) -> PendingApproval {
        for _ in 0..200 {
            if let Some(pending) = world.broker.pending(None).into_iter().next() {
                return pending;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("no approval was requested");
    }

    // ── runtime launches ────────────────────────────────────────────────────────────────

    #[test]
    fn a_runtime_launch_in_the_project_starts_and_is_audited_with_its_ids() {
        let w = world();
        let s = scope("ws-a", "reader", ToolAccess::NONE);

        let result = run(&w, &runtime_launch(s.clone(), &w.a));

        assert!(result.is_ok());
        assert_eq!(started(&w), 1);
        let recorded = w.audit.take(&s.execution_id);
        assert_eq!(recorded.len(), 1);
        let event = &recorded[0];
        assert_eq!(event.decision, PermissionOutcome::Allowed);
        assert_eq!(event.action, PermissionAction::LaunchRuntime);
        assert_eq!(event.source, DecisionSource::Policy);
        assert_eq!(
            (
                &event.workspace_id[..],
                &event.agent_id[..],
                &event.execution_id[..],
                &event.task_id[..]
            ),
            ("ws-a", "reader", "exec-ws-a-reader", "task-1")
        );
    }

    #[test]
    fn a_runtime_launch_outside_the_project_never_starts() {
        let w = world();
        let s = scope("ws-a", "reader", ToolAccess::NONE);

        // The working directory of workspace B is not workspace A's project.
        let result = run(&w, &runtime_launch(s.clone(), &w.b));

        assert_eq!(
            result.unwrap_err(),
            ProcessError::PermissionDenied(Reason::OutsideProject)
        );
        assert_eq!(started(&w), 0);
        assert_eq!(
            events(&w, &s.execution_id),
            [(PermissionOutcome::Denied, Some(Reason::OutsideProject))]
        );
    }

    #[test]
    fn an_unknown_workspace_or_agent_in_the_scope_is_denied() {
        let w = world();
        for (ws, ag) in [("ws-zzz", "dev"), ("ws-a", "ghost")] {
            let result = run(&w, &runtime_launch(scope(ws, ag, ToolAccess::NONE), &w.a));
            assert_eq!(
                result.unwrap_err(),
                ProcessError::PermissionDenied(Reason::UnknownScope)
            );
        }
        assert_eq!(started(&w), 0);
    }

    #[test]
    fn a_runtime_that_can_do_more_than_the_policy_is_launched_but_the_excess_is_recorded() {
        let w = world();
        let s = scope("ws-a", "reader", OPEN);

        run(&w, &runtime_launch(s.clone(), &w.a)).unwrap();

        let recorded = w.audit.take(&s.execution_id);
        assert_eq!(
            recorded[0].notes,
            [
                "runtime_exceeds_policy:filesystem_write",
                "runtime_exceeds_policy:process_execution",
                "runtime_exceeds_policy:network"
            ]
        );
        // The same runtime under a developer profile exceeds only what even developers lack.
        let s2 = scope("ws-a", "dev", OPEN);
        run(&w, &runtime_launch(s2.clone(), &w.a)).unwrap();
        assert_eq!(
            w.audit.take(&s2.execution_id)[0].notes,
            ["runtime_exceeds_policy:network"]
        );
    }

    #[test]
    fn runtimes_can_only_launch_programs_atlas_knows() {
        let w = world();
        let mut spec = runtime_launch(scope("ws-a", "reader", ToolAccess::NONE), &w.a);
        spec.program = "curl".to_owned();

        assert_eq!(
            run(&w, &spec).unwrap_err(),
            ProcessError::PermissionDenied(Reason::UnknownProgram)
        );
        assert_eq!(started(&w), 0);
    }

    // ── probes ──────────────────────────────────────────────────────────────────────────

    #[test]
    fn probes_may_ask_a_known_runtime_about_itself_and_nothing_else() {
        let w = world();

        assert!(run(
            &w,
            &ProcessSpec::probe("claude", &["--version"], Duration::from_secs(1))
        )
        .is_ok());
        for program in ["curl", "sh", "npm"] {
            let result = run(
                &w,
                &ProcessSpec::probe(program, &["--version"], Duration::from_secs(1)),
            );
            assert!(
                matches!(result, Err(ProcessError::PermissionDenied(_))),
                "{program}"
            );
        }
        assert_eq!(started(&w), 1);
    }

    // ── agent-requested processes ───────────────────────────────────────────────────────

    #[test]
    fn an_allowed_command_starts_and_is_audited() {
        let w = world();
        let s = scope("ws-a", "dev", ToolAccess::NONE);
        // Runtime limits: this runtime could do everything, so the profile is what counts.
        let s = ExecutionScope {
            runtime_access: OPEN,
            ..s
        };

        let result = run(&w, &agent_request(s.clone(), &w.a, "npm", &["test"]));

        assert!(result.is_ok());
        assert_eq!(started(&w), 1);
        assert_eq!(
            events(&w, &s.execution_id),
            [(PermissionOutcome::Allowed, None)]
        );
    }

    #[test]
    fn a_denied_command_never_starts() {
        let w = world();
        let s = scope("ws-a", "dev", OPEN);

        for (program, args) in [
            ("curl", &["https://example.com"][..]),
            ("sh", &["-c", "echo hi"]),
            ("cmd", &["/c", "dir"]),
        ] {
            let result = run(&w, &agent_request(s.clone(), &w.a, program, args));
            assert!(
                matches!(result, Err(ProcessError::PermissionDenied(_))),
                "{program}"
            );
        }
        assert_eq!(started(&w), 0);
        assert!(events(&w, &s.execution_id)
            .iter()
            .all(|(outcome, _)| *outcome == PermissionOutcome::Denied));
    }

    #[test]
    fn arguments_reach_the_process_separate_and_unchanged() {
        let w = world();
        let s = scope("ws-a", "dev", OPEN);
        let hostile = [
            "; rm -rf ..",
            "& whoami",
            "$(touch pwned)",
            "`id`",
            "a && b",
            "| tee x",
        ];

        run(&w, &agent_request(s, &w.a, "npm", &hostile)).unwrap();

        let calls = w.fake.calls.lock().unwrap();
        assert_eq!(calls[0].program, "npm");
        assert_eq!(calls[0].args, hostile);
    }

    #[test]
    fn a_command_that_requires_approval_does_not_start_until_the_user_approves() {
        let w = world();
        let s = scope("ws-a", "dev", OPEN);
        let spec = agent_request(s.clone(), &w.a, "make", &["all"]);

        let running = {
            let guard = w.guard.clone();
            thread::spawn(move || guard.run(&spec, &|_| {}))
        };
        let pending = wait_for_pending(&w);
        thread::sleep(Duration::from_millis(100));

        // Waiting is a real state: the request is listed, the audit says so, nothing started.
        assert_eq!(started(&w), 0);
        assert!(!running.is_finished());
        assert_eq!(pending.executable, "make");
        assert_eq!(pending.args, ["all"]);
        assert_eq!(pending.cwd.as_deref(), Some(&*w.a.to_string_lossy()));
        assert_eq!(pending.reason, Reason::NotInAllowedList);
        assert_eq!(pending.agent_name, "Agent dev");
        assert_eq!(
            events(&w, &s.execution_id),
            [(
                PermissionOutcome::ApprovalRequested,
                Some(Reason::NotInAllowedList)
            )]
        );

        w.broker.resolve(&pending.id, true).unwrap();

        assert!(running.join().unwrap().is_ok());
        assert_eq!(started(&w), 1);
        assert_eq!(
            events(&w, &s.execution_id),
            [
                (
                    PermissionOutcome::ApprovalRequested,
                    Some(Reason::NotInAllowedList)
                ),
                (PermissionOutcome::Approved, None)
            ]
        );
        let recorded = w.audit.take(&s.execution_id);
        assert_eq!(recorded[0].approval_id, recorded[1].approval_id);
        assert_eq!(recorded[1].source, DecisionSource::User);
    }

    #[test]
    fn a_rejected_command_never_starts() {
        let w = world();
        let s = scope("ws-a", "dev", OPEN);
        let spec = agent_request(s.clone(), &w.a, "make", &["all"]);
        let running = {
            let guard = w.guard.clone();
            thread::spawn(move || guard.run(&spec, &|_| {}))
        };

        w.broker.resolve(&wait_for_pending(&w).id, false).unwrap();

        assert_eq!(
            running.join().unwrap().unwrap_err(),
            ProcessError::PermissionDenied(Reason::ApprovalRejected)
        );
        assert_eq!(started(&w), 0);
        assert_eq!(
            events(&w, &s.execution_id),
            [
                (
                    PermissionOutcome::ApprovalRequested,
                    Some(Reason::NotInAllowedList)
                ),
                (PermissionOutcome::Rejected, Some(Reason::ApprovalRejected))
            ]
        );
    }

    #[test]
    fn an_unanswered_approval_times_out_as_a_rejection() {
        let w = world();
        let guard = Arc::new(
            GuardedProcessRunner::new(
                w.fake.clone(),
                Arc::new(SecurityService::new(w.config.clone())),
                w.broker.clone(),
                w.audit.clone(),
                Arc::new(NoSandbox),
                vec![],
            )
            .with_approval_timeout(Duration::from_millis(50)),
        );
        let s = scope("ws-a", "dev", OPEN);

        let result = guard.run(&agent_request(s.clone(), &w.a, "make", &[]), &|_| {});

        assert_eq!(
            result.unwrap_err(),
            ProcessError::PermissionDenied(Reason::ApprovalTimeout)
        );
        assert_eq!(started(&w), 0);
        let recorded = w.audit.take(&s.execution_id);
        assert_eq!(recorded[1].source, DecisionSource::Timeout);
        assert_eq!(w.broker.pending(None).len(), 0);
    }

    #[test]
    fn approving_cannot_lift_a_denial() {
        let w = world();
        let s = scope("ws-a", "dev", OPEN);
        // `curl` is denied (network is off), so there is nothing to approve.
        let result = run(
            &w,
            &agent_request(s.clone(), &w.a, "curl", &["https://x.test"]),
        );

        assert_eq!(
            result.unwrap_err(),
            ProcessError::PermissionDenied(Reason::NetworkAccess)
        );
        assert_eq!(w.broker.pending(None).len(), 0);
        assert_eq!(started(&w), 0);
    }

    // ── layering ────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_read_only_agent_cannot_run_commands_even_if_the_workspace_allows_them() {
        let w = world();
        let s = scope("ws-a", "reader", OPEN);

        let result = run(&w, &agent_request(s, &w.a, "npm", &["test"]));

        assert_eq!(
            result.unwrap_err(),
            ProcessError::PermissionDenied(Reason::ProcessesDenied)
        );
        assert_eq!(started(&w), 0);
    }

    #[test]
    fn agents_without_a_valid_profile_get_the_most_restrictive_one() {
        let w = world();
        for id in ["unset", "bogus"] {
            let result = run(
                &w,
                &agent_request(scope("ws-a", id, OPEN), &w.a, "npm", &["test"]),
            );
            assert_eq!(
                result.unwrap_err(),
                ProcessError::PermissionDenied(Reason::ProcessesDenied),
                "{id}"
            );
        }
    }

    #[test]
    fn a_developer_agent_cannot_exceed_a_read_only_workspace() {
        let w = world_with(SecurityPolicy::read_only());

        let in_a = run(
            &w,
            &agent_request(scope("ws-a", "dev", OPEN), &w.a, "npm", &["test"]),
        );
        let in_b = run(
            &w,
            &agent_request(scope("ws-b", "dev", OPEN), &w.b, "npm", &["test"]),
        );

        // Same agent, same profile: the workspace decides.
        assert!(in_a.is_ok());
        assert_eq!(
            in_b.unwrap_err(),
            ProcessError::PermissionDenied(Reason::ProcessesDenied)
        );
    }

    #[test]
    fn a_workspace_policy_cannot_exceed_the_global_maximum() {
        let mut everything = SecurityPolicy::developer();
        everything.network.mode = Permission::Allowed;
        everything.git.destructive = Permission::Allowed;
        let w = world_with(everything);
        let s = scope("ws-b", "dev", OPEN);

        // Hand-edited config says "allowed", Atlas's maximum says "ask".
        let guard = Arc::new(
            GuardedProcessRunner::new(
                w.fake.clone(),
                Arc::new(SecurityService::new(w.config.clone())),
                w.broker.clone(),
                w.audit.clone(),
                Arc::new(NoSandbox),
                vec![],
            )
            .with_approval_timeout(Duration::from_millis(50)),
        );
        let reset = guard.run(
            &agent_request(s.clone(), &w.b, "git", &["reset", "--hard"]),
            &|_| {},
        );
        let curl = guard.run(
            &agent_request(s, &w.b, "curl", &["https://x.test"]),
            &|_| {},
        );

        // Destructive Git asked (and timed out) instead of running; the developer profile keeps
        // network off whatever the workspace says. Neither ran.
        assert_eq!(
            reset.unwrap_err(),
            ProcessError::PermissionDenied(Reason::ApprovalTimeout)
        );
        assert_eq!(
            curl.unwrap_err(),
            ProcessError::PermissionDenied(Reason::NetworkAccess)
        );
        assert_eq!(started(&w), 0);
    }

    #[test]
    fn a_runtime_that_cannot_run_commands_limits_the_agent_running_through_it() {
        let w = world();

        let limited = run(
            &w,
            &agent_request(
                scope("ws-a", "dev", ToolAccess::NONE),
                &w.a,
                "npm",
                &["test"],
            ),
        );
        let open = run(
            &w,
            &agent_request(scope("ws-a", "dev", OPEN), &w.a, "npm", &["test"]),
        );

        assert_eq!(
            limited.unwrap_err(),
            ProcessError::PermissionDenied(Reason::ProcessesDenied)
        );
        assert!(open.is_ok());
    }

    #[test]
    fn an_agent_cannot_use_another_workspaces_folder_or_permissions() {
        let w = world_with(SecurityPolicy::read_only());
        let a = scope("ws-a", "dev", OPEN);
        let b = scope("ws-b", "dev", OPEN);

        // Workspace A's scope reaching into B's folder is outside A's project.
        assert_eq!(
            run(&w, &agent_request(a.clone(), &w.b, "npm", &["test"])).unwrap_err(),
            ProcessError::PermissionDenied(Reason::OutsideProject)
        );
        // And B's scope does not inherit A's developer permissions in A's folder.
        assert_eq!(
            run(&w, &agent_request(b, &w.a, "npm", &["test"])).unwrap_err(),
            ProcessError::PermissionDenied(Reason::OutsideProject)
        );
        // Each is fine at home.
        assert!(run(&w, &agent_request(a, &w.a, "npm", &["test"])).is_ok());
    }

    // ── audit and sink ──────────────────────────────────────────────────────────────────

    #[test]
    fn decisions_are_announced_live_in_order_and_kept_per_execution() {
        let w = world();
        let s1 = scope("ws-a", "dev", OPEN);
        let s2 = scope("ws-a", "reader", OPEN);

        run(&w, &agent_request(s1.clone(), &w.a, "npm", &["test"])).unwrap();
        run(&w, &agent_request(s2.clone(), &w.a, "npm", &["test"])).unwrap_err();

        let live: Vec<_> = w
            .sink
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|e| (e.execution_id.clone(), e.decision))
            .collect();
        assert_eq!(
            live,
            [
                (s1.execution_id.clone(), PermissionOutcome::Allowed),
                (s2.execution_id.clone(), PermissionOutcome::Denied)
            ]
        );
        assert_eq!(w.audit.take(&s1.execution_id).len(), 1);
        assert_eq!(w.audit.take(&s2.execution_id).len(), 1);
        assert_eq!(w.audit.take(&s1.execution_id).len(), 0);
    }

    // ── sandbox seam ────────────────────────────────────────────────────────────────────

    struct Wrapping;

    impl SandboxProvider for Wrapping {
        fn prepare(
            &self,
            mut spec: ProcessSpec,
            _policy: &SecurityPolicy,
        ) -> Result<ProcessSpec, String> {
            spec.args.insert(0, "--sandboxed".to_owned());
            Ok(spec)
        }
    }

    struct Refusing;

    impl SandboxProvider for Refusing {
        fn prepare(
            &self,
            _spec: ProcessSpec,
            _policy: &SecurityPolicy,
        ) -> Result<ProcessSpec, String> {
            Err("cannot confine".to_owned())
        }
    }

    #[test]
    fn a_sandbox_provider_sees_every_approved_process_and_can_refuse_it() {
        let w = world();
        let build = |sandbox: Arc<dyn SandboxProvider>| {
            GuardedProcessRunner::new(
                w.fake.clone(),
                Arc::new(SecurityService::new(w.config.clone())),
                w.broker.clone(),
                w.audit.clone(),
                sandbox,
                vec!["claude".to_owned()],
            )
        };
        let s = scope("ws-a", "dev", OPEN);
        let spec = agent_request(s.clone(), &w.a, "npm", &["test"]);

        build(Arc::new(Wrapping)).run(&spec, &|_| {}).unwrap();
        assert_eq!(
            w.fake.calls.lock().unwrap()[0].args,
            ["--sandboxed", "test"]
        );

        let refused = build(Arc::new(Refusing)).run(&spec, &|_| {});
        assert_eq!(
            refused.unwrap_err(),
            ProcessError::PermissionDenied(Reason::SandboxRefused)
        );
        assert_eq!(started(&w), 1);
    }

    // ── real processes ──────────────────────────────────────────────────────────────────

    /// Through the real runner: the hostile text is printed as an ordinary argument, so nothing
    /// ran it and the file it tried to create does not exist.
    #[cfg(unix)]
    #[test]
    fn shell_metacharacters_are_not_interpreted_by_the_real_runner() {
        use crate::infrastructure::SystemProcessRunner;

        let w = world();
        let guard = Arc::new(
            GuardedProcessRunner::new(
                Arc::new(SystemProcessRunner::new()),
                Arc::new(SecurityService::new(w.config.clone())),
                w.broker.clone(),
                w.audit.clone(),
                Arc::new(NoSandbox),
                vec![],
            )
            .with_approval_timeout(Duration::from_secs(10)),
        );
        let s = scope("ws-a", "dev", OPEN);
        let marker = w.a.join("pwned");
        let hostile = [
            format!("; touch {}", marker.display()),
            format!("& touch {}", marker.display()),
            format!("$(touch {})", marker.display()),
            format!("`touch {}`", marker.display()),
            format!("&& touch {} #", marker.display()),
        ];
        let spec = agent_request(
            s,
            &w.a,
            "echo",
            &hostile.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        // `echo` is not a listed development command, so it asks first.
        let running = {
            let guard = guard.clone();
            thread::spawn(move || guard.run(&spec, &|_| {}))
        };
        w.broker.resolve(&wait_for_pending(&w).id, true).unwrap();

        let output = running.join().unwrap().unwrap();

        assert_eq!(output.stdout.trim_end(), hostile.join(" "));
        assert!(!marker.exists(), "an argument was run by a shell");
    }

    #[test]
    fn display_text_quotes_arguments_with_spaces_but_is_only_for_people() {
        let spec = ProcessSpec::probe("npm", &["test", "a b", ""], Duration::from_secs(1));

        assert_eq!(display_command(&spec), r#"npm test "a b" """#);
    }
}
