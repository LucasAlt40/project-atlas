//! Secrets of the mobile access: where they come from and how they are kept.
//!
//! Everything is drawn from the operating system's CSPRNG. A secret is shown once and only its
//! SHA-256 is remembered, so the table of sessions in memory is not itself a way in.

use sha2::{Digest, Sha256};

/// RFC 4648 base32: 32 symbols, so every character carries exactly 5 bits.
const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
/// 16 characters × 5 bits = 80 bits.
const CODE_LEN: usize = 16;

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0_u8; N];
    // The OS refusing to give randomness is not something to continue past: a predictable
    // secret would be worse than no server.
    getrandom::fill(&mut bytes).expect("the operating system's random source is unavailable");
    bytes
}

/// A session token: 256 random bits as 64 hex characters.
pub fn session_token() -> String {
    hex(&random::<32>())
}

/// A pairing code: 80 random bits as 16 base32 characters, unformatted.
pub fn pairing_code() -> String {
    random::<CODE_LEN>()
        .iter()
        // 256 is a multiple of 32, so the modulo adds no bias.
        .map(|byte| char::from(ALPHABET[usize::from(byte % 32)]))
        .collect()
}

/// A pairing code as a person reads and types it: `ABCD-EFGH-IJKL-MNOP`.
pub fn display_code(code: &str) -> String {
    code.as_bytes()
        .chunks(4)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

/// What a person typed, as the code it stands for: upper case, without the separators people add.
/// Anything outside the alphabet is kept (and so fails the lookup) rather than guessed at.
pub fn normalize_code(typed: &str) -> String {
    typed
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// The form a secret is remembered in.
pub fn digest(secret: &str) -> String {
    hex(&Sha256::digest(secret.as_bytes()))
}

/// A short opaque id for a device (not a secret: it names the device in the desktop list).
pub fn device_id() -> String {
    format!("dev-{}", hex(&random::<6>()))
}

/// A per-start identifier: a stream cursor from another start must not be trusted.
pub fn epoch() -> String {
    hex(&random::<6>())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn a_session_token_has_256_bits_and_is_never_repeated() {
        let tokens: HashSet<String> = (0..200).map(|_| session_token()).collect();
        assert_eq!(tokens.len(), 200);
        assert!(tokens
            .iter()
            .all(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit())));
    }

    #[test]
    fn a_pairing_code_has_80_bits_from_the_alphabet() {
        let code = pairing_code();
        assert_eq!(code.len(), 16);
        assert!(code.bytes().all(|b| ALPHABET.contains(&b)));
        let codes: HashSet<String> = (0..200).map(|_| pairing_code()).collect();
        assert_eq!(codes.len(), 200);
    }

    #[test]
    fn typing_a_code_the_way_people_do_finds_the_same_code() {
        let code = pairing_code();
        let shown = display_code(&code);
        assert_eq!(shown.matches('-').count(), 3);
        assert_eq!(normalize_code(&shown), code);
        assert_eq!(
            normalize_code(&shown.to_lowercase().replace('-', " ")),
            code
        );
    }

    #[test]
    fn only_a_digest_of_a_secret_is_kept() {
        let token = session_token();
        let kept = digest(&token);
        assert_eq!(kept.len(), 64);
        assert!(!kept.contains(&token));
        assert_eq!(kept, digest(&token));
        assert_ne!(kept, digest("another"));
    }
}
