//! The Harness is context, and context travels to a model. Nothing that looks like a secret may
//! be written to it or sent from it.

const SECRET_WORDS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "api-key",
    "private_key",
    "credential",
    "auth_key",
    "access_key",
];
const TOKEN_PREFIXES: &[&str] = &[
    "sk-",
    "ghp_",
    "gho_",
    "github_pat_",
    "xoxb-",
    "AKIA",
    "AIza",
];
const REDACTED: &str = "[redacted: looks like a secret]";

/// Replaces every line that looks like a secret with a notice. Returns the text and how many
/// lines were replaced.
pub fn redact_secrets(text: &str) -> (String, usize) {
    let mut count = 0;
    let lines: Vec<&str> = text
        .lines()
        .map(|line| {
            if looks_secret(line) {
                count += 1;
                REDACTED
            } else {
                line
            }
        })
        .collect();
    (lines.join("\n"), count)
}

fn looks_secret(line: &str) -> bool {
    if line.contains("-----BEGIN") && line.contains("PRIVATE KEY") {
        return true;
    }
    if line
        .split(|c: char| c.is_whitespace() || "\"'`(),;".contains(c))
        .any(|word| {
            TOKEN_PREFIXES.iter().any(|p| word.starts_with(p))
                && word.len() >= 16
                && word
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
        })
    {
        return true;
    }
    // `NAME=value` / `NAME: value`, where the name is one identifier that names a secret.
    let Some((name, value)) = line.split_once('=').or_else(|| line.split_once(": ")) else {
        return false;
    };
    // The assignment is the last word before the `=`, even in the middle of a sentence.
    let name = name
        .trim()
        .rsplit(char::is_whitespace)
        .next()
        .unwrap_or("")
        .trim_start_matches(|c: char| "-*`# ".contains(c))
        .trim_end_matches('`');
    let value = value.trim().trim_matches(['"', '\'', '`']);
    let identifier = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c));
    let lower = name.to_lowercase();
    // The first word of the value is what a secret would be; the rest may be prose.
    let first = value.split_whitespace().next().unwrap_or("");
    identifier && SECRET_WORDS.iter().any(|w| lower.contains(w)) && first.len() >= 6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_assignments_keys_and_tokens() {
        let (text, count) = redact_secrets(
            "ok line\nDB_PASSWORD=hunter2hunter2\n- api_key: abcdef123456\nuse ghp_abcdefghijklmnopqrstuv here\n-----BEGIN RSA PRIVATE KEY-----",
        );
        assert_eq!(count, 4);
        assert!(text.starts_with("ok line"));
        assert!(!text.contains("hunter2"));
        assert!(!text.contains("ghp_"));
    }

    #[test]
    fn leaves_ordinary_prose_alone() {
        let text = "Secrets are kept in a vault.\nToken: JWT is used for sessions\nPassword policy: see wiki";
        let (out, count) = redact_secrets(text);
        assert_eq!(count, 0);
        assert_eq!(out, text);
    }
}
