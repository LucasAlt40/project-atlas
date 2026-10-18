//! The built-in templates (Implementer, Debugger, Reviewer), the templates a person saves, and what
//! the two say about tools: what they promise before a run, and that they promise nothing the
//! system cannot keep.

use std::collections::BTreeMap;

use super::orchestrator_tests::{env, outcome as declared, Env, Scripted};
use super::template_text as text;
use super::templates::{self, TemplateKind};
use crate::application::errors::ErrorCode;
use crate::domain::mcp::{
    McpConnection, McpDiscovery, McpGrant, McpServerStatus, McpTransport, ToolSelection,
};
use crate::domain::result_contract::{ContractKind, Outcome, ResultContract};
use crate::domain::workflow::{
    EndOutcome, NodeKind, SuggestionTarget, ToolCapability, Workflow, WorkflowMode, WorkflowNode,
};

fn outcome(id: &str, label: &str) -> Outcome {
    Outcome {
        id: id.to_owned(),
        label: label.to_owned(),
        description: String::new(),
    }
}

const FEATURED: [&str; 3] = ["software_feature", "bug_fix", "code_review"];

fn build(env: &Env, template: &str) -> Workflow {
    env.service
        .create_from_template(&env.workspace_id, template, None, WorkflowMode::Custom)
        .unwrap()
        .workflow
}

fn agent_steps(workflow: &Workflow) -> Vec<&WorkflowNode> {
    workflow
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Agent(_)))
        .collect()
}

fn connection(env: &Env, name: &str, enabled: bool, tools: Option<&[&str]>) -> McpConnection {
    McpConnection {
        id: format!("conn-{name}"),
        workspace_id: env.workspace_id.clone(),
        name: name.to_owned(),
        transport: McpTransport::Stdio {
            executable: "/usr/bin/node".to_owned(),
            args: vec![],
            env: vec![],
        },
        enabled,
        required: false,
        secrets: BTreeMap::new(),
        discovery: tools.map(|t| McpDiscovery {
            discovered_at: 1,
            runtime_id: "rt".to_owned(),
            status: McpServerStatus::Connected,
            tools: t.iter().map(|s| (*s).to_owned()).collect(),
        }),
        created_at: 1,
    }
}

fn add_connection(env: &Env, connection: McpConnection) {
    env.config
        .modify(|c| {
            c.mcp_connections.push(connection);
            Ok(())
        })
        .unwrap();
}

// ---- the three templates ---------------------------------------------------------------------------

#[test]
fn the_three_featured_templates_lead_the_list_and_say_what_they_do_before_anything_runs() {
    let list = templates::list();

    let leading: Vec<&str> = list.iter().take(3).map(|t| t.id.as_str()).collect();
    assert_eq!(leading, FEATURED);
    for info in list.iter().take(3) {
        assert!(
            info.featured && info.kind == TemplateKind::Builtin,
            "{}",
            info.id
        );
        assert!(
            !info.goal.is_empty() && !info.outputs.is_empty(),
            "{}",
            info.id
        );
        assert!(!info.steps.is_empty(), "{}", info.id);
        for step in &info.steps {
            // Each step says what it does, when it is done and who does it.
            assert!(!step.instructions.is_empty(), "{}/{}", info.id, step.id);
            assert!(
                !step.completion_criteria.is_empty(),
                "{}/{}",
                info.id,
                step.id
            );
            assert!(step.role.is_some(), "{}/{}", info.id, step.id);
        }
    }
    // The rest keep their ids, so workflows made from them before still point at a template.
    assert!(list.iter().skip(3).all(|t| !t.featured));
    assert_eq!(list.len(), 8);
}

