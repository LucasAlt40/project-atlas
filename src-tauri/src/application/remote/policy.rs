//! What a phone may do (ADR 0034 §8). One place decides, so the page, the API and the tests all
//! say the same thing.
//!
//! The rule behind the table: a phone may *narrow* what an agent is allowed to do, and answer what
//! a person is asked in order to carry on; it may not *widen* authority. Anything that grants a
//! permission, lets context go to a model, or applies code stays at the computer.

use serde::Serialize;

use crate::domain::interaction::{
    DetectionSource, InteractionAnswer, InteractionKind, PendingInteraction,
};

/// Why a question is not answered from the phone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopOnly {
    /// Atlas's own guardrail asks what context may go to a model.
    Guardrail,
    /// A permission or confirmation asks for a grant; only refusing is allowed from here.
    GrantsAuthority,
}

/// What the phone may send for one question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRule {
    /// The answer can be given from the phone at all.
    pub can_answer: bool,
    /// For a decision: the option ids the phone may send. `None`: any of the question's options
    /// (and free text for a clarification).
    pub choices: Option<Vec<String>>,
    pub desktop_only: Option<DesktopOnly>,
}

pub fn rule_for(interaction: &PendingInteraction) -> RemoteRule {
    if interaction.source == DetectionSource::Guardrail {
        return RemoteRule {
            can_answer: false,
            choices: Some(Vec::new()),
            desktop_only: Some(DesktopOnly::Guardrail),
        };
    }
    let refusal = match interaction.kind {
        InteractionKind::Permission => Some("deny"),
        InteractionKind::RuntimeConfirmation => Some("no"),
        InteractionKind::Clarification | InteractionKind::Approval | InteractionKind::Plan => None,
    };
    match refusal {
        Some(only) => RemoteRule {
            can_answer: interaction.options.iter().any(|o| o.id == only),
            choices: Some(vec![only.to_owned()]),
            desktop_only: Some(DesktopOnly::GrantsAuthority),
        },
        None => RemoteRule {
            can_answer: true,
            choices: None,
            desktop_only: None,
        },
    }
}

/// Whether this answer may come from a phone.
pub fn allows(interaction: &PendingInteraction, answer: &InteractionAnswer) -> bool {
    let rule = rule_for(interaction);
    if !rule.can_answer {
        return false;
    }
    match (&rule.choices, answer.choice.as_deref().map(str::trim)) {
        (None, _) => true,
        (Some(allowed), Some(choice)) => allowed.iter().any(|a| a == choice),
        (Some(_), None) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::interaction::{decision_options, InteractionStatus};

    fn question(kind: InteractionKind, source: DetectionSource) -> PendingInteraction {
        PendingInteraction {
            id: "i1".into(),
            execution_id: "e1".into(),
            workflow_id: "wf".into(),
            workflow_execution_id: "r1".into(),
            workspace_id: "ws".into(),
            step_id: "dev".into(),
            step_label: "Developer".into(),
            agent_id: "a1".into(),
            iteration: 1,
            kind,
            question: "?".into(),
            context: String::new(),
            document: String::new(),
            options: decision_options(kind),
            source,
            confidence: 100,
            status: InteractionStatus::Pending,
            created_at: 1,
            answered_at: None,
            choice: None,
            answer: None,
            evaluation: None,
            auto: false,
        }
    }

    fn pick(choice: &str) -> InteractionAnswer {
        InteractionAnswer {
            choice: Some(choice.into()),
            text: None,
        }
    }

    #[test]
    fn plans_questions_and_approvals_can_be_answered_from_the_phone() {
        for kind in [
            InteractionKind::Plan,
            InteractionKind::Clarification,
            InteractionKind::Approval,
        ] {
            let q = question(kind, DetectionSource::Structured);
            assert!(rule_for(&q).can_answer, "{kind:?}");
            assert!(rule_for(&q).desktop_only.is_none());
        }
        let plan = question(InteractionKind::Plan, DetectionSource::Structured);
        assert!(allows(&plan, &pick("approve")));
        assert!(allows(&plan, &pick("revise")));
    }

    #[test]
    fn a_phone_may_refuse_a_permission_but_never_grant_it() {
        let q = question(InteractionKind::Permission, DetectionSource::Structured);
        assert!(rule_for(&q).can_answer);
        assert!(allows(&q, &pick("deny")));
        assert!(!allows(&q, &pick("allow")));
        assert!(!allows(&q, &InteractionAnswer::default()));
        let confirm = question(
            InteractionKind::RuntimeConfirmation,
            DetectionSource::Adapter,
        );
        assert!(allows(&confirm, &pick("no")));
        assert!(!allows(&confirm, &pick("yes")));
        assert_eq!(
            rule_for(&confirm).desktop_only,
            Some(DesktopOnly::GrantsAuthority)
        );
    }

    #[test]
    fn a_guardrail_question_is_for_the_computer_only() {
        for kind in [InteractionKind::Approval, InteractionKind::Clarification] {
            let q = question(kind, DetectionSource::Guardrail);
            let rule = rule_for(&q);
            assert!(!rule.can_answer);
            assert_eq!(rule.desktop_only, Some(DesktopOnly::Guardrail));
            assert!(!allows(&q, &pick("approve")));
            assert!(!allows(
                &q,
                &InteractionAnswer {
                    choice: None,
                    text: Some("ok".into())
                }
            ));
        }
    }
}
