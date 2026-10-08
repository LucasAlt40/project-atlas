//! Phase B through the real execution path: the manifest's hash against what a process port
//! really received, the plan, the budget and the runtime surface (ADR 0026).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

use super::*;
use crate::application::agents::CreateAgentRequest;
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::optimization::FixedFlags;
use crate::application::process::fake::{ok, FakeProcessRunner};
use crate::application::process::ProcessRunner as _;
use crate::application::process::{ExecutionScope, ProcessSpec};
use crate::application::projects::fake::FakeInspector;
use crate::application::prompt::Prompt;
use crate::application::runtimes::{
    AntigravityRuntime, ClaudeRuntime, CodexRuntime, GeminiRuntime, ModelRuntime, OpenCodeRuntime,
    RuntimeError, RuntimeEvent, RuntimeOutput,
};
use crate::application::workspace::WorkspaceInput;
use crate::domain::context::{
    ContextWarning, FigureSource, Observation, Precision, SurfaceControl, SurfaceKind,
};
use crate::domain::execution::ExecutionStatus;
use crate::domain::runtime::SystemPromptChannel;

#[derive(Default)]
struct Quiet(Mutex<Vec<ExecutionEvent>>);

impl ExecutionObserver for Quiet {
    fn on_event(&self, event: &ExecutionEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

/// What the real CLI printed at start-up (shape captured from Claude Code 2.1.285; the plugins
/// and the memory path are the kinds of thing a user's machine adds) and then its answer.
const CLAUDE_OUTPUT: &str = concat!(
    r#"{"type":"system","subtype":"init","tools":["Glob","Grep","Read"],"mcp_servers":[],"slash_commands":[],"skills":[],"plugins":[{"name":"figma","source":"figma@claude-plugins-official"},{"name":"cc-plugin-telemetry","source":"cc-plugin-telemetry@builtin"}],"memory_paths":{"auto":"/home/u/.claude/projects/p/memory/"}}"#,
    "\n",
    r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":5,"num_turns":1,"result":"done\n"}"#,
);

fn sha256_of(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

/// The prompt a process received, whichever way it travelled: on stdin, as the last argument, or
/// as the value of `--prompt=`.
fn received_prompt(spec: &ProcessSpec) -> String {
    if let Some(stdin) = &spec.stdin {
        return stdin.clone();
    }
    let last = spec.args.last().expect("the prompt was an argument");
    last.strip_prefix("--prompt=").unwrap_or(last).to_owned()
}

struct Stack {
    service: ExecutionService,
    agent_id: String,
    workspace_id: String,
}

/// The real `ExecutionService` over the real Claude adapter and a spy process port.
fn claude_stack(spy: &Arc<FakeProcessRunner>, flags: FixedFlags) -> Stack {
    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    let personalities = Arc::new(PersonalityService::new(config.clone()));
    let registry = Arc::new(RuntimeRegistry::new(vec![
        Arc::new(ClaudeRuntime::new(spy.clone())) as Arc<_>,
    ]));
    let agents = Arc::new(AgentService::new(
        config.clone(),
        personalities.clone(),
        registry.clone(),
    ));
    let workspaces = Arc::new(WorkspaceService::new(
        config,
        agents.clone(),
        Arc::new(FakeInspector::with(&[("/atlas", &["Rust"])])),
    ));
    let workspace_id = workspaces
        .create(&WorkspaceInput {
            name: "Project Atlas".to_owned(),
            project_path: "/atlas".to_owned(),
            description: None,
        })
        .unwrap()
        .id;
    let agent_id = agents
        .create(CreateAgentRequest {
            permission_profile_id: None,
            name: "claude agent".to_owned(),
            personality_id: "architect".to_owned(),
            runtime_id: "claude".to_owned(),
            model_id: "sonnet".to_owned(),
            instructions: "Be brief.".to_owned(),
            worktree_isolation: Some(false),
            result_contract: None,
        })
        .unwrap()
        .id;
    let service = ExecutionService::new(
        agents,
        personalities,
        registry,
        workspaces,
        Arc::new(crate::application::security::AuditLog::default()),
    )
    .with_optimization(Arc::new(flags));
    Stack {
        service,
        agent_id,
        workspace_id,
    }
}

fn spy_for_claude() -> Arc<FakeProcessRunner> {
    Arc::new(FakeProcessRunner::new(&["claude"], |_| ok(CLAUDE_OUTPUT)))
}

fn run(stack: &Stack, description: &str) -> ExecutionRecord {
    stack
        .service
        .run_with_id(
            stack.service.next_execution_id(),
            RunAgentRequest {
                task_id: "task-1".to_owned(),
                workspace_id: stack.workspace_id.clone(),
                agent_id: stack.agent_id.clone(),
                description: description.to_owned(),
            },
            &Quiet::default(),
        )
        .unwrap()
}

#[test]
fn the_manifest_hash_is_the_hash_of_the_payload_the_process_port_really_received() {
    let spy = spy_for_claude();
    let stack = claude_stack(&spy, FixedFlags::metrics(true));

    let record = run(&stack, "Explain the module layout, with accents: ação");

    assert_eq!(record.execution.status, ExecutionStatus::Completed);
    let calls = spy.calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "one launch");
    let captured = received_prompt(&calls[0]);
    let manifest = record
        .execution
        .manifest
        .as_ref()
        .expect("every execution that built a prompt has a manifest");
    // The hash is of what the port received, computed here by an independent route.
    assert_eq!(manifest.delivery.prompt_hash, sha256_of(&captured));
    assert_eq!(manifest.delivery.bytes, captured.len());
    assert_eq!(manifest.delivery.chars, captured.chars().count());
    assert!(
        manifest.delivery.chars < manifest.delivery.bytes,
        "accents are one char, two bytes"
    );
    // And it is the prompt the execution records, in the sections the manifest lists.
    assert_eq!(captured, record.execution.prompt);
    assert_eq!(
        manifest.sections.iter().map(|s| s.bytes).sum::<usize>(),
        captured.len()
    );
    assert!(manifest.delivery.delivered);
}

#[test]
fn every_adapter_hands_its_process_exactly_the_payload_it_hashes() {
    let prompt = Prompt {
        system: "SYS ação".to_owned(),
        harness: Some("harness text".to_owned()),
        task_aware: true,
        skills: None,
        rules: None,
        context: "CTX".to_owned(),
        instruction: "Task:\nline one\nline two".to_owned(),
    };
    let request = |model: &str| RuntimeRequest {
        model_id: model.to_owned(),
        prompt: prompt.clone(),
        working_dir: "/atlas".into(),
        scope: ExecutionScope::for_tests(),
        text_only: false,
        allow_edits: false,
        mcp: None,
    };
    let spy = |program: &str| {
        Arc::new(FakeProcessRunner::new(&[program], |spec| {
            match spec.args.first().map(String::as_str) {
                Some("models") => ok("opencode/big-pickle\n"),
                _ => ok(""),
            }
        }))
    };
    let case = |name: &'static str,
                program: &str,
                model: &'static str,
                build: &dyn Fn(Arc<FakeProcessRunner>) -> Box<dyn ModelRuntime>| {
        let runner = spy(program);
        Case {
            name,
            runtime: build(runner.clone()),
            runner,
            model,
        }
    };
    let cases = [
        case("claude", "claude", "sonnet", &|r| {
            Box::new(ClaudeRuntime::new(r))
        }),
        case("codex", "codex", "gpt-x", &|r| {
            Box::new(CodexRuntime::new(r))
        }),
        case("gemini", "gemini", "gemini-x", &|r| {
            Box::new(GeminiRuntime::new(r))
        }),
        case("opencode", "opencode", "opencode/big-pickle", &|r| {
            Box::new(OpenCodeRuntime::new(r))
        }),
        case("antigravity", "agy", "model-x", &|r| {
            Box::new(AntigravityRuntime::new(r))
        }),
    ];

    for case in cases {
        let request = request(case.model);
        // What the manifest would hash, taken before the launch like the service does.
        let planned = case.runtime.delivery(&request).prompt_hash();

        let _ = case.runtime.execute(&request, &|_| {});

        let calls = case.runner.calls.lock().unwrap();
        let launch = calls
            .last()
            .unwrap_or_else(|| panic!("{} launched nothing", case.name));
        assert_eq!(
            planned,
            sha256_of(&received_prompt(launch)),
            "{}",
            case.name
        );
    }
}

struct Case {
    name: &'static str,
    runtime: Box<dyn ModelRuntime>,
    runner: Arc<FakeProcessRunner>,
    model: &'static str,
}

#[test]
fn the_budget_says_the_model_limit_is_unknown_and_the_tokens_are_estimated() {
    let spy = spy_for_claude();
    let stack = claude_stack(&spy, FixedFlags::metrics(true));

    let record = run(&stack, "Explain the module layout");

    let metrics = record.execution.optimization.unwrap();
    let budget = metrics.budget.unwrap();
    // Per runtime + model: nothing states these, so nothing is invented.
    assert_eq!(
        (budget.runtime_id.as_str(), budget.model_id.as_str()),
        ("claude", "sonnet")
    );
    assert_eq!(budget.limits.input.value, None);
    assert_eq!(budget.input.limit.value, None);
    assert_eq!(budget.input.remaining.value, None);
    // The prompt's size is an estimate, labelled with its method.
    assert_eq!(budget.input.used.precision, Precision::Estimated);
    assert_eq!(
        budget.input.used.value,
        Some(metrics.prompt.estimated_tokens)
    );
    let warnings = record.execution.manifest.unwrap().warnings;
    for expected in [
        ContextWarning::ModelLimitUnknown,
        ContextWarning::TokensEstimated,
        ContextWarning::SurfacePartlyUnobserved,
    ] {
        assert!(warnings.contains(&expected), "{expected:?}");
    }
}

#[test]
fn a_configured_limit_is_in_force_only_with_the_engine_and_is_never_the_models() {
    let spy = spy_for_claude();
    let on = claude_stack(&spy, FixedFlags::context(true, Some(50_000)));
    let off = claude_stack(
        &spy,
        FixedFlags {
            metrics: true,
            max_tokens: Some(50_000),
            ..FixedFlags::default()
        },
    );

    let with_engine = run(&on, "Explain")
        .execution
        .optimization
        .unwrap()
        .budget
        .unwrap();
    let without = run(&off, "Explain")
        .execution
        .optimization
        .unwrap()
        .budget
        .unwrap();

    assert_eq!(with_engine.input.limit.value, Some(50_000));
    assert_eq!(with_engine.input.limit.source, FigureSource::Configured);
    assert!(!with_engine.model_limit_known());
    assert_eq!(without.input.limit.value, None);
}

#[test]
fn the_plan_and_the_manifest_agree_when_nothing_changed_the_prompt() {
    let spy = spy_for_claude();
    let stack = claude_stack(&spy, FixedFlags::metrics(true));

    let record = run(&stack, "Explain the module layout");

    let metrics = record.execution.optimization.clone().unwrap();
    let plan = record.execution.plan.clone().unwrap();
    let manifest = record.execution.manifest.clone().unwrap();
    assert_eq!(manifest.plan_fingerprint, plan.fingerprint);
    assert!(!manifest.diverged_from_plan);
    assert_eq!(plan.total_bytes, manifest.delivery.bytes);
    assert_eq!(manifest.execution_id, record.execution.id);
    assert_eq!(manifest.task_id, "task-1");
    assert_eq!(manifest.runtime_id, "claude");
    assert!(metrics.latency.delivery_ms.is_some());
}

#[test]
fn the_surface_separates_what_atlas_controls_from_what_the_runtime_and_the_user_do() {
    let spy = spy_for_claude();
    let stack = claude_stack(&spy, FixedFlags::metrics(true));

    let record = run(&stack, "Explain the module layout");

    let surface = record.execution.manifest.unwrap().surface;
    let find = |kind: SurfaceKind, control: SurfaceControl| {
        surface
            .entries
            .iter()
            .find(|e| e.kind == kind && e.control == control)
            .unwrap_or_else(|| panic!("{kind:?} {control:?}"))
    };
    // Atlas: what it wrote and passed. Reported where the runtime confirmed it at start-up.
    assert_eq!(
        find(SurfaceKind::Prompt, SurfaceControl::AtlasControlled).observation,
        Observation::Declared
    );
    assert_eq!(
        find(SurfaceKind::Tools, SurfaceControl::AtlasControlled).observation,
        Observation::Reported
    );
    assert_eq!(
        find(SurfaceKind::McpServers, SurfaceControl::AtlasControlled)
            .detail
            .as_deref(),
        Some("none")
    );
    assert!(
        find(SurfaceKind::LaunchFlags, SurfaceControl::AtlasControlled)
            .detail
            .as_deref()
            .unwrap()
            .contains("--strict-mcp-config")
    );
    // Runtime: its own plugins and memory, and its own system prompt.
    assert_eq!(
        find(SurfaceKind::Plugins, SurfaceControl::RuntimeControlled)
            .detail
            .as_deref(),
        Some("cc-plugin-telemetry@builtin")
    );
    assert_eq!(
        find(SurfaceKind::AutoMemory, SurfaceControl::RuntimeControlled).observation,
        Observation::Reported
    );
    assert_eq!(
        find(SurfaceKind::SystemPrompt, SurfaceControl::RuntimeControlled).observation,
        Observation::Declared
    );
    // The user: their plugins were reported; their instruction files, hooks and settings were not.
    assert_eq!(
        find(SurfaceKind::Plugins, SurfaceControl::UserControlled)
            .detail
            .as_deref(),
        Some("figma@claude-plugins-official")
    );
    for kind in [
        SurfaceKind::UserInstructions,
        SurfaceKind::Hooks,
        SurfaceKind::UserSettings,
    ] {
        assert_eq!(
            find(kind, SurfaceControl::UserControlled).observation,
            Observation::NotObserved
        );
    }
    // And what nobody can see.
    assert_eq!(
        find(SurfaceKind::Other, SurfaceControl::Unknown).observation,
        Observation::NotObserved
    );
    // Only the prompt Atlas delivered is confirmed to have reached the model.
    let confirmed: Vec<_> = surface.entries.iter().filter(|e| e.reaches_model).collect();
    assert_eq!(confirmed.len(), 1);
    assert_eq!(confirmed[0].kind, SurfaceKind::Prompt);
}

#[test]
fn a_run_without_metrics_still_has_its_manifest_and_the_same_prompt() {
    let spy = spy_for_claude();
    let measured = claude_stack(&spy, FixedFlags::metrics(true));
    let plain = claude_stack(&spy, FixedFlags::metrics(false));

    let with = run(&measured, "Explain the module layout");
    let without = run(&plain, "Explain the module layout");

    // The metrics are optional; the evidence of the delivery is not.
    assert!(without.execution.optimization.is_none());
    let manifest = without
        .execution
        .manifest
        .expect("a manifest without metrics");
    assert!(without.execution.plan.is_some());
    assert_eq!(
        manifest.delivery.prompt_hash,
        with.execution.manifest.unwrap().delivery.prompt_hash
    );
    assert_eq!(with.execution.prompt, without.execution.prompt);
    let calls = spy.calls.lock().unwrap();
    assert_eq!(received_prompt(&calls[0]), received_prompt(&calls[1]));
    assert_eq!(
        manifest.delivery.prompt_hash,
        sha256_of(&received_prompt(&calls[1]))
    );
}

/// A runtime that really has a system-prompt channel (none of the five does), to prove the split
/// is delivered, hashed and recorded as it is delivered. Its process is the spy.
struct SplitRuntime {
    runner: Arc<FakeProcessRunner>,
}

impl ModelRuntime for SplitRuntime {
    fn info(&self) -> crate::domain::runtime::RuntimeInfo {
        let mut info = ClaudeRuntime::new(self.runner.clone()).info();
        info.id = "split".to_owned();
        info.capabilities.system_prompt = SystemPromptChannel::Native;
        info
    }

    fn detect(&self) -> crate::application::runtimes::Detection {
        crate::application::runtimes::Detection {
            installed: true,
            version: None,
            unavailable: None,
            authentication: crate::domain::runtime::Authentication {
                kind: None,
                state: crate::domain::runtime::AuthState::Unknown,
            },
            notice: None,
        }
    }

    fn list_models(&self) -> Result<Vec<crate::domain::runtime::ModelInfo>, RuntimeError> {
        Ok(Vec::new())
    }

    fn execute(
        &self,
        request: &RuntimeRequest,
        _progress: &dyn Fn(RuntimeEvent),
    ) -> Result<RuntimeOutput, RuntimeError> {
        let delivery = self.delivery(request);
        // Each part goes where it belongs: the system text on its channel, the body on stdin.
        let spec = ProcessSpec {
            program: "split".to_owned(),
            args: vec![
                "--system".to_owned(),
                delivery
                    .system()
                    .expect("a runtime with a channel is given a split")
                    .to_owned(),
            ],
            stdin: Some(delivery.body().to_owned()),
            cwd: Some(request.working_dir.clone()),
            env: Vec::new(),
            timeout: std::time::Duration::from_secs(5),
            context: crate::application::process::ProcessContext::Probe,
            terminal: None,
        };
        self.runner
            .run(&spec, &|_| {})
            .map_err(|e| RuntimeError::ExecutionFailed(format!("{e:?}")))?;
        Ok(RuntimeOutput {
            text: "done".to_owned(),
            metadata: BTreeMap::new(),
            usage: None,
            quota: None,
        })
    }
}

#[test]
fn a_runtime_with_a_system_channel_gets_authoritative_text_there_and_the_manifest_hashes_both_parts(
) {
    let spy = Arc::new(FakeProcessRunner::new(&["split"], |_| ok("")));
    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    config
        .modify(|c| {
            c.rules = vec![crate::domain::rules::tests::rule(
                "tests",
                crate::domain::rules::RuleScope::Global,
                crate::domain::rules::RuleStrength::Mandatory,
                "All code must have tests",
            )];
            Ok(())
        })
        .unwrap();
    let personalities = Arc::new(PersonalityService::new(config.clone()));
    let registry = Arc::new(RuntimeRegistry::new(vec![Arc::new(SplitRuntime {
        runner: spy.clone(),
    }) as Arc<_>]));
    let agents = Arc::new(AgentService::new(
        config.clone(),
        personalities.clone(),
        registry.clone(),
    ));
    let workspaces = Arc::new(WorkspaceService::new(
        config.clone(),
        agents.clone(),
        Arc::new(FakeInspector::with(&[("/atlas", &["Rust"])])),
    ));
    let workspace_id = workspaces
        .create(&WorkspaceInput {
            name: "Project Atlas".to_owned(),
            project_path: "/atlas".to_owned(),
            description: None,
        })
        .unwrap()
        .id;
    let agent_id = agents
        .create(CreateAgentRequest {
            permission_profile_id: None,
            name: "split agent".to_owned(),
            personality_id: "architect".to_owned(),
            runtime_id: "split".to_owned(),
            model_id: "m1".to_owned(),
            instructions: "Be brief.".to_owned(),
            worktree_isolation: Some(false),
            result_contract: None,
        })
        .unwrap()
        .id;
    let service = ExecutionService::new(
        agents,
        personalities,
        registry,
        workspaces,
        Arc::new(crate::application::security::AuditLog::default()),
    )
    .with_optimization(Arc::new(FixedFlags::metrics(false)))
    .with_rules(Arc::new(crate::application::rules::RuleService::new(
        config,
    )));

    let record = service
        .run_with_id(
            service.next_execution_id(),
            RunAgentRequest {
                task_id: "task-1".to_owned(),
                workspace_id,
                agent_id,
                description: "Explain the layout".to_owned(),
            },
            &Quiet::default(),
        )
        .unwrap();

    assert_eq!(record.execution.status, ExecutionStatus::Completed);
    let calls = spy.calls.lock().unwrap();
    let system = calls[0].args[1].clone();
    let body = calls[0].stdin.clone().unwrap();
    // Atlas's instructions and the rules are on the channel; the task is in the body only.
    assert!(system.contains("[MANDATORY · Global] All code must have tests"));
    assert!(system.contains("PLANS ARE MANDATORY DOCUMENTS"));
    assert!(!system.contains("Explain the layout"));
    assert!(body.contains("Explain the layout") && !body.contains("PLANS ARE MANDATORY"));
    // The manifest hashes exactly those two parts, framed by their lengths, and says how they went.
    let manifest = record.execution.manifest.unwrap();
    let framed = format!(
        "atlas.delivery.split.v1\nsystem:{}\n{system}\nbody:{}\n{body}",
        system.len(),
        body.len()
    );
    assert_eq!(manifest.delivery.prompt_hash, sha256_of(&framed));
    assert_eq!(
        manifest.delivery.system_channel,
        SystemPromptChannel::Native
    );
    assert_eq!(manifest.delivery.system_bytes, system.len());
    assert_eq!(manifest.delivery.bytes, system.len() + body.len());
    let channel = manifest
        .surface
        .entries
        .iter()
        .find(|e| e.kind == SurfaceKind::SystemChannel)
        .unwrap();
    assert_eq!(channel.detail.as_deref(), Some("native"));
    assert_eq!(channel.observation, Observation::Declared);
}

#[test]
fn no_adapter_claims_a_system_channel_it_does_not_use() {
    let prompt = Prompt {
        system: "SYS".to_owned(),
        harness: None,
        task_aware: false,
        skills: None,
        rules: Some("a rule".to_owned()),
        context: "CTX".to_owned(),
        instruction: "INS".to_owned(),
    };
    let request = RuntimeRequest {
        model_id: "m".to_owned(),
        prompt: prompt.clone(),
        working_dir: "/atlas".into(),
        scope: ExecutionScope::for_tests(),
        text_only: false,
        allow_edits: false,
        mcp: None,
    };
    let runner: Arc<FakeProcessRunner> = Arc::new(FakeProcessRunner::new(&[], |_| ok("")));
    let runtimes: Vec<Box<dyn ModelRuntime>> = vec![
        Box::new(ClaudeRuntime::new(runner.clone())),
        Box::new(CodexRuntime::new(runner.clone())),
        Box::new(GeminiRuntime::new(runner.clone())),
        Box::new(OpenCodeRuntime::new(runner.clone())),
        Box::new(AntigravityRuntime::new(runner)),
    ];

    for runtime in runtimes {
        let name = runtime.info().name;
        assert_eq!(
            runtime.info().capabilities.system_prompt,
            SystemPromptChannel::Unsupported,
            "{name}"
        );
        // One text, as it always was: nothing is split for a runtime that cannot take it.
        let delivery = runtime.delivery(&request);
        assert_eq!(delivery.system(), None, "{name}");
        assert_eq!(delivery.body(), prompt.combined(), "{name}");
        let channel = runtime
            .surface(&request)
            .entries
            .into_iter()
            .find(|e| e.kind == SurfaceKind::SystemChannel)
            .unwrap_or_else(|| panic!("{name} declares no channel"));
        assert_eq!(channel.detail.as_deref(), Some("prompt_body"), "{name}");
    }
}
