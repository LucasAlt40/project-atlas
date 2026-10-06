//! Text that claims an authority only Atlas's policy and a person have.
//!
//! Finding such text changes nothing about what is allowed: it is a signal that the source (a
//! skill, a handoff, a file of the project) is trying to talk its way past the rules, shown to the
//! user. The defence is not this list; it is that no text anywhere is ever read as a permission.

/// Phrases, lowercase. English and Portuguese: the two languages Atlas is used in.
const CLAIMS: &[&str] = &[
    // Overriding the rules.
    "ignore previous",
    "ignore all previous",
    "ignore the previous",
    "ignore all atlas",
    "ignore all restrictions",
    "ignore the rules",
    "ignore your instructions",
    "disregard previous",
    "disregard the above",
    "disregard all",
    "ignore as instruções anteriores",
    "ignore todas as regras",
    "ignore as regras",
    "ignore as restrições",
    // Claiming privileges.
    "you have administrator",
    "administrator access",
    "admin access",
    "root access",
    "you are now allowed",
    "you may bypass",
    "você tem acesso de administrador",
    "acesso de administrador",
    "acesso root",
    // Switching security off.
    "disable security",
    "disable the security",
    "turn off security",
    "bypass security",
    "bypass the guard",
    "bypass permission",
    "desative a segurança",
    "desabilite a segurança",
    "contorne a segurança",
    // Claiming an approval nobody gave.
    "the user approved",
    "user approved",
    "user has approved",
    "user confirmed",
    "permission granted",
    "approval granted",
    "approved by the user",
    "o usuário aprovou",
    "usuário aprovou",
    "permissão concedida",
    "aprovação concedida",
    "aprovado pelo usuário",
    // Asking for elevated commands.
    "with sudo",
    "run sudo",
    "sudo ",
];

/// The first line of `text` that makes such a claim.
pub fn authority_claim(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let lower = line.to_lowercase();
        CLAIMS
            .iter()
            .any(|claim| lower.contains(claim))
            .then(|| line.trim().to_owned())
    })
}