/// Every template of the catalogue, not only the featured ones, with the agents as a person creates
/// them in the app (the agent form starts from the personality's suggested contract): a template
/// the list offers must build a workflow that can start.
#[test]
fn every_template_of_the_catalogue_builds_a_workflow_that_can_start_with_default_agents() {
    let env = env();
    let personalities =
        crate::application::personalities::PersonalityService::new(env.config.clone());
    for id in env.agents.values() {
        let personality = env.agent_service.find(id).unwrap().personality_id;
        let suggested = personalities.find(&personality).unwrap().suggested_contract;
        env.agent_service
            .set_result_contract(id, suggested)
            .unwrap();
    }
    let ids: Vec<String> = env.service.templates().into_iter().map(|t| t.id).collect();
    assert!(ids.len() >= 8, "{ids:?}");
    let mut broken = Vec::new();
    for id in &ids {
        let built = env
            .service
            .create_from_template(&env.workspace_id, id, None, WorkflowMode::Custom)
            .unwrap();
        let report = env.service.validate(&built.workflow);
        if !report.valid || !built.missing_roles.is_empty() {
            broken.push(format!(
                "{id}: {:?} missing={:?}",
                report.issues, built.missing_roles
            ));
        }
    }
    assert!(broken.is_empty(), "{broken:#?}");
}

#[test]
fn every_featured_template_builds_a_valid_ready_workflow_whatever_its_fixer_declares() {
    let env = env();
    env.declare_validation_outcomes();
    let declares = ResultContract {
        kind: ContractKind::Custom,
        outcomes: vec![outcome("fixed", "Fixed"), outcome("blocked", "Blocked")],
    };
    for contract in [ResultContract::general(), declares] {
        env.agent_service
            .set_result_contract(&env.agents["fixer"], contract)
            .unwrap();
        for id in FEATURED {
            let built = env
                .service
                .create_from_template(&env.workspace_id, id, None, WorkflowMode::Automatic)
                .unwrap();
            let report = env.service.validate(&built.workflow);
            assert!(report.valid, "{id}: {:?}", report.issues);
            assert!(built.missing_roles.is_empty(), "{id}");
            assert_eq!(
                built.workflow.status,
                crate::domain::workflow::WorkflowStatus::Ready
            );
        }
    }
}

#[test]
fn no_template_asks_an_agent_to_run_what_it_cannot_and_the_ones_that_matter_say_so() {
    let forbidden = [
        "run the tests",
        "run the test suite",
        "run or review",
        "execute the tests",
        "run the linter",
        "run cargo",
        "run npm",
    ];
    for info in templates::list() {
        for step in &info.steps {
            let said = step.instructions.to_lowercase();
            for phrase in forbidden {
                assert!(!said.contains(phrase), "{}/{}: {phrase}", info.id, step.id);
            }
        }
    }
    // Where a step would be tempted to report a test run, it says plainly that it cannot.
    for words in [
        text::IMPL_DEVELOPER,
        text::IMPL_QA,
        text::IMPL_FIXER,
        text::DEBUG_FIXER,
        text::DEBUG_VERIFY,
        text::REVIEW_AUDIT,
    ] {
        assert!(words.contains("cannot run"), "{words}");
    }
}

#[test]
fn the_implementer_keeps_its_steps_and_routes_and_reads_before_it_judges() {
    let env = env();
    env.declare_validation_outcomes();

    let workflow = build(&env, "software_feature");

    let ids: Vec<&str> = workflow.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "architect",
            "developer",
            "validator",
            "bug-fixer-architecture",
            "qa",
            "bug-fixer-qa",
            "done"
        ]
    );
    // The two that judge only read, whatever their agents could do; the others are as before.
    let read_only: Vec<&str> = agent_steps(&workflow)
        .into_iter()
        .filter(|n| n.agent().is_some_and(|a| a.execution_policy.read_only))
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(read_only, ["validator", "qa"]);
    // Both loops are bounded.
    for id in ["validator", "qa"] {
        assert_eq!(
            workflow
                .node(id)
                .unwrap()
                .loop_policy
                .as_ref()
                .unwrap()
                .max_iterations,
            3
        );
    }
    // The first step sizes the task, and the next ones are told to scale to that size.
    let architect = workflow.node("architect").unwrap().agent().unwrap();
    assert!(architect.instructions.contains("Size: trivial"));
    assert!(workflow
        .node("validator")
        .unwrap()
        .agent()
        .unwrap()
        .instructions
        .contains("Scale the depth"));
}

