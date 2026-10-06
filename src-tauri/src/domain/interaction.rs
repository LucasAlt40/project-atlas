//! Human-in-the-loop: what it means for an agent to be waiting for a person.
//!
//! A *pending interaction* is business state of Atlas, not a transport detail: whether a runtime
//! has a terminal that accepts input (`RuntimeCapabilities::terminal_input`) says nothing about
//! whether an agent needs an answer. An execution that waits for a person has not completed and
//! has not failed; it is `waiting_for_input`, and its step has no outcome yet.
//!
//! Everything an agent says is data. A detection can pause a step; an answer can only be handed
//! back to the agent as text. Neither changes a permission, a policy or the workflow.

use serde::{Deserialize, Serialize};

/// Longest question kept.
pub const MAX_QUESTION: usize = 600;
/// Longest context kept.
pub const MAX_CONTEXT: usize = 800;
/// Longest document (the agent's whole message, often a plan in Markdown) kept.
pub const MAX_DOCUMENT: usize = 20_000;
/// Most options an agent can offer.
pub const MAX_OPTIONS: usize = 6;
/// Longest option label kept.
pub const MAX_OPTION: usize = 80;
/// Longest free-text answer accepted.
pub const MAX_ANSWER: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionKind {
    /// The agent needs information to go on: free text (possibly with suggestions).
    Clarification,
    /// The agent knows what to do and wants a yes or no before it does.
    Approval,
    /// The agent wants to do something its policy would ask about. Answering tells the agent;
    /// the policy stays the source of truth for what is really allowed.
    Permission,
    /// The runtime itself asked ("Apply these changes?"). Shown as the runtime asked it.
    RuntimeConfirmation,
}

impl InteractionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clarification => "clarification",
            Self::Approval => "approval",
            Self::Permission => "permission",
            Self::RuntimeConfirmation => "runtime_confirmation",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str()
        {
            "clarification" | "question" | "input" => Some(Self::Clarification),
            "approval" | "approve" | "confirm" | "confirmation" => Some(Self::Approval),
            "permission" | "permission_request" => Some(Self::Permission),
            "runtime_confirmation" | "runtimeconfirmation" => Some(Self::RuntimeConfirmation),
            _ => None,
        }
    }

    /// Whether the answer is a yes or a no (as opposed to text).
    pub fn is_decision(self) -> bool {
        !matches!(self, Self::Clarification)
    }
}

/// What the person may pick. For decisions Atlas fixes the pair; the agent cannot invent the
/// buttons of an approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionOption {
    /// What is sent back (`approve`, `reject`, `allow`, `deny`, or a suggestion's text).
    pub id: String,
    pub label: String,
}

impl InteractionOption {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_owned(),
            label: label.to_owned(),
        }
    }
}

/// The yes/no pair of a decision kind.
pub fn decision_options(kind: InteractionKind) -> Vec<InteractionOption> {
    match kind {
        InteractionKind::Permission => vec![
            InteractionOption::new("allow", "Allow"),
            InteractionOption::new("deny", "Deny"),
        ],
        InteractionKind::Approval => vec![
            InteractionOption::new("approve", "Approve"),
            InteractionOption::new("reject", "Reject"),
        ],
        InteractionKind::RuntimeConfirmation => vec![
            InteractionOption::new("yes", "Yes"),
            InteractionOption::new("no", "No"),
        ],
        InteractionKind::Clarification => Vec::new(),
    }
}

/// Whether the option id means "no" (the agent is told it may not).
pub fn is_negative(option_id: &str) -> bool {
    matches!(option_id, "reject" | "deny" | "no")
}

/// How the detection was made, from the most to the least reliable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionSource {
    /// A structured signal: the block the agent was asked to end its answer with.
    Structured,
    /// The runtime adapter recognised its own tool's way of asking.
    Adapter,
    /// Text analysis, conservative.
    Heuristic,
    /// Atlas's own guardrail asked, before the agent started. The agent said nothing.
    Guardrail,
}

/// What the detector concluded about an execution's output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionDetection {
    pub detected: bool,
    pub kind: Option<InteractionKind>,
    /// 0 to 100. A structured block is 100; text analysis never is.
    pub confidence: u8,
    pub question: String,
    pub context: String,
    /// What the agent wrote before asking, as Markdown (a plan, an analysis…), to be read in full.
    #[serde(default)]
    pub document: String,
    pub options: Vec<InteractionOption>,
    pub source: DetectionSource,
    /// The guardrail evaluation this question is about (its fingerprint), for Atlas's own
    /// questions: an answer approves that evaluation and nothing else.
    #[serde(default)]
    pub evaluation: Option<String>,
}

