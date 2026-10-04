//! What an agent promises to say at the end of a step, so a workflow can route on it. A contract
//! is a list of outcomes the agent may declare (`pass`/`fail`, `approved`/`changes_requested`…).
//! The kinds are presets for the editor, nothing more: the workflow only ever sees outcome ids.

use serde::{Deserialize, Serialize};

pub const MAX_OUTCOMES: usize = 12;
pub const MAX_OUTCOME_ID: usize = 40;
const MAX_LABEL: usize = 60;
const MAX_DESCRIPTION: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContractKind {
    /// No outcome is required: the step's result is only a summary.
    #[default]
    General,
    Validation,
    Review,
    Implementation,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// Stable: edges refer to it. Lowercase letters, digits, `_` and `-`.
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
}

impl Outcome {
    fn new(id: &str, label: &str, description: &str) -> Self {
        Self {
            id: id.to_owned(),
            label: label.to_owned(),
            description: description.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResultContract {
    #[serde(default)]
    pub kind: ContractKind,
    #[serde(default)]
    pub outcomes: Vec<Outcome>,
}

/// Why a contract cannot be saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractError {
    /// An outcome id is empty, too long or has characters other than `a-z 0-9 _ -`.
    InvalidId,
    DuplicateId,
    EmptyLabel,
    TooMany,
    /// A contract that is not `general` needs at least one outcome (and `general` has none).
    WrongOutcomeCount,
}

impl ResultContract {
    pub fn general() -> Self {
        Self::default()
    }

    /// The outcomes a preset starts from. `Custom` starts empty.
    pub fn preset(kind: ContractKind) -> Self {
        let outcomes = match kind {
            ContractKind::General | ContractKind::Custom => Vec::new(),
            ContractKind::Validation => vec![
                Outcome::new("pass", "Pass", "Validation passed."),
                Outcome::new("fail", "Fail", "Validation failed."),
            ],
            ContractKind::Review => vec![
                Outcome::new("approved", "Approved", "The work is approved."),
                Outcome::new(
                    "changes_requested",
                    "Changes requested",
                    "The work needs changes before it can go on.",
                ),
            ],
            ContractKind::Implementation => vec![
                Outcome::new("implemented", "Implemented", "The work was done."),
                Outcome::new("partial", "Partial", "Only part of the work was done."),
                Outcome::new("blocked", "Blocked", "The work could not be done."),
            ],
        };
        Self { kind, outcomes }
    }

    /// The step must end with one of the declared outcomes.
    pub fn requires_outcome(&self) -> bool {
        !self.outcomes.is_empty()
    }

    /// The declared id equal to `text` (ignoring case and surrounding space), if any.
    pub fn declared(&self, text: &str) -> Option<&str> {
        let text = text.trim();
        self.outcomes
            .iter()
            .find(|o| o.id.eq_ignore_ascii_case(text))
            .map(|o| o.id.as_str())
    }

    /// Trims the text fields and checks the contract.
    ///
    /// # Errors
    ///
    /// See [`ContractError`].
    pub fn normalized(mut self) -> Result<Self, ContractError> {
        if self.outcomes.len() > MAX_OUTCOMES {
            return Err(ContractError::TooMany);
        }
        for outcome in &mut self.outcomes {
            outcome.id = outcome.id.trim().to_owned();
            outcome.label = outcome.label.trim().chars().take(MAX_LABEL).collect();
            outcome.description = outcome
                .description
                .trim()
                .chars()
                .take(MAX_DESCRIPTION)
                .collect();
            let valid_id = !outcome.id.is_empty()
                && outcome.id.len() <= MAX_OUTCOME_ID
                && outcome
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
            if !valid_id {
                return Err(ContractError::InvalidId);
            }
            if outcome.label.is_empty() {
                return Err(ContractError::EmptyLabel);
            }
        }
        for (index, outcome) in self.outcomes.iter().enumerate() {
            if self.outcomes[..index].iter().any(|o| o.id == outcome.id) {
                return Err(ContractError::DuplicateId);
            }
        }
        if (self.kind == ContractKind::General) != self.outcomes.is_empty() {
            return Err(ContractError::WrongOutcomeCount);
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_valid_contracts() {
        for kind in [
            ContractKind::General,
            ContractKind::Validation,
            ContractKind::Review,
            ContractKind::Implementation,
        ] {
            assert!(
                ResultContract::preset(kind).normalized().is_ok(),
                "{kind:?}"
            );
        }
        assert!(!ResultContract::general().requires_outcome());
        assert!(ResultContract::preset(ContractKind::Validation).requires_outcome());
    }

    #[test]
    fn declared_matches_ignoring_case_and_returns_the_stable_id() {
        let contract = ResultContract::preset(ContractKind::Validation);
        assert_eq!(contract.declared(" PASS "), Some("pass"));
        assert_eq!(contract.declared("success"), None);
    }

    #[test]
    fn duplicate_or_malformed_ids_are_refused() {
        let outcome = |id: &str| Outcome::new(id, "L", "");
        let custom = |ids: &[&str]| ResultContract {
            kind: ContractKind::Custom,
            outcomes: ids.iter().map(|id| outcome(id)).collect(),
        };
        assert_eq!(
            custom(&["a", "a"]).normalized(),
            Err(ContractError::DuplicateId)
        );
        assert_eq!(
            custom(&["Bad Id"]).normalized(),
            Err(ContractError::InvalidId)
        );
        assert_eq!(custom(&[""]).normalized(), Err(ContractError::InvalidId));
        assert_eq!(
            custom(&[]).normalized(),
            Err(ContractError::WrongOutcomeCount)
        );
        assert!(custom(&["blocked", "ok-1"]).normalized().is_ok());
        let general_with_outcomes = ResultContract {
            kind: ContractKind::General,
            outcomes: vec![outcome("x")],
        };
        assert_eq!(
            general_with_outcomes.normalized(),
            Err(ContractError::WrongOutcomeCount)
        );
    }
}