#[test]
fn the_debugger_collects_evidence_before_it_fixes_and_ends_inconclusive_instead_of_guessing() {
    let env = env();
    env.declare_validation_outcomes();
    // The fixer declares the outcomes the template routes on.
    let fixer = ResultContract {
        kind: ContractKind::Custom,
        outcomes: vec![outcome("fixed", "Fixed"), outcome("blocked", "Blocked")],
    };
    env.agent_service
        .set_result_contract(&env.agents["fixer"], fixer)
        .unwrap();

    let workflow = build(&env, "bug_fix");

    let first = workflow.node("bug-fixer").unwrap().agent().unwrap();
    let evidence = first.instructions.find("PHASE 1").unwrap();
    let fix = first.instructions.find("PHASE 2").unwrap();
    assert!(evidence < fix);
    assert!(first.instructions.contains("Root cause: confirmed"));
    assert!(first.instructions.contains("change nothing"));
    let route = |from: &str, outcome: &str| {
        workflow
            .edges
            .iter()
            .find(|e| {
                e.source_node_id == from
                    && e.condition.as_ref().and_then(|c| c.value.as_deref()) == Some(outcome)
            })
            .map(|e| e.target_node_id.as_str())
    };
    assert_eq!(route("bug-fixer", "fixed"), Some("qa"));
    assert_eq!(route("bug-fixer", "blocked"), Some("inconclusive"));
    assert_eq!(route("bug-fixer-retry", "blocked"), Some("inconclusive"));
    let end = workflow.node("inconclusive").unwrap();
    assert!(matches!(&end.kind, NodeKind::End(e) if e.outcome == EndOutcome::Failed));
    assert!(env.service.validate(&workflow).valid);
    // The verification reads; it never edits.
    assert!(
        workflow
            .node("qa")
            .unwrap()
            .agent()
            .unwrap()
            .execution_policy
            .read_only
    );

    // With a fixer that declares no outcomes there is no route to take on them, so the step goes
    // on to the verification and the workflow is still valid.
    env.agent_service
        .set_result_contract(&env.agents["fixer"], ResultContract::general())
        .unwrap();
    let general = build(&env, "bug_fix");
    assert!(general
        .edges
        .iter()
        .any(|e| e.id == "bug-fixer->qa" && e.condition.is_none()));
    assert!(env.service.validate(&general).valid);
}

#[test]
fn the_reviewer_changes_nothing_by_default_and_offers_no_tool_that_acts() {
    let env = env();
    env.declare_validation_outcomes();

    let workflow = build(&env, "code_review");

    // One step, an agent, and it is held to reading: the template cannot make a file change.
    let steps = agent_steps(&workflow);
    assert_eq!(steps.len(), 1);
    let step = steps[0].agent().unwrap();
    assert!(step.execution_policy.read_only);
    assert!(step.instructions.contains("do not edit files"));
    let info = templates::find("code_review").unwrap();
    assert!(info.read_only);
    assert_eq!(info.roles, [templates::Role::Validator]);
    // Every tool it suggests is one it only consults with: never a browser, never the whole server.
    assert_eq!(info.tools.len(), 3);
    for tool in &info.tools {
        let SuggestionTarget::Capability { capability } = &tool.target else {
            panic!("a built-in suggestion names a kind");
        };
        assert!(capability.consults(), "{capability:?}");
    }
    // The other two are not read-only as a whole.
    assert!(!templates::find("software_feature").unwrap().read_only);
    assert!(!templates::find("bug_fix").unwrap().read_only);
}