impl InteractionDetection {
    pub fn none(source: DetectionSource) -> Self {
        Self {
            detected: false,
            kind: None,
            confidence: 0,
            question: String::new(),
            context: String::new(),
            document: String::new(),
            options: Vec::new(),
            source,
            evaluation: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionStatus {
    Pending,
    Answered,
    Cancelled,
    /// Reserved: a timeout is not applied in this version, but the state exists so adding one
    /// does not change the model.
    Expired,
}

/// A question to a person that blocks one step of one run. At most one is pending per step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingInteraction {
    pub id: String,
    /// The execution (of the step's attempt) that asked.
    pub execution_id: String,
    pub workflow_id: String,
    pub workflow_execution_id: String,
    pub workspace_id: String,
    pub step_id: String,
    pub step_label: String,
    pub agent_id: String,
    /// The pass through the step it was asked in; a loop starts a new one.
    pub iteration: u32,
    pub kind: InteractionKind,
    pub question: String,
    pub context: String,
    #[serde(default)]
    pub document: String,
    pub options: Vec<InteractionOption>,
    pub source: DetectionSource,
    pub confidence: u8,
    pub status: InteractionStatus,
    pub created_at: u64,
    #[serde(default)]
    pub answered_at: Option<u64>,
    /// The option id picked, for a decision or a suggestion.
    #[serde(default)]
    pub choice: Option<String>,
    /// The free text the person wrote.
    #[serde(default)]
    pub answer: Option<String>,
    /// The guardrail evaluation the question is bound to (see [`InteractionDetection`]).
    #[serde(default)]
    pub evaluation: Option<String>,
}

impl PendingInteraction {
    pub fn is_pending(&self) -> bool {
        self.status == InteractionStatus::Pending
    }

    /// Whether the person said no.
    pub fn was_declined(&self) -> bool {
        self.choice.as_deref().is_some_and(is_negative)
    }
}

/// What the person sent back, as it arrives from the webview.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionAnswer {
    pub choice: Option<String>,
    pub text: Option<String>,
}

/// Why an answer was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerProblem {
    NotFound,
    /// Already answered, cancelled or expired.
    NotPending,
    /// Empty, too long, or not one of the options of a decision.
    Invalid,
}

/// Checks an answer against the question it is for and normalizes it to `(choice, text)`.
/// A decision accepts only its own options; a clarification accepts text (and suggestions).
pub fn validate_answer(
    interaction: &PendingInteraction,
    answer: &InteractionAnswer,
) -> Result<(Option<String>, Option<String>), AnswerProblem> {
    if !interaction.is_pending() {
        return Err(AnswerProblem::NotPending);
    }
    let text = answer
        .text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    if text.is_some_and(|t| t.chars().count() > MAX_ANSWER) {
        return Err(AnswerProblem::Invalid);
    }
    let choice = answer
        .choice
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if let Some(choice) = choice {
        if !interaction.options.iter().any(|o| o.id == choice) {
            return Err(AnswerProblem::Invalid);
        }
    }
    if interaction.kind.is_decision() {
        // A decision is a pick between its options; a note may come with it.
        return choice
            .map(|c| (Some(c.to_owned()), text.map(str::to_owned)))
            .ok_or(AnswerProblem::Invalid);
    }
    if choice.is_none() && text.is_none() {
        return Err(AnswerProblem::Invalid);
    }
    Ok((choice.map(str::to_owned), text.map(str::to_owned)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(kind: InteractionKind) -> PendingInteraction {
        PendingInteraction {
            id: "i1".into(),
            execution_id: "e1".into(),
            workflow_id: "wf".into(),
            workflow_execution_id: "w1".into(),
            workspace_id: "ws".into(),
            step_id: "dev".into(),
            step_label: "Developer".into(),
            agent_id: "a".into(),
            iteration: 1,
            kind,
            question: "Which API?".into(),
            context: String::new(),
            document: String::new(),
            options: decision_options(kind),
            source: DetectionSource::Structured,
            confidence: 100,
            status: InteractionStatus::Pending,
            created_at: 1,
            answered_at: None,
            choice: None,
            answer: None,
            evaluation: None,
        }
    }

    #[test]
    fn a_clarification_takes_text_and_nothing_empty() {
        let i = pending(InteractionKind::Clarification);
        let text = InteractionAnswer {
            choice: None,
            text: Some("/api/users".into()),
        };
        assert_eq!(
            validate_answer(&i, &text),
            Ok((None, Some("/api/users".into())))
        );
        assert_eq!(
            validate_answer(&i, &InteractionAnswer::default()),
            Err(AnswerProblem::Invalid)
        );
    }

    #[test]
    fn a_decision_takes_only_its_own_options() {
        let i = pending(InteractionKind::Approval);
        let ok = InteractionAnswer {
            choice: Some("approve".into()),
            text: None,
        };
        assert_eq!(validate_answer(&i, &ok), Ok((Some("approve".into()), None)));
        let invented = InteractionAnswer {
            choice: Some("apply-to-main".into()),
            text: None,
        };
        assert_eq!(validate_answer(&i, &invented), Err(AnswerProblem::Invalid));
        // Free text alone is not a decision.
        let text = InteractionAnswer {
            choice: None,
            text: Some("sure".into()),
        };
        assert_eq!(validate_answer(&i, &text), Err(AnswerProblem::Invalid));
    }

    #[test]
    fn an_answered_interaction_cannot_be_answered_again() {
        let mut i = pending(InteractionKind::Clarification);
        i.status = InteractionStatus::Answered;
        let text = InteractionAnswer {
            choice: None,
            text: Some("x".into()),
        };
        assert_eq!(validate_answer(&i, &text), Err(AnswerProblem::NotPending));
    }
}
