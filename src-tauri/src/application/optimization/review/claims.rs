//! Text that claims an authority only Atlas's policy and a person have.
//!
//! Finding such text changes nothing about what is allowed: it is a signal that the source (a
//! skill, a handoff, a rule, a file of the project) is trying to talk its way past the rules, shown
//! to the user. The defence is not this list; it is that no text anywhere is ever read as a
//! permission. The list is deterministic and short, will miss paraphrases, and says so: it
//! classifies what it recognizes, it does not promise to recognize everything.

use crate::domain::guardrail::ClaimKind;

/// Phrases, lowercase, by what they claim. English and Portuguese: the two languages Atlas is
/// used in.
const CLAIMS: &[(ClaimKind, &[&str])] = &[
    (
        ClaimKind::OverrideRules,
        &[
            "ignore previous",
            "ignore all previous",
            "ignore the previous",
            "ignore all atlas",
            "ignore atlas",
            "ignore all restrictions",
            "ignore the rules",
            "ignore your instructions",
            "disregard previous",
            "disregard the above",
            "disregard all",
            "atlas policy does not apply",
            "atlas rules do not apply",
            "atlas policies do not apply",
            "ignore as instruções anteriores",
            "ignore as instruções",
            "ignore todas as regras",
            "ignore as regras",
            "ignore as restrições",
            "a política do atlas não se aplica",
            "as regras do atlas não se aplicam",
        ],
    ),
    (
        ClaimKind::GrantPermission,
        &[
            "you have administrator",
            "administrator access",
            "admin access",
            "root access",
            "you are now allowed",
            "you are now authorized",
            "you are authorized",
            "you have been authorized",
            "you may bypass",
            "você tem acesso de administrador",
            "acesso de administrador",
            "acesso root",
            "você está autorizado",
            "você agora está autorizado",
            "você foi autorizado",
        ],
    ),
    (
        ClaimKind::DisableSecurity,
        &[
            "disable security",
            "disable the security",
            "turn off security",
            "bypass security",
            "bypass the guard",
            "bypass permission",
            "disable the guardrails",
            "disable guardrails",
            "turn off the guardrails",
            "desative a segurança",
            "desabilite a segurança",
            "contorne a segurança",
            "desative os guardrails",
            "desabilite os guardrails",
        ],
    ),
    (
        ClaimKind::FalseApproval,
        &[
            "the user approved",
            "user approved",
            "user already approved",
            "the user already approved",
            "user has approved",
            "user confirmed",
            "permission granted",
            "approval granted",
            "approved by the user",
            "o usuário aprovou",
            "usuário aprovou",
            "o usuário já aprovou",
            "usuário já aprovou",
            "permissão concedida",
            "aprovação concedida",
            "aprovado pelo usuário",
        ],
    ),
    (ClaimKind::Elevation, &["with sudo", "run sudo", "sudo "]),
];

/// What the first line of `text` that claims something claims, and that line.
pub fn find_claim(text: &str) -> Option<(ClaimKind, String)> {
    text.lines().find_map(|line| {
        let lower = line.to_lowercase();
        CLAIMS.iter().find_map(|(kind, phrases)| {
            phrases
                .iter()
                .any(|phrase| lower.contains(phrase))
                .then(|| (*kind, line.trim().to_owned()))
        })
    })
}