#[test]
fn every_tool_a_template_suggests_belongs_to_a_step_that_exists_and_says_why() {
    for info in templates::list() {
        let step_ids: Vec<&str> = info.steps.iter().map(|s| s.id.as_str()).collect();
        for tool in &info.tools {
            assert!(
                step_ids.contains(&tool.node_id.as_str()),
                "{}/{}",
                info.id,
                tool.node_id
            );
            assert!(!tool.purpose.is_empty(), "{}/{}", info.id, tool.node_id);
        }
    }
    // Each kind has names Atlas recognizes: a suggestion nothing can match would be decoration.
    for kind in [
        ToolCapability::Browser,
        ToolCapability::Issues,
        ToolCapability::Docs,
        ToolCapability::Repo,
    ] {
        assert!(kind.known_names().len() >= 2, "{kind:?}");
    }
}

#[test]
fn a_small_task_gets_the_quick_template_and_everything_else_is_unchanged() {
    for (task, expected) in [
        ("Fix a typo in the settings label", "bug_fix"),
        ("Rename the helper to something clearer", "feature_basic"),
        ("Renomear o botão de salvar", "feature_basic"),
        ("Add a dark mode toggle", "software_feature"),
    ] {
        assert_eq!(templates::select_for_task(task), expected, "{task}");
    }
    // A long task is not small because one of its sentences says so.
    let long = format!("rename the module. {}", "context ".repeat(40));
    assert_eq!(templates::select_for_task(&long), "software_feature");
}

#[test]
fn automatic_mode_reads_what_the_task_says_in_either_language_not_just_a_few_fixed_words() {
    let cases = [
        // The task that was wrongly sent to the Implementer: it describes a problem.
        (
            "precisamos ajustar a area de pagamentos do projeto. O seguinte problema está acontecendo: \
             ao fazer uma doação recorrente o registro do pagamento é feito mas o mês seguinte remove \
             do mês anterior",
            "bug_fix",
        ),
        ("The export button does not work on Safari", "bug_fix"),
        ("O botão de salvar não funciona depois do login", "bug_fix"),
        ("Users report unexpected logouts after the update", "bug_fix"),
        ("Revisar a segurança do módulo de autenticação", "code_review"),
        ("Audit the payment webhooks for race conditions", "code_review"),
        ("Reestruturar o módulo de relatórios", "refactoring"),
        ("Criar uma tela de relatórios com filtros por período", "software_feature"),
        ("Permitir exportar as doações em CSV", "software_feature"),
        // No cue at all: the feature template, as before.
        ("Dark mode", "software_feature"),
        ("", "software_feature"),
    ];
    for (task, expected) in cases {
        assert_eq!(templates::select_for_task(task), expected, "{task}");
    }
    // A tie goes to the more specific category, and the same text always gives the same answer.
    let tie = "Revisar e corrigir o checkout";
    assert_eq!(templates::select_for_task(tie), "code_review");
    assert_eq!(
        templates::select_for_task(tie),
        templates::select_for_task(tie)
    );
    // A word is not found inside another ("address" is not "add", "debugging" is not a request to build).
    assert_eq!(
        templates::select_for_task("Change the address label"),
        "software_feature"
    );
}

// ---- tools: suggestion, not authorization ------------------------------------------------------------

#[test]
fn a_template_says_which_tools_the_workspace_can_give_and_authorizes_none_of_them() {
    let env = env();
    add_connection(
        &env,
        connection(
            &env,
            "jira",
            true,
            Some(&["getJiraIssue", "createJiraIssue"]),
        ),
    );
    add_connection(&env, connection(&env, "chrome-devtools", false, None));

    let built = env
        .service
        .create_from_template(&env.workspace_id, "bug_fix", None, WorkflowMode::Custom)
        .unwrap();

    let by_kind = |kind: ToolCapability, node: &str| {
        built
            .tools
            .iter()
            .find(|t| t.capability == Some(kind) && t.node_id == node)
            .unwrap()
    };
    let issues = by_kind(ToolCapability::Issues, "bug-fixer");
    assert_eq!(issues.connection.as_deref(), Some("jira"));
    assert_eq!(
        issues.tools,
        Some(ToolSelection::Only {
            tools: vec!["getJiraIssue".to_owned()]
        })
    );
    // Off in the hub: said so, not given.
    let browser = by_kind(ToolCapability::Browser, "bug-fixer");
    assert!(matches!(
        browser.reason,
        Some(super::template_tools::ToolUnavailable::Disabled)
    ));
    // The workspace has nothing for repository history: said so too.
    let repo = by_kind(ToolCapability::Repo, "bug-fixer");
    assert!(matches!(
        repo.reason,
        Some(super::template_tools::ToolUnavailable::NoConnection)
    ));
    // Nothing was granted by building the workflow.
    assert!(env.config.read(|c| c.mcp_grants.is_empty()));
}

