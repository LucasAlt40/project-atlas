//! What the Context Review says about MCP: a connection granted to the step that could not be
//! given to it, and a policy that asks a person before MCP tools are handed out.
//!
//! It reads what planning decided (`application::mcp`) and decides nothing: whether a missing
//! connection stops the step is the finding's severity (a required connection is blocking), and
//! what a finding means for the execution is the guardrail's decision, in `security/`.

use super::issue;
use crate::domain::guardrail::{IssueCode, IssueSeverity, ReviewIssue};
use crate::domain::mcp::McpProblem;
use crate::domain::optimization::SectionKind;

/// A connection granted to this step that was not given to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpFailure {
    pub name: String,
    pub required: bool,
    pub problem: McpProblem,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpFacts {
    pub failures: Vec<McpFailure>,
    /// The agent's policy asks a person first, and this step would be given MCP tools.
    pub requires_approval: bool,
}

fn what(problem: &McpProblem) -> &'static str {
    match problem {
        McpProblem::NotEnabled => "it is switched off",
        McpProblem::NoGrant => "nothing grants it",
        McpProblem::PolicyDenied => "the agent's policy does not allow MCP",
        McpProblem::RuntimeUnsupported => "the agent's runtime has no MCP adapter",
        McpProblem::InvalidConfiguration { .. } => "its configuration is not valid",
        McpProblem::SecretMissing { .. } => "a secret it needs is not stored",
        McpProblem::NeedsDiscovery => "it was never discovered, so named tools cannot be held to",
    }
}

/// MCP is not a prompt section; the finding is attributed to the section the step's tools belong
/// to for the purposes of the review, the context as a whole.
pub fn check(facts: &McpFacts) -> Vec<ReviewIssue> {
    let mut issues = Vec::new();
    for failure in &facts.failures {
        let (severity, kind) = if failure.required {
            (IssueSeverity::Blocking, "required")
        } else {
            (IssueSeverity::Warning, "optional")
        };
        issues.push(issue(
            IssueCode::McpUnavailable,
            severity,
            SectionKind::Framing,
            format!(
                "the {kind} MCP connection {} cannot be given to this step: {}",
                failure.name,
                what(&failure.problem)
            ),
        ));
    }
    if facts.requires_approval {
        issues.push(issue(
            IssueCode::McpApprovalRequired,
            IssueSeverity::Error,
            SectionKind::Framing,
            "the agent's policy asks a person before a step is given MCP tools",
        ));
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(required: bool, problem: McpProblem) -> McpFailure {
        McpFailure {
            name: "files".to_owned(),
            required,
            problem,
        }
    }

    #[test]
    fn a_required_connection_that_cannot_be_given_blocks_and_an_optional_one_warns() {
        let facts = McpFacts {
            failures: vec![
                failure(
                    true,
                    McpProblem::SecretMissing {
                        name: "T".to_owned(),
                    },
                ),
                failure(false, McpProblem::RuntimeUnsupported),
            ],
            requires_approval: false,
        };

        let issues = check(&facts);

        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].severity, IssueSeverity::Blocking);
        assert!(issues[0].message.contains("required") && issues[0].message.contains("secret"));
        assert_eq!(issues[1].severity, IssueSeverity::Warning);
        assert!(issues.iter().all(|i| i.code == IssueCode::McpUnavailable));
    }

    #[test]
    fn a_policy_that_asks_is_an_error_for_a_person() {
        let issues = check(&McpFacts {
            failures: vec![],
            requires_approval: true,
        });

        assert_eq!(issues[0].code, IssueCode::McpApprovalRequired);
        assert_eq!(issues[0].severity, IssueSeverity::Error);
    }

    #[test]
    fn nothing_to_report_is_nothing() {
        assert_eq!(check(&McpFacts::default()).len(), 0);
    }
}
