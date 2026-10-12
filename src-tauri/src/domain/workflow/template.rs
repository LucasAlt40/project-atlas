//! What a workflow template says about the tools (MCP integrations) its steps may use, and the
//! templates a person saved. A template never authorizes anything: it *suggests* tools, and a
//! suggestion becomes a grant only when the person starts the workflow with that option on.

use serde::{Deserialize, Serialize};

use super::edge::WorkflowEdge;
use super::node::WorkflowNode;
use crate::domain::mcp::ToolSelection;

/// A kind of tool a step is better off with. A template names the kind, not a product: whichever
/// connection of the workspace matches is the one suggested, and a workspace without one simply
/// does not get it (the preview says so).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
    /// Opening and inspecting a page: console, network, DOM. The tools act on a browser, so the
    /// whole server is suggested.
    Browser,
    /// Reading the ticket a task refers to.
    Issues,
    /// Reading the documentation of a library or API.
    Docs,
    /// Reading the history and the changes of the repository on a hosting service.
    Repo,
}

impl ToolCapability {
    /// The connection names Atlas recognizes for this kind. Matched against the name the person
    /// gave the connection (equal, or containing it); a guess by name, never a claim about what
    /// the server does, so it only ever *suggests*.
    pub fn known_names(self) -> &'static [&'static str] {
        match self {
            Self::Browser => &["chrome-devtools", "playwright", "puppeteer", "browser"],
            Self::Issues => &["jira", "linear", "atlassian", "issues"],
            Self::Docs => &["context7", "docs", "documentation"],
            Self::Repo => &["github", "gitlab"],
        }
    }

    /// Whether the step only *consults* what the tool offers. Then the suggestion covers the
    /// reading tools of the server (when they are known) and never the ones that write.
    pub fn consults(self) -> bool {
        !matches!(self, Self::Browser)
    }
}

/// What a suggestion points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SuggestionTarget {
    /// Any connection of the workspace that matches the kind.
    Capability { capability: ToolCapability },
    /// A connection by name, with the tools it was granted (a saved template remembers this).
    Connection { name: String, tools: ToolSelection },
}

/// A tool a step may be given, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSuggestion {
    /// The step (the node's id, which is the same in every workflow made from the template).
    pub node_id: String,
    pub target: SuggestionTarget,
    /// When and why the step would use it, shown to the person.
    #[serde(default)]
    pub purpose: String,
}

/// A template the person saved from a workflow. Built-in templates are code and are never
/// touched; a saved one is a copy of a workflow's steps and routes with the tools it had.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// The built-in template it started from, when it did.
    #[serde(default)]
    pub based_on: Option<String>,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    #[serde(default)]
    pub tools: Vec<ToolSuggestion>,
    pub created_at: u64,
    pub updated_at: u64,
}