// ---- saving, keeping and deleting ---------------------------------------------------------------------

#[test]
fn a_saved_template_keeps_the_edits_survives_a_restart_and_leaves_the_built_in_one_untouched() {
    let env = env();
    env.declare_validation_outcomes();
    let mut workflow = build(&env, "software_feature");
    for node in &mut workflow.nodes {
        if let NodeKind::Agent(step) = &mut node.kind {
            if node.id == "developer" {
                step.instructions = "Our house rules first.".to_owned();
                step.completion_criteria = "Our own definition of done.".to_owned();
            }
        }
    }
    let workflow = env.service.update(workflow).unwrap();
    add_connection(
        &env,
        connection(&env, "context7", true, Some(&["get-library-docs"])),
    );
    let developer_agent = workflow
        .node("developer")
        .unwrap()
        .agent_id()
        .unwrap()
        .to_owned();
    env.config
        .modify(|c| {
            c.mcp_grants.push(McpGrant {
                id: "g1".to_owned(),
                connection_id: "conn-context7".to_owned(),
                agent_id: Some(developer_agent.clone()),
                workflow_id: Some(workflow.id.clone()),
                node_id: Some("developer".to_owned()),
                tools: ToolSelection::Only {
                    tools: vec!["get-library-docs".to_owned()],
                },
            });
            Ok(())
        })
        .unwrap();

    let saved = env
        .service
        .save_as_template(&workflow.id, "House rules", "How we do it here")
        .unwrap();

    // Listed after the built-in ones, as a custom template with its own steps and tools.
    let listed = env.service.templates();
    assert_eq!(listed.len(), 9);
    let info = listed.last().unwrap();
    assert_eq!(info.id, saved.id);
    assert_eq!(info.kind, TemplateKind::Custom);
    assert_eq!(info.based_on.as_deref(), Some("software_feature"));
    let developer = info.steps.iter().find(|s| s.id == "developer").unwrap();
    assert_eq!(developer.instructions, "Our house rules first.");
    assert_eq!(developer.completion_criteria, "Our own definition of done.");
    assert_eq!(info.tools.len(), 1);
    // The built-in one is as it was.
    let original = templates::find("software_feature").unwrap();
    let original_developer = original.steps.iter().find(|s| s.id == "developer").unwrap();
    assert_eq!(original_developer.instructions, text::IMPL_DEVELOPER);

    // A restart: the template is read back from what was written to disk.
    let reloaded = crate::application::config::ConfigRepository::load(Box::new(
        crate::application::config::memory::MemoryStore {
            saved: std::sync::Mutex::new(env.store.saved.lock().unwrap().clone()),
            fail_saves: false,
        },
    ));
    assert_eq!(reloaded.read(|c| c.workflow_templates.len()), 1);

    // A workflow made from it has the edited step, and says which tool it would use.
    let again = env
        .service
        .create_from_template(
            &env.workspace_id,
            &saved.id,
            Some("Second"),
            WorkflowMode::Custom,
        )
        .unwrap();
    assert_eq!(
        again.workflow.template_id.as_deref(),
        Some(saved.id.as_str())
    );
    assert_eq!(again.workflow.name, "Second");
    assert_ne!(again.workflow.id, workflow.id);
    let again_developer = again.workflow.node("developer").unwrap().agent().unwrap();
    assert_eq!(again_developer.instructions, "Our house rules first.");
    assert_eq!(again.tools.len(), 1);
    assert_eq!(again.tools[0].connection.as_deref(), Some("context7"));
    assert!(again.tools[0].reason.is_none());
    assert!(env.service.validate(&again.workflow).valid);
}

