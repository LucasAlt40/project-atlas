//! C3 of the ADR checklist (ADR 0036): does sending Atlas's instructions on the CLI's system
//! channel (`--append-system-prompt-file`) let the provider serve them from its cache? The same
//! small task, run three times back to back with the channel off and three times with it on,
//! through the whole of Atlas; the numbers are the runtime's own (`cachedInputTokens`).
//!
//! It costs tokens (six short runs) and runs only by exact name:
//! `cargo test --lib application::executions::system_channel_real_tests::system_channel_real_cache -- --ignored --exact --nocapture`.
//! `ATLAS_C3_MODEL` picks the Claude model (default `haiku`).

use super::real_support::{RealWorld, Setup};
use crate::domain::memory::MemorySettings;
use crate::domain::security::SandboxMode;

const TASK: &str = "Read README.md and reply with its first line, nothing else.";

fn runs(model: &str, system_channel: bool) -> Vec<String> {
    let world = RealWorld::build(&Setup {
        runtime_id: "claude",
        model,
        files: &[("README.md", "Notes app: a tiny web app to write notes.\n")],
        sandbox: SandboxMode::Off,
        memory: MemorySettings::default(),
        system_channel,
    });
    (1..=3)
        .map(|n| {
            let record = world.run(TASK);
            let usage = record.execution.usage.clone();
            let channel = record
                .execution
                .manifest
                .as_ref()
                .map(|m| format!("{:?}", m.delivery.system_channel))
                .unwrap_or_default();
            let row = match usage {
                Some(u) => format!(
                    "| {} | {n} | {channel} | {:?} | {:?} | {:?} | {:?} | {:?} |",
                    if system_channel { "on" } else { "off" },
                    record.execution.status,
                    u.input_tokens,
                    u.cached_input_tokens,
                    u.output_tokens,
                    u.cost
                ),
                None => format!(
                    "| {} | {n} | {channel} | {:?} | n/d | n/d | n/d | n/d |",
                    if system_channel { "on" } else { "off" },
                    record.execution.status
                ),
            };
            eprintln!("{row}");
            world.reset_project();
            row
        })
        .collect()
}

#[test]
#[ignore = "calls a real model (tokens): six short Claude runs; run by exact name"]
fn system_channel_real_cache() {
    let model = std::env::var("ATLAS_C3_MODEL").unwrap_or_else(|_| "haiku".to_owned());
    eprintln!(
        "\n[C3] model={model}\n| channel | run | delivered as | status | input | of it cached | output | cost |\n| --- | --- | --- | --- | --- | --- | --- | --- |"
    );
    let off = runs(&model, false);
    let on = runs(&model, true);
    assert_eq!(off.len() + on.len(), 6);
    assert!(on.iter().all(|row| row.contains("Appended")), "{on:?}");
    assert!(off.iter().all(|row| row.contains("Unsupported")), "{off:?}");
}
