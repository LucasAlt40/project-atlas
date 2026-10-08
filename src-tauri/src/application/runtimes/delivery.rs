//! What a runtime adapter hands to its process, as one value.
//!
//! The adapter builds the payload in exactly one place, [`ModelRuntime::delivery`], and both the
//! launch (`execute`) and the Context Manifest's hash read it from there. A payload that was
//! hashed is therefore the payload that was delivered, by construction; a test with a spy
//! `ProcessRunner` checks it against what the process port really received.
//!
//! Two shapes, by what the runtime offers ([`SystemPromptChannel`]):
//!
//! - **`Unsupported`** (every runtime today): one text, the whole prompt, Atlas's instructions as
//!   its first labelled section. How it travels (stdin, last argument, `--prompt=`) is decided at
//!   launch and does not change the text.
//! - **`Native` / `Appended`**: Atlas's instructions and the rules (authoritative) on the system
//!   channel, everything else in the body. The hash then covers both parts, framed so that moving
//!   text from one to the other changes it.
//!
//! [`ModelRuntime::delivery`]: super::ModelRuntime::delivery

use sha2::{Digest, Sha256};

use crate::application::prompt::Prompt;
use crate::domain::context::{DeliveryRecord, Figure};
use crate::domain::runtime::SystemPromptChannel;

/// The prompt as the runtime will receive it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    channel: SystemPromptChannel,
    /// What goes on the system channel; only when the runtime has one.
    system: Option<String>,
    body: String,
}

impl Delivery {
    /// The prompt Atlas built, whole, as one text.
    pub fn of_prompt(prompt: &Prompt) -> Self {
        Self {
            channel: SystemPromptChannel::Unsupported,
            system: None,
            body: prompt.combined(),
        }
    }

    /// The prompt as `channel` takes it: whole for a runtime with no system channel (it is never
    /// pretended to have one), split for one that has.
    pub fn for_channel(prompt: &Prompt, channel: SystemPromptChannel) -> Self {
        if !channel.is_separate() {
            return Self::of_prompt(prompt);
        }
        Self {
            channel,
            system: Some(prompt.system_channel_text()),
            body: prompt.body_without_system(),
        }
    }

    /// The system channel's text, when the runtime has one. For an adapter that really has such a
    /// channel (none does yet) and for the tests that prove the split.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn system(&self) -> Option<&str> {
        self.system.as_deref()
    }

    /// The prompt body: the whole prompt when there is no system channel.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn body(&self) -> &str {
        &self.body
    }

    /// The whole prompt as one text, for an adapter that has no system channel. A split delivery
    /// is joined back with its labels, so nothing is lost; the hash still tells the two apart.
    pub fn into_payload(self) -> String {
        match self.system {
            None => self.body,
            Some(system) => format!("SYSTEM / PERSONALITY\n\n{system}\n\n{}", self.body),
        }
    }

    /// `sha256:<hex>` of what is delivered. With no system channel, the bytes of the one text
    /// (UTF-8, exactly as written to the process); with one, the two parts framed by their
    /// lengths.
    pub fn prompt_hash(&self) -> String {
        match &self.system {
            None => sha256_hex(self.body.as_bytes()),
            Some(system) => sha256_hex(
                format!(
                    "atlas.delivery.split.v1\nsystem:{}\n{system}\nbody:{}\n{}",
                    system.len(),
                    self.body.len(),
                    self.body
                )
                .as_bytes(),
            ),
        }
    }

    pub fn record(&self, delivered: bool) -> DeliveryRecord {
        let system_bytes = self.system.as_ref().map_or(0, String::len);
        let chars =
            self.body.chars().count() + self.system.as_ref().map_or(0, |s| s.chars().count());
        DeliveryRecord {
            delivered,
            prompt_hash: self.prompt_hash(),
            bytes: self.body.len() + system_bytes,
            chars,
            estimated_tokens: Figure::estimated_from_chars(chars),
            system_channel: self.channel,
            system_bytes,
        }
    }
}

/// `sha256:` followed by the lowercase hex digest.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(instruction: &str) -> Prompt {
        Prompt {
            system: "SYS".to_owned(),
            harness: None,
            task_aware: false,
            skills: None,
            rules: None,
            context: "CTX".to_owned(),
            instruction: instruction.to_owned(),
        }
    }

    #[test]
    fn the_hash_is_a_real_sha256() {
        // NIST test vectors.
        assert_eq!(
            sha256_hex(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_hash_is_deterministic_and_follows_every_byte() {
        let a = Delivery::of_prompt(&prompt("one"));

        assert_eq!(
            a.prompt_hash(),
            Delivery::of_prompt(&prompt("one")).prompt_hash()
        );
        assert_ne!(
            a.prompt_hash(),
            Delivery::of_prompt(&prompt("two")).prompt_hash()
        );
        // One character is enough.
        assert_ne!(
            a.prompt_hash(),
            Delivery::of_prompt(&prompt("one ")).prompt_hash()
        );
    }

    #[test]
    fn a_runtime_without_a_system_channel_gets_the_whole_prompt_and_is_not_given_one() {
        let whole = Delivery::for_channel(&prompt("INS"), SystemPromptChannel::Unsupported);

        assert_eq!(whole.system(), None);
        assert_eq!(whole.body(), prompt("INS").combined());
        assert_eq!(whole, Delivery::of_prompt(&prompt("INS")));
        let record = whole.record(true);
        assert_eq!(record.system_channel, SystemPromptChannel::Unsupported);
        assert_eq!(record.system_bytes, 0);
    }

    #[test]
    fn a_runtime_with_a_system_channel_gets_the_authoritative_text_there_and_the_rest_in_the_body()
    {
        let mut with_rules = prompt("INS");
        with_rules.rules = Some("[MANDATORY · Project] Write tests".to_owned());
        with_rules.harness = Some("harness".to_owned());

        let split = Delivery::for_channel(&with_rules, SystemPromptChannel::Appended);

        let system = split.system().unwrap();
        assert!(system.starts_with("SYS"));
        assert!(system.contains("[MANDATORY · Project] Write tests"));
        // Knowledge and the task are not authoritative: they stay in the body.
        assert!(!system.contains("harness") && !system.contains("INS"));
        assert!(split.body().contains("harness") && split.body().contains("INS"));
        assert!(!split.body().contains("SYS"));
        let record = split.record(true);
        assert_eq!(record.system_channel, SystemPromptChannel::Appended);
        assert_eq!(record.system_bytes, system.len());
        assert_eq!(record.bytes, system.len() + split.body().len());
    }

    #[test]
    fn moving_text_between_the_system_channel_and_the_body_changes_the_hash() {
        let a = Delivery {
            channel: SystemPromptChannel::Native,
            system: Some("ab".to_owned()),
            body: "cd".to_owned(),
        };
        let b = Delivery {
            channel: SystemPromptChannel::Native,
            system: Some("a".to_owned()),
            body: "bcd".to_owned(),
        };

        assert_ne!(a.prompt_hash(), b.prompt_hash());
    }

    #[test]
    fn the_record_counts_bytes_and_characters_separately() {
        let delivery = Delivery {
            channel: SystemPromptChannel::Unsupported,
            system: None,
            body: "ação".to_owned(),
        };

        let record = delivery.record(true);

        assert_eq!((record.bytes, record.chars), (6, 4));
        assert_eq!(record.estimated_tokens.value, Some(1));
        assert!(record.delivered);
        assert!(!delivery.record(false).delivered);
    }
}