#[test]
fn only_a_saved_template_can_be_deleted_and_the_workflows_made_from_it_stay() {
    let env = env();
    env.declare_validation_outcomes();
    let workflow = build(&env, "feature_basic");
    let saved = env
        .service
        .save_as_template(&workflow.id, "Mine", "")
        .unwrap();
    let from_it = env
        .service
        .create_from_template(&env.workspace_id, &saved.id, None, WorkflowMode::Custom)
        .unwrap()
        .workflow;

    assert!(env
        .service
        .delete_template("software_feature")
        .unwrap_err()
        .is(ErrorCode::WorkflowTemplateBuiltin));
    assert!(env
        .service
        .delete_template("nope")
        .unwrap_err()
        .is(ErrorCode::WorkflowTemplateNotFound));
    env.service.delete_template(&saved.id).unwrap();

    assert_eq!(env.service.templates().len(), 8);
    assert!(env.service.get(&from_it.id).is_some());
    assert!(env
        .service
        .create_from_template(&env.workspace_id, &saved.id, None, WorkflowMode::Custom)
        .err()
        .is_some_and(|e| e.is(ErrorCode::WorkflowTemplateNotFound)));
    assert!(env
        .service
        .save_as_template("nope", "x", "")
        .unwrap_err()
        .is(ErrorCode::WorkflowNotFound));
    assert!(env
        .service
        .save_as_template(&workflow.id, "   ", "")
        .unwrap_err()
        .is(ErrorCode::NameRequired));
}

#[test]
fn deleting_a_workflow_takes_back_what_was_granted_to_its_steps() {
    let env = env();
    env.declare_validation_outcomes();
    let workflow = build(&env, "code_review");
    let other = build(&env, "feature_basic");
    add_connection(&env, connection(&env, "jira", true, None));
    env.config
        .modify(|c| {
            for (id, wf) in [("g1", &workflow.id), ("g2", &other.id)] {
                c.mcp_grants.push(McpGrant {
                    id: id.to_owned(),
                    connection_id: "conn-jira".to_owned(),
                    agent_id: None,
                    workflow_id: Some(wf.clone()),
                    node_id: None,
                    tools: ToolSelection::Server,
                });
            }
            Ok(())
        })
        .unwrap();

    env.service.delete(&workflow.id).unwrap();

    let left: Vec<String> = env
        .config
        .read(|c| c.mcp_grants.iter().map(|g| g.id.clone()).collect());
    assert_eq!(left, ["g2"]);
}

// ---- what was stored before ----------------------------------------------------------------------------

#[test]
fn a_workflow_stored_before_criteria_and_read_only_steps_reads_as_it_always_ran() {
    let legacy = r#"{
        "id": "n1", "type": "agent", "agentId": "a1", "label": "Dev",
        "instructions": "Implement it",
        "retryPolicy": {"maxRetries": 0}, "failurePolicy": {"type": "stop_workflow"},
        "executionPolicy": {"isolation": "from_agent", "announceIntegrations": true}
    }"#;

    let node: WorkflowNode = serde_json::from_str(legacy).unwrap();

    let step = node.agent().unwrap();
    assert_eq!(step.completion_criteria, "");
    assert!(!step.execution_policy.read_only);
    assert!(step.execution_policy.announce_integrations);
    // Nor does a configuration written before saved templates existed lack anything.
    let config: crate::application::config::UserConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(config.workflow_templates.len(), 0);
}

/// The agents as the agent form creates them: each with its personality's suggested contract.
fn seat_as_the_form_does(env: &Env) {
    let personalities =
        crate::application::personalities::PersonalityService::new(env.config.clone());
    for id in env.agents.values() {
        let personality = env.agent_service.find(id).unwrap().personality_id;
        let suggested = personalities.find(&personality).unwrap().suggested_contract;
        env.agent_service
            .set_result_contract(id, suggested)
            .unwrap();
    }
}

