//! Which tools a template can really give a workspace. A template only *suggests* (see
//! `domain::workflow::template`); this looks at the workspace's connections and says, for each
//! suggestion, which connection would serve it, which tools it would cover, or why none can.
//! Nothing here starts a server or authorizes anything: [`apply`] turns the usable suggestions
//! into grants, and only when the person asked for that.
//!
//! The rule for tools a step only *consults* (tickets, documentation, history): the grant covers
//! the server's reading tools, and only when they are known (the connection was discovered), so
//! a tool that writes is never part of it. Without a discovery there is no way to hold the rest
//! back, so the suggestion is not applied and says so.

use serde::Serialize;

use crate::domain::mcp::{McpConnection, McpServerStatus, ToolSelection};
use crate::domain::workflow::{SuggestionTarget, ToolCapability, ToolSuggestion, Workflow};

/// Why a suggestion cannot be applied in this workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolUnavailable {
    /// The workspace has no connection of that kind (or with that name).
    NoConnection,
    /// It has one, switched off. Switching it on is the person's decision, in the MCP hub.
    Disabled,
    /// The connection needs a secret that was never stored.
    #[serde(rename_all = "camelCase")]
    NeedsSecret { name: String },
    /// The last look at the server said it could not connect.
    NotConnected,
    /// The step only consults, and the server's tools are not known, so the ones that write cannot
    /// be held back. Discover the server's tools in the MCP hub first.
    NeedsDiscovery,
    /// The server was discovered and none of its tools is one that reads.
    NoReadingTools,
    /// The grant was refused when it was made.
    Refused,
}

/// One suggestion, resolved against a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPlanItem {
    pub node_id: String,
    pub purpose: String,
    pub capability: Option<ToolCapability>,
    pub connection_id: Option<String>,
    pub connection: Option<String>,
    /// What would be (or was) granted.
    pub tools: Option<ToolSelection>,
    /// Why not. `None`: it can be granted.
    pub reason: Option<ToolUnavailable>,
    /// A grant was made for it.
    pub granted: bool,
}

/// The first word of a tool's name (`get_issue`, `getIssue`, `get-issue` → `get`).
fn first_word(tool: &str) -> String {
    let mut word = String::new();
    for c in tool.chars() {
        if c == '_' || c == '-' || c == '.' || c == ' ' || (c.is_uppercase() && !word.is_empty()) {
            break;
        }
        word.push(c.to_ascii_lowercase());
    }
    word
}

/// Whether a tool's name says it reads: its first word is one of Atlas's reading verbs. A guess
/// from the name (a server's tool descriptions are not known to Atlas), used only to *narrow* a
/// grant, never to widen one.
pub fn reads(tool: &str) -> bool {
    const READING: [&str; 12] = [
        "get", "list", "search", "read", "fetch", "find", "query", "view", "describe", "inspect",
        "lookup", "resolve",
    ];
    READING.contains(&first_word(tool).as_str())
}

fn matches(capability: ToolCapability, name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    capability
        .known_names()
        .iter()
        .any(|known| name == *known || name.contains(known))
}

/// Whether the connection can be given at all, before looking at tools.
fn blocked(connection: &McpConnection) -> Option<ToolUnavailable> {
    if !connection.enabled {
        return Some(ToolUnavailable::Disabled);
    }
    if let Some(name) = connection.missing_secret() {
        return Some(ToolUnavailable::NeedsSecret {
            name: name.to_owned(),
        });
    }
    if let Some(discovery) = &connection.discovery {
        if matches!(
            discovery.status,
            McpServerStatus::Failed | McpServerStatus::NeedsAuth
        ) {
            return Some(ToolUnavailable::NotConnected);
        }
    }
    None
}

