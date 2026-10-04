use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The facts a condition may look at: a flat map from a known field name to its value. Built
/// from a step's structured result. Conditions never see anything else, and they cannot run code.
pub type Facts = BTreeMap<String, String>;

/// Fields a condition may name. Anything else is rejected when the workflow is validated, so a
/// typo is an error the user sees, not an edge that silently never fires.
pub const KNOWN_FIELDS: [&str; 7] = [
    "result.status",
    "result.outcome",
    "result.summary",
    "result.next_action",
    "result.findings",
    "result.matched",
    "validation.status",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    Exists,
    NotExists,
}

impl ConditionOperator {
    fn needs_value(self) -> bool {
        matches!(self, Self::Equals | Self::NotEquals)
    }
}

/// A structured, closed test of one fact: `result.status equals fail`. Deliberately not an
/// expression language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub operator: ConditionOperator,
    #[serde(default)]
    pub value: Option<String>,
}

/// Why a condition cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionError {
    UnknownField,
    MissingValue,
}

impl Condition {
    pub fn equals(field: &str, value: &str) -> Self {
        Self {
            field: field.to_owned(),
            operator: ConditionOperator::Equals,
            value: Some(value.to_owned()),
        }
    }

    /// # Errors
    ///
    /// Fails if the field is not one conditions may read, or a comparison has nothing to
    /// compare with.
    pub fn validate(&self) -> Result<(), ConditionError> {
        if !KNOWN_FIELDS.contains(&self.field.as_str()) {
            return Err(ConditionError::UnknownField);
        }
        if self.operator.needs_value() && self.value.as_deref().is_none_or(|v| v.trim().is_empty())
        {
            return Err(ConditionError::MissingValue);
        }
        Ok(())
    }

    /// Whether the condition holds for these facts. A missing (or empty) fact equals nothing, so
    /// `not_equals` holds for it.
    pub fn evaluate(&self, facts: &Facts) -> bool {
        let fact = facts.get(&self.field).filter(|v| !v.is_empty());
        let expected = self.value.as_deref().unwrap_or_default();
        match self.operator {
            ConditionOperator::Equals => fact.is_some_and(|v| v.eq_ignore_ascii_case(expected)),
            ConditionOperator::NotEquals => !fact.is_some_and(|v| v.eq_ignore_ascii_case(expected)),
            ConditionOperator::Exists => fact.is_some(),
            ConditionOperator::NotExists => fact.is_none(),
        }
    }
}