/// The quick templates have no fixer: a judge's rejection must end the run as failed, never as a
/// success, and an agent that declares no verdict keeps the plain route it always had.
#[test]
fn a_quick_template_ends_failed_when_its_judge_rejects_and_completed_when_all_accept() {
    use crate::domain::workflow::WorkflowExecutionStatus::{Completed, Failed};
    let cases: [(&str, &str, &str, _); 5] = [
        ("feature_with_validation", "fail", "pass", Failed),
        ("feature_with_validation", "pass", "fail", Failed),
        ("feature_with_validation", "pass", "pass", Completed),
        ("feature_basic", "", "fail", Failed),
        ("feature_basic", "", "pass", Completed),
    ];
    for (template, validator, qa, expected) in cases {
        let env = env();
        seat_as_the_form_does(&env);
        let workflow = build(&env, template);
        assert!(env.service.validate(&workflow).valid, "{template}");
        let runner = Scripted::new();
        runner.script("architect", vec![declared("approved", "")]);
        runner.script("developer", vec![declared("implemented", "")]);
        if !validator.is_empty() {
            runner.script("validator", vec![declared(validator, "")]);
        }
        runner.script("qa", vec![declared(qa, "")]);

        let (exec, _) = env.run(&runner, &workflow.id);

        assert_eq!(
            exec.status, expected,
            "{template} validator={validator} qa={qa}"
        );
        if expected == Failed {
            // QA never runs after a rejected validation.
            assert_eq!(
                runner.requests_for("qa").len(),
                usize::from(validator != "fail")
            );
        }
    }
}

#[test]
fn a_quick_template_with_general_agents_keeps_its_plain_route() {
    let env = env();
    for id in ["feature_basic", "feature_with_validation"] {
        let workflow = build(&env, id);
        assert!(env.service.validate(&workflow).valid, "{id}");
        assert!(
            workflow.edges.iter().all(|e| e.condition.is_none()),
            "{id}: a general agent has no verdict to route on"
        );
    }
}

/// An agent saved before result contracts existed (or imported without one) declares no verdict.
/// Nothing is invented for it: a template that must route on its verdict is born a draft that
/// names it, and the run cannot start until the person gives it a contract.
#[test]
fn a_template_whose_judge_declares_no_verdict_is_a_draft_that_names_it_and_cannot_start() {
    let env = env();
    // As loaded from an old config: the agents carry the `general` contract.
    let built = env
        .service
        .create_from_template(
            &env.workspace_id,
            "software_feature",
            None,
            WorkflowMode::Custom,
        )
        .unwrap();
    assert_eq!(
        built.workflow.status,
        crate::domain::workflow::WorkflowStatus::Draft
    );
    let report = env.service.validate(&built.workflow);
    let named: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.code == super::validation::IssueCode::UndeclaredOutcome)
        .filter_map(|i| i.params.get("agent").cloned())
        .collect();
    assert!(named.iter().any(|a| a == "qa agent"), "{named:?}");
    let refused = env
        .service
        .start(&built.workflow.id, "Implement password recovery")
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::WorkflowInvalid);

    // Once the person gives the judges the contract their personality suggests, it can start.
    seat_as_the_form_does(&env);
    let rebuilt = build(&env, "software_feature");
    assert!(env.service.validate(&rebuilt).valid);
}

#[test]
fn an_agent_stored_or_imported_without_a_contract_reads_as_general() {
    let stored: crate::domain::agent::Agent = serde_json::from_value(serde_json::json!({
        "id": "agent-old", "name": "Old QA", "personalityId": "qa", "runtimeId": "opencode",
        "modelId": "m", "instructions": "", "createdAt": 1
    }))
    .unwrap();
    assert_eq!(stored.result_contract, ResultContract::general());
    assert!(!stored.result_contract.requires_outcome());
}