/// The tools a suggestion would cover on a connection, or why it cannot.
fn cover(
    connection: &McpConnection,
    consults: bool,
    saved: Option<&ToolSelection>,
) -> Result<ToolSelection, ToolUnavailable> {
    if let Some(reason) = blocked(connection) {
        return Err(reason);
    }
    let known = connection.discovery.as_ref().map(|d| &d.tools);
    match (saved, consults) {
        // A saved template remembers exactly what it granted.
        (Some(selection), _) => {
            if matches!(selection, ToolSelection::Only { .. }) && known.is_none() {
                return Err(ToolUnavailable::NeedsDiscovery);
            }
            Ok(selection.clone())
        }
        (None, false) => Ok(ToolSelection::Server),
        (None, true) => {
            let tools = known.ok_or(ToolUnavailable::NeedsDiscovery)?;
            let reading: Vec<String> = tools.iter().filter(|t| reads(t)).cloned().collect();
            if reading.is_empty() {
                Err(ToolUnavailable::NoReadingTools)
            } else {
                Ok(ToolSelection::Only { tools: reading })
            }
        }
    }
}

/// Resolves the suggestions against the connections of one workspace. Deterministic: with several
/// connections of a kind, the usable ones come first, then the order they were added in.
pub fn resolve(suggestions: &[ToolSuggestion], connections: &[McpConnection]) -> Vec<ToolPlanItem> {
    suggestions
        .iter()
        .map(|suggestion| {
            let item = |connection: Option<&McpConnection>,
                        capability: Option<ToolCapability>,
                        outcome: Result<ToolSelection, ToolUnavailable>| {
                let (tools, reason) = match outcome {
                    Ok(tools) => (Some(tools), None),
                    Err(reason) => (None, Some(reason)),
                };
                ToolPlanItem {
                    node_id: suggestion.node_id.clone(),
                    purpose: suggestion.purpose.clone(),
                    capability,
                    connection_id: connection.map(|c| c.id.clone()),
                    connection: connection.map(|c| c.name.clone()),
                    tools,
                    reason,
                    granted: false,
                }
            };
            match &suggestion.target {
                SuggestionTarget::Capability { capability } => {
                    let mut candidates: Vec<&McpConnection> = connections
                        .iter()
                        .filter(|c| matches(*capability, &c.name))
                        .collect();
                    candidates.sort_by_key(|c| blocked(c).is_some()); // stable
                    match candidates.first() {
                        None => item(None, Some(*capability), Err(ToolUnavailable::NoConnection)),
                        Some(found) => item(
                            Some(found),
                            Some(*capability),
                            cover(found, capability.consults(), None),
                        ),
                    }
                }
                SuggestionTarget::Connection { name, tools } => {
                    match connections.iter().find(|c| &c.name == name) {
                        None => item(None, None, Err(ToolUnavailable::NoConnection)),
                        Some(found) => item(Some(found), None, cover(found, false, Some(tools))),
                    }
                }
            }
        })
        .collect()
}

/// What a grant needs, for the caller that makes it.
pub struct GrantRequest<'a> {
    pub connection_id: &'a str,
    pub agent_id: &'a str,
    pub workflow_id: &'a str,
    pub node_id: &'a str,
    pub tools: ToolSelection,
}

