//! The native recall on notes that are not generated: lessons the project actually recorded, and
//! tasks worded like real commits and requests, with the notes a developer would want written down
//! BEFORE the run (ADR 0037, amendment). No model, no cost.
//!
//! `ATLAS_REAL_CORPUS=<corpus.json> cargo test application::memory::real_notes_tests -- --ignored --nocapture`
//! where the file is `{"notes":[{id,kind,title,content}],"tasks":[{task,relevant:[id]}]}`.

use std::collections::BTreeSet;

use serde::Deserialize;

use super::ports::FakeProbe;
use super::recall::{recall, RecallRequest};
use crate::domain::memory::tests::memory;
use crate::domain::memory::{Memory, MemoryKind, MemoryMode, MemoryScope, MemorySettings};

#[derive(Deserialize)]
struct Note {
    id: String,
    kind: String,
    title: String,
    content: String,
}

#[derive(Deserialize)]
struct Task {
    task: String,
    relevant: Vec<String>,
}

#[derive(Deserialize)]
struct Corpus {
    notes: Vec<Note>,
    tasks: Vec<Task>,
}

fn notes(corpus: &Corpus) -> Vec<Memory> {
    corpus
        .notes
        .iter()
        .map(|n| {
            let mut m = memory(&n.id, MemoryScope::Workspace, Some("ws"), &n.content);
            m.title.clone_from(&n.title);
            m.kind = match n.kind.as_str() {
                "pitfall" => MemoryKind::Pitfall,
                "decision" => MemoryKind::Decision,
                "preference" => MemoryKind::Preference,
                _ => MemoryKind::HowTo,
            };
            m
        })
        .collect()
}

#[test]
#[ignore = "needs ATLAS_REAL_CORPUS; calls no model"]
#[allow(clippy::cast_precision_loss)] // counts of a few dozen notes, printed
fn recall_on_real_lessons() {
    let path = std::env::var("ATLAS_REAL_CORPUS").expect("ATLAS_REAL_CORPUS");
    let corpus: Corpus = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let memories = notes(&corpus);
    let settings = MemorySettings {
        workspace: MemoryMode::Propose,
        ..MemorySettings::default()
    };
    let (mut wanted, mut found, mut brought_positive, mut brought_negative) = (0, 0, 0, 0);
    let (mut positive_tasks, mut negative_tasks, mut tokens) = (0usize, 0usize, 0u64);
    let mut missed: Vec<String> = Vec::new();
    for (i, t) in corpus.tasks.iter().enumerate() {
        let got = recall(
            &memories,
            &RecallRequest {
                workspace: "ws",
                agent: "a",
                task: &t.task,
                project_root: "/p",
                settings,
                budget_tokens: None,
                now: 1,
            },
            &FakeProbe::default(),
        );
        let chosen: BTreeSet<&str> = got.selected.iter().map(|m| m.id.as_str()).collect();
        let hit: Vec<&String> = t
            .relevant
            .iter()
            .filter(|r| chosen.contains(r.as_str()))
            .collect();
        let extra = chosen.len() - hit.len();
        println!(
            "{i:2} {:3}/{:<3} wanted found, {:2} notes ({:3} tokens) | {}",
            hit.len(),
            t.relevant.len(),
            chosen.len(),
            got.tokens,
            t.task.chars().take(68).collect::<String>()
        );
        for r in &t.relevant {
            if !chosen.contains(r.as_str()) {
                missed.push(format!("    missed {r} for task {i}"));
            }
        }
        tokens += got.tokens;
        if t.relevant.is_empty() {
            negative_tasks += 1;
            brought_negative += chosen.len();
        } else {
            positive_tasks += 1;
            wanted += t.relevant.len();
            found += hit.len();
            brought_positive += extra;
        }
    }
    for line in &missed {
        println!("{line}");
    }
    println!(
        "TOTAL wanted notes found {found}/{wanted} | other notes brought on tasks with an answer: {} ({:.1}/task) | notes brought on tasks with none: {brought_negative} over {negative_tasks} | tokens/task {}",
        brought_positive,
        brought_positive as f64 / positive_tasks as f64,
        tokens / corpus.tasks.len() as u64
    );
}

/// The notes `scripts/demo-memoria.py` writes load with the real store and the demo task finds the
/// answer without the flood. `ATLAS_DEMO_DATA=<folder with memory/>`, workspace id `ws-demo-1`.
#[test]
#[ignore = "needs ATLAS_DEMO_DATA written by scripts/demo-memoria.py; calls no model"]
fn the_demo_notes_load_and_the_answer_is_recalled() {
    use crate::application::memory::MemoryStore;
    let root = std::path::PathBuf::from(std::env::var("ATLAS_DEMO_DATA").expect("ATLAS_DEMO_DATA"));
    let (memories, unreadable) =
        crate::infrastructure::FsMemoryStore::new(root.join("memory")).load_all();
    assert!(unreadable.is_empty(), "{unreadable:?}");
    assert_eq!(memories.len(), 15);
    let settings = MemorySettings {
        workspace: MemoryMode::Propose,
        ..MemorySettings::default()
    };
    let task =
        "Change the retry limit of this service from 3 to 5, and tell me briefly what you changed.";
    let got = recall(
        &memories,
        &RecallRequest {
            workspace: "ws-demo-1",
            agent: "a",
            task,
            project_root: "/p",
            settings,
            budget_tokens: None,
            now: 10_000_000_000_000,
        },
        &FakeProbe::default(),
    );
    println!(
        "prompt receives {} of {} notes (~{} tokens):",
        got.selected.len(),
        memories.len(),
        got.tokens
    );
    for m in &got.selected {
        println!("  - {}", m.title);
    }
    assert!(got
        .selected
        .iter()
        .any(|m| m.title.starts_with("Retry limit lives in")));
    assert!(got.selected.len() < memories.len());
}
