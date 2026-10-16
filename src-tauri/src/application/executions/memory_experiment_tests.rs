//! M4 of the Memory (ADR 0037): does a note of earlier work make a real run cheaper and less
//! error-prone? The whole of Atlas around a real CLI, three arms on one small task, measured.
//!
//! The project has a decoy: `config/app.toml` has a `retries` key that nothing reads, and the real
//! limit is `MAXTRY` in `deploy/limits.cfg`. The task is "change the retry limit from 3 to 5".
//!
//! - **A, control**: Memory off.
//! - **B, a note**: Memory on, with one note the person wrote ("it is MAXTRY in deploy/limits.cfg").
//! - **C, the loop**: Memory in `Auto`. Run 1 knows nothing and may leave a note (`atlas-memory`);
//!   the project is put back; run 2 recalls whatever run 1 left.
//!
//! Nothing here asserts that Memory *helps*: it measures, prints a table and, when
//! `ATLAS_M4_OUT=<file>` is set, writes it as Markdown. What it does assert are the invariants of
//! the arms (A carries no notes, B does, C's second run carries what the first left).
//!
//! Costs tokens, so it runs only by exact name, never with a broad `--ignored`:
//! `ATLAS_M4_N=3 cargo test application::executions::memory_experiment_tests::<name> -- --ignored --exact --nocapture`.

use std::fmt::Write as _;
use std::time::Instant;

use super::real_support::{git, RealWorld, Setup};
use super::*;
use crate::application::memory::NewMemory;
use crate::domain::execution::ExecutionStatus;
use crate::domain::memory::{
    MemoryKind, MemoryMode, MemoryOrigin, MemoryScope, MemorySettings, MemoryValidity,
};
use crate::domain::optimization::SectionKind;
use crate::domain::security::SandboxMode;

const FILES: &[(&str, &str)] = &[
    ("README.md", "# Billing client\n\nA small client for the billing API.\n"),
    (
        "config/app.toml",
        "# Application settings\nname = \"billing\"\nretries = 3\ntimeout_seconds = 30\n",
    ),
    (
        "deploy/limits.cfg",
        "# Limits applied when the service starts\nMAXTRY=3\nTIMEOUT=30\n",
    ),
    (
        "src/client.py",
        "import os\n\nMAX_TRY = int(os.environ.get(\"MAXTRY\", \"3\"))\n\n\ndef call(api):\n    for _ in range(MAX_TRY):\n        result = api()\n        if result is not None:\n            return result\n    raise RuntimeError(\"billing API unavailable\")\n",
    ),
    (
        "src/config_loader.py",
        "import tomllib\n\n\ndef load(path=\"config/app.toml\"):\n    with open(path, \"rb\") as handle:\n        data = tomllib.load(handle)\n    return {\"name\": data[\"name\"], \"timeout\": data[\"timeout_seconds\"]}\n",
    ),
    (
        "docs/operations.md",
        "# Operations\n\nThe service is restarted by the deploy script, which exports the limits file.\n",
    ),
];

const TASK: &str =
    "Change the retry limit of this service from 3 to 5, then reply with the single word DONE.";

/// The same task worded the way a person would: it asks for an account of the change instead of a
/// one-word reply, so a block with notes does not compete with the answer format.
/// `ATLAS_M4_TASK=realistic` selects it.
const TASK_REALISTIC: &str =
    "Change the retry limit of this service from 3 to 5, and tell me briefly what you changed.";

fn task() -> &'static str {
    if std::env::var("ATLAS_M4_TASK").as_deref() == Ok("realistic") {
        TASK_REALISTIC
    } else {
        TASK
    }
}

const NOTE_TITLE: &str = "Retry limit";
const NOTE: &str = "The retry limit is MAXTRY in deploy/limits.cfg. The retries key in config/app.toml is not read by anything: leave it alone.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    A,
    B,
    C1,
    C2,
}

impl Arm {
    fn label(self) -> &'static str {
        match self {
            Self::A => "A control",
            Self::B => "B note",
            Self::C1 => "C run 1",
            Self::C2 => "C run 2",
        }
    }
}