/// Grants what can be granted, to the step and the agent it names in `workflow`, and records on
/// each item whether it was. A step without an agent yet, or that is not in the workflow, gets
/// nothing. `grant` does the granting (and its own validation); a refusal is recorded, not raised.
pub fn apply<E>(
    workflow: &Workflow,
    items: &mut [ToolPlanItem],
    mut grant: impl FnMut(GrantRequest<'_>) -> Result<(), E>,
) {
    for item in items.iter_mut().filter(|i| i.reason.is_none()) {
        let (Some(connection_id), Some(tools)) = (item.connection_id.clone(), item.tools.clone())
        else {
            continue;
        };
        let agent_id = workflow
            .node(&item.node_id)
            .and_then(|n| n.agent_id())
            .filter(|id| !id.is_empty());
        let Some(agent_id) = agent_id else {
            continue;
        };
        let request = GrantRequest {
            connection_id: &connection_id,
            agent_id,
            workflow_id: &workflow.id,
            node_id: &item.node_id,
            tools,
        };
        if grant(request).is_ok() {
            item.granted = true;
        } else {
            item.reason = Some(ToolUnavailable::Refused);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::domain::mcp::{McpDiscovery, McpEnv, McpEnvValue, McpTransport};

    fn connection(name: &str, enabled: bool, tools: Option<&[&str]>) -> McpConnection {
        McpConnection {
            id: format!("c-{name}"),
            workspace_id: "w".to_owned(),
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
                runtime_id: "claude".to_owned(),
                status: McpServerStatus::Connected,
                tools: t.iter().map(|s| (*s).to_owned()).collect(),
            }),
            created_at: 1,
        }
    }

    fn suggestion(capability: ToolCapability) -> ToolSuggestion {
        ToolSuggestion {
            node_id: "step".to_owned(),
            target: SuggestionTarget::Capability { capability },
            purpose: "why".to_owned(),
        }
    }

    #[test]
    fn a_reading_tool_is_told_from_a_writing_one_by_its_first_word() {
        for reading in [
            "get_issue",
            "getIssue",
            "search-issues",
            "list_pages",
            "resolve-library-id",
        ] {
            assert!(reads(reading), "{reading}");
        }
        for writing in [
            "create_issue",
            "updateIssue",
            "delete_page",
            "navigate_page",
        ] {
            assert!(!reads(writing), "{writing}");
        }
        // The first word is the whole word: `getaway` is not `get`.
        assert!(!reads("getaway_x1"));
    }

    #[test]
    fn a_consulting_suggestion_covers_the_reading_tools_and_never_a_writing_one() {
        let jira = connection(
            "jira",
            true,
            Some(&[
                "getJiraIssue",
                "searchJiraIssues",
                "createJiraIssue",
                "editJiraIssue",
            ]),
        );

        let plan = resolve(&[suggestion(ToolCapability::Issues)], &[jira]);

        assert_eq!(plan[0].connection.as_deref(), Some("jira"));
        assert_eq!(plan[0].reason, None);
        assert_eq!(
            plan[0].tools,
            Some(ToolSelection::Only {
                tools: vec!["getJiraIssue".to_owned(), "searchJiraIssues".to_owned()]
            })
        );
    }

    #[test]
    fn a_consulting_suggestion_waits_for_discovery_instead_of_granting_the_whole_server() {
        let plan = resolve(
            &[suggestion(ToolCapability::Docs)],
            &[connection("context7", true, None)],
        );

        assert_eq!(plan[0].reason, Some(ToolUnavailable::NeedsDiscovery));
        assert_eq!(plan[0].tools, None);
    }

    #[test]
    fn a_browser_suggestion_covers_the_server_because_its_tools_act_on_a_page() {
        let plan = resolve(
            &[suggestion(ToolCapability::Browser)],
            &[connection("chrome-devtools", true, None)],
        );

        assert_eq!(plan[0].tools, Some(ToolSelection::Server));
        assert_eq!(plan[0].reason, None);
    }

    #[test]
    fn what_the_workspace_lacks_is_said_not_worked_around() {
        let connections = [
            connection("jira", false, Some(&["getJiraIssue"])),
            connection("unrelated", true, None),
        ];

        let plan = resolve(
            &[
                suggestion(ToolCapability::Issues),
                suggestion(ToolCapability::Repo),
            ],
            &connections,
        );

        assert_eq!(plan[0].reason, Some(ToolUnavailable::Disabled));
        assert_eq!(plan[1].reason, Some(ToolUnavailable::NoConnection));
        assert!(plan.iter().all(|p| p.tools.is_none() && !p.granted));
    }

    #[test]
    fn a_connection_that_cannot_connect_or_lacks_its_secret_is_not_suggested() {
        let mut failing = connection("jira", true, Some(&["getJiraIssue"]));
        failing.discovery.as_mut().unwrap().status = McpServerStatus::NeedsAuth;
        let mut secretless = connection("github", true, Some(&["get_issue"]));
        secretless.transport = McpTransport::Stdio {
            executable: "/usr/bin/node".to_owned(),
            args: vec![],
            env: vec![McpEnv {
                name: "TOKEN".to_owned(),
                value: McpEnvValue::Secret,
            }],
        };

        let plan = resolve(
            &[
                suggestion(ToolCapability::Issues),
                suggestion(ToolCapability::Repo),
            ],
            &[failing, secretless],
        );

        assert_eq!(plan[0].reason, Some(ToolUnavailable::NotConnected));
        assert_eq!(
            plan[1].reason,
            Some(ToolUnavailable::NeedsSecret {
                name: "TOKEN".to_owned()
            })
        );
    }

    #[test]
    fn a_usable_connection_is_preferred_over_one_that_is_off() {
        let connections = [
            connection("docs-old", false, Some(&["get_page"])),
            connection("docs-new", true, Some(&["get_page"])),
        ];

        let plan = resolve(&[suggestion(ToolCapability::Docs)], &connections);

        assert_eq!(plan[0].connection.as_deref(), Some("docs-new"));
        assert_eq!(plan[0].reason, None);
    }

    #[test]
    fn a_saved_template_asks_for_the_connection_by_name_with_the_tools_it_had() {
        let saved = ToolSuggestion {
            node_id: "step".to_owned(),
            target: SuggestionTarget::Connection {
                name: "files".to_owned(),
                tools: ToolSelection::Only {
                    tools: vec!["read".to_owned()],
                },
            },
            purpose: String::new(),
        };

        let with = resolve(
            std::slice::from_ref(&saved),
            &[connection("files", true, Some(&["read", "write"]))],
        );
        let without = resolve(
            std::slice::from_ref(&saved),
            &[connection("other", true, None)],
        );

        assert_eq!(
            with[0].tools,
            Some(ToolSelection::Only {
                tools: vec!["read".to_owned()]
            })
        );
        assert_eq!(without[0].reason, Some(ToolUnavailable::NoConnection));
    }

    fn workflow_with(agent_id: &str) -> Workflow {
        let mut workflow = crate::application::workflow::templates::instantiate(
            "code_review",
            "w",
            "wf",
            None,
            &[crate::application::workflow::templates::AgentChoice {
                id: agent_id.to_owned(),
                personality_id: "architecture-validator".to_owned(),
                placed: true,
                outcomes: vec![],
            }],
            crate::domain::workflow::WorkflowMode::Automatic,
            1,
        )
        .unwrap()
        .workflow;
        workflow.id = "wf".to_owned();
        workflow
    }

    #[test]
    fn applying_grants_only_the_usable_suggestions_to_the_step_and_its_agent() {
        let workflow = workflow_with("agent-1");
        let mut plan = resolve(
            &[
                ToolSuggestion {
                    node_id: "validator".to_owned(),
                    ..suggestion(ToolCapability::Issues)
                },
                ToolSuggestion {
                    node_id: "validator".to_owned(),
                    ..suggestion(ToolCapability::Repo)
                },
            ],
            &[connection(
                "jira",
                true,
                Some(&["getJiraIssue", "createJiraIssue"]),
            )],
        );
        let mut granted = Vec::new();

        apply::<()>(&workflow, &mut plan, |request| {
            granted.push((
                request.connection_id.to_owned(),
                request.agent_id.to_owned(),
                request.workflow_id.to_owned(),
                request.node_id.to_owned(),
                request.tools,
            ));
            Ok(())
        });

        assert_eq!(
            granted,
            [(
                "c-jira".to_owned(),
                "agent-1".to_owned(),
                "wf".to_owned(),
                "validator".to_owned(),
                ToolSelection::Only {
                    tools: vec!["getJiraIssue".to_owned()]
                }
            )]
        );
        assert!(plan[0].granted);
        assert!(!plan[1].granted);
    }

    #[test]
    fn a_step_without_an_agent_gets_no_grant_and_a_refusal_is_recorded() {
        let mut plan = resolve(
            &[ToolSuggestion {
                node_id: "validator".to_owned(),
                ..suggestion(ToolCapability::Issues)
            }],
            &[connection("jira", true, Some(&["getJiraIssue"]))],
        );

        apply::<()>(&workflow_with(""), &mut plan, |_| {
            panic!("no agent: nothing to grant")
        });
        assert!(!plan[0].granted);

        apply::<()>(&workflow_with("agent-1"), &mut plan, |_| Err(()));
        assert_eq!(plan[0].reason, Some(ToolUnavailable::Refused));
        assert!(!plan[0].granted);
    }
}