#[derive(Debug, Clone)]
struct Trial {
    arm: Arm,
    status: ExecutionStatus,
    /// `MAXTRY=5` is in `deploy/limits.cfg` of the project afterwards.
    right_file: bool,
    /// `config/app.toml` was changed too (the decoy).
    touched_decoy: bool,
    input: u64,
    output: u64,
    cached: u64,
    tool_calls: usize,
    seconds: f64,
    /// What the notes added to this prompt, in estimated tokens.
    note_tokens: u64,
    notes_sent: usize,
    proposed: u64,
    /// Notes held for an independent validator (automatic mode).
    accepted: u64,
    execution_id: String,
    prompt_has_notes: bool,
}

fn counter(record: &ExecutionRecord, key: &str) -> u64 {
    record
        .execution
        .metadata
        .get(key)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn measure(arm: Arm, world: &RealWorld, started: Instant, record: &ExecutionRecord) -> Trial {
    let execution = &record.execution;
    let usage = execution.usage.as_ref();
    let tokens = |field: fn(&crate::domain::usage::UsageMetrics) -> Option<u64>| {
        usage.and_then(field).unwrap_or(0)
    };
    let manifest = execution.manifest.as_ref();
    let limits = git(&world.project, &["show", "main:deploy/limits.cfg"]);
    let app = git(&world.project, &["show", "main:config/app.toml"]);
    Trial {
        arm,
        status: execution.status,
        right_file: limits.contains("MAXTRY=5"),
        touched_decoy: app.contains("retries = 5") || !app.contains("retries = 3"),
        input: tokens(|u| u.input_tokens),
        output: tokens(|u| u.output_tokens),
        cached: tokens(|u| u.cached_input_tokens),
        tool_calls: execution
            .logs
            .iter()
            .filter(|line| line.starts_with("Using "))
            .count(),
        seconds: started.elapsed().as_secs_f64(),
        note_tokens: manifest
            .and_then(|m| {
                m.sections
                    .iter()
                    .find(|s| s.section == SectionKind::Memory)
                    .map(|s| s.estimated_tokens)
            })
            .unwrap_or(0),
        notes_sent: manifest.map_or(0, |m| {
            m.memories
                .iter()
                .filter(|n| n.status == crate::domain::context::MemoryStatusInPrompt::Sent)
                .count()
        }),
        proposed: counter(record, "memoryProposed"),
        accepted: counter(record, "memoryHeld"),
        prompt_has_notes: execution.prompt.contains("NOTES FROM EARLIER WORK"),
        execution_id: execution.id.clone(),
    }
}

fn setup<'a>(runtime: &'a str, model: &'a str, memory: MemorySettings) -> Setup<'a> {
    Setup {
        runtime_id: runtime,
        model,
        files: FILES,
        sandbox: SandboxMode::Off,
        memory,
    }
}

fn workspace_only(mode: MemoryMode) -> MemorySettings {
    MemorySettings {
        workspace: mode,
        ..MemorySettings::default()
    }
}

fn timed(world: &RealWorld, arm: Arm) -> Trial {
    let started = Instant::now();
    let record = world.run(task());
    let trial = measure(arm, world, started, &record);
    eprintln!("{trial:?}");
    if record.execution.status != ExecutionStatus::Completed {
        // Why a run did not complete, and the last of what it did: a failed run is a result too.
        eprintln!(
            "FAILURE after {}: {:?}\n  last logs: {:?}",
            arm.label(),
            record.execution.failure,
            record
                .execution
                .logs
                .iter()
                .rev()
                .take(6)
                .map(|l| l.chars().take(160).collect::<String>())
                .collect::<Vec<_>>()
        );
    }
    if std::env::var("ATLAS_M4_ANSWER").is_ok() {
        // The whole final answer, to see whether a block was written and not read, or not written.
        eprintln!(
            "ANSWER after {}:\n{}\n---END---",
            arm.label(),
            record.execution.result.clone().unwrap_or_default()
        );
    }
    // What the agent left behind, to judge the notes themselves and not only the counts.
    for note in world.memory.list() {
        if note.provenance.origin == MemoryOrigin::Agent {
            eprintln!(
                "NOTE after {}: [{:?}/{:?}] {} :: {}",
                arm.label(),
                note.status,
                note.kind,
                note.title,
                note.content
            );
        }
    }
    trial
}

fn trial_a(runtime: &str, model: &str) -> Trial {
    let world = RealWorld::build(&setup(runtime, model, MemorySettings::default()));
    timed(&world, Arm::A)
}

fn trial_b(runtime: &str, model: &str) -> Trial {
    let world = RealWorld::build(&setup(runtime, model, workspace_only(MemoryMode::Propose)));
    world
        .memory
        .create(
            NewMemory {
                scope: MemoryScope::Workspace,
                owner: Some(world.workspace_id.clone()),
                kind: MemoryKind::Pitfall,
                title: NOTE_TITLE.to_owned(),
                content: NOTE.to_owned(),
                topic: None,
                validity: MemoryValidity {
                    paths: vec!["deploy/limits.cfg".to_owned()],
                    ..MemoryValidity::default()
                },
                origin: MemoryOrigin::User,
            },
            1,
        )
        .unwrap();
    timed(&world, Arm::B)
}

/// Run 1 knows nothing; whatever it leaves is what run 2 gets.
fn trial_c(runtime: &str, model: &str) -> (Trial, Trial) {
    let world = RealWorld::build(&setup(runtime, model, workspace_only(MemoryMode::Auto)));
    let first = timed(&world, Arm::C1);
    // Automatic notes wait for an independent validator. This task has none, so
    // `ATLAS_M4_VALIDATE=1` plays one (another agent passing the run), to measure what the loop
    // is worth when a validator exists; without it the notes stay held and run 2 sees nothing.
    if std::env::var("ATLAS_M4_VALIDATE").is_ok() {
        let held = world.memory.confirm_held(
            std::slice::from_ref(&first.execution_id),
            &crate::application::memory::Validator {
                execution: "simulated-validator",
                agent: "simulated-validator-agent",
            },
            crate::application::support::now_ms(),
        );
        eprintln!("VALIDATOR let in {} note(s)", held.len());
    }
    world.reset_project();
    let second = timed(&world, Arm::C2);
    (first, second)
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    match values.len() {
        0 => 0.0,
        n if n % 2 == 1 => values[n / 2],
        n => f64::midpoint(values[n / 2 - 1], values[n / 2]),
    }
}

#[allow(clippy::cast_precision_loss)] // counts of tens of thousands: exact as a float
/// The table, one row per arm. Medians, because three runs are too few for a mean to mean much.
fn report(runtime: &str, model: &str, trials: &[Trial]) -> String {
    let mut out = format!(
        "## {runtime} · {model}\n\n| arm | runs | completed | right file | touched decoy | input (median) | cached | output | tool calls | notes in prompt (est. tokens) | seconds |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n"
    );
    for arm in [Arm::A, Arm::B, Arm::C1, Arm::C2] {
        let of: Vec<&Trial> = trials.iter().filter(|t| t.arm == arm).collect();
        if of.is_empty() {
            continue;
        }
        let med = |f: &dyn Fn(&Trial) -> f64| median(of.iter().map(|t| f(t)).collect());
        let _ =
            writeln!(
            out,
            "| {} | {} | {}/{} | {}/{} | {}/{} | {:.0} | {:.0} | {:.0} | {:.1} | {:.0} | {:.0} |",
            arm.label(),
            of.len(),
            of.iter().filter(|t| t.status == ExecutionStatus::Completed).count(),
            of.len(),
            of.iter().filter(|t| t.right_file).count(),
            of.len(),
            of.iter().filter(|t| t.touched_decoy).count(),
            of.len(),
            med(&|t| t.input as f64),
            med(&|t| t.cached as f64),
            med(&|t| t.output as f64),
            med(&|t| t.tool_calls as f64),
            med(&|t| t.note_tokens as f64),
            med(&|t| t.seconds),
        );
    }
    let c1: Vec<&Trial> = trials.iter().filter(|t| t.arm == Arm::C1).collect();
    if !c1.is_empty() {
        let _ = writeln!(
            out,
            "\nLoop: run 1 proposed {} note(s) for a person and held {} for a validator, over {} run(s); run 2 saw {} note(s) in total.",
            c1.iter().map(|t| t.proposed).sum::<u64>(),
            c1.iter().map(|t| t.accepted).sum::<u64>(),
            c1.len(),
            trials
                .iter()
                .filter(|t| t.arm == Arm::C2)
                .map(|t| t.notes_sent)
                .sum::<usize>(),
        );
    }
    out
}

fn experiment(runtime: &str, model: &str) {
    let n: usize = std::env::var("ATLAS_M4_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    // `ATLAS_M4_ARMS=C` (or `AB`, or `ABC`, the default) runs only the arms named.
    let arms = std::env::var("ATLAS_M4_ARMS").unwrap_or_else(|_| "ABC".to_owned());
    let mut trials = Vec::new();
    // Interleaved, so a slow hour or a model update does not fall on one arm only.
    for _ in 0..n {
        if arms.contains('A') {
            let a = trial_a(runtime, model);
            assert!(!a.prompt_has_notes, "the control carried notes");
            trials.push(a);
        }
        if arms.contains('B') {
            let b = trial_b(runtime, model);
            assert!(
                b.prompt_has_notes && b.notes_sent == 1,
                "the note did not reach the prompt"
            );
            trials.push(b);
        }
    }
    if arms.contains('C') && std::env::var("ATLAS_M4_C1_ONLY").is_ok() {
        // One run, without the second: to look at what the agent answered.
        let world = RealWorld::build(&setup(runtime, model, workspace_only(MemoryMode::Auto)));
        trials.push(timed(&world, Arm::C1));
    } else if arms.contains('C') {
        for _ in 0..n {
            let (first, second) = trial_c(runtime, model);
            assert!(!first.prompt_has_notes, "run 1 of the loop knew something");
            if first.accepted > 0 && std::env::var("ATLAS_M4_VALIDATE").is_ok() {
                assert!(
                    second.prompt_has_notes,
                    "a note a validator let in was not recalled"
                );
            }
            trials.push(first);
            trials.push(second);
        }
    }
    let table = report(runtime, model, &trials);
    eprintln!("\n{table}");
    if let Ok(path) = std::env::var("ATLAS_M4_OUT") {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(&mut file, format!("{table}\n").as_bytes()).unwrap();
    }
}

#[test]
#[ignore = "calls a real model (tokens): OpenCode, Memory experiment"]
fn memory_experiment_opencode() {
    experiment("opencode", "opencode/big-pickle");
}

#[test]
#[ignore = "calls a real model (tokens): Claude, Memory experiment"]
fn memory_experiment_claude() {
    experiment("claude", "haiku");
}

#[test]
#[ignore = "calls a real model (tokens): Claude Sonnet, Memory experiment"]
fn memory_experiment_claude_sonnet() {
    experiment("claude", "sonnet");
}

#[test]
#[ignore = "calls a real model (tokens): Codex, Memory experiment"]
fn memory_experiment_codex() {
    experiment("codex", "gpt-5.6-luna");
}

#[test]
#[ignore = "calls a real model (tokens): Copilot, Memory experiment"]
fn memory_experiment_copilot() {
    experiment("copilot", "auto");
}

#[test]
#[ignore = "calls a real model (tokens): Antigravity, Memory experiment"]
fn memory_experiment_antigravity() {
    experiment("antigravity", "gemini-3.8-flash-low");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial(arm: Arm, right: bool, input: u64) -> Trial {
        Trial {
            arm,
            status: ExecutionStatus::Completed,
            right_file: right,
            touched_decoy: !right,
            input,
            output: 100,
            cached: 0,
            tool_calls: 4,
            seconds: 10.0,
            note_tokens: 0,
            notes_sent: 0,
            proposed: 1,
            accepted: 1,
            execution_id: "e".to_owned(),
            prompt_has_notes: false,
        }
    }

    #[test]
    fn the_median_of_few_runs_is_the_middle_one_and_the_mean_of_the_middle_two_for_an_even_count() {
        assert_eq!(median(vec![]), 0.0);
        assert_eq!(median(vec![9.0, 1.0, 5.0]), 5.0);
        assert_eq!(median(vec![1.0, 2.0, 3.0, 10.0]), 2.5);
    }

    #[test]
    fn the_table_has_a_row_per_arm_that_ran_and_counts_what_went_right_and_wrong() {
        let trials = [
            trial(Arm::A, false, 30_000),
            trial(Arm::A, true, 20_000),
            trial(Arm::B, true, 12_000),
            trial(Arm::C1, true, 25_000),
            trial(Arm::C2, true, 11_000),
        ];

        let table = report("claude", "haiku", &trials);

        assert!(table.contains("## claude · haiku"));
        assert!(
            table.contains("| A control | 2 | 2/2 | 1/2 | 1/2 | 25000 |"),
            "{table}"
        );
        assert!(
            table.contains("| B note | 1 | 1/1 | 1/1 | 0/1 | 12000 |"),
            "{table}"
        );
        assert!(table.contains("| C run 2 | 1 | 1/1 | 1/1 |"), "{table}");
        assert!(table
            .contains("Loop: run 1 proposed 1 note(s) for a person and held 1 for a validator"));
    }
}
