//! Atlas's own memory on the same corpus as `spikes/ai-memory-bench/retrieval_bench.py` (ADR 0040):
//! 240 lookalike notes, 40 tasks that name the subject and 40 that word it differently. It measures
//! what the native recall puts in a prompt, and how that grows with the number of notes: the
//! numbers behind the flood pruning of `recall.rs` (ADR 0037, amendment). No model, no cost.
//!
//! Run by name, with the corpus the Python script dumps:
//! `ATLAS_BENCH_CORPUS=<corpus.json> cargo test application::memory::native_bench_tests -- --ignored --nocapture`

use std::collections::BTreeMap;

use serde::Deserialize;

use super::ports::FakeProbe;
use super::recall::{recall, RecallRequest};
use crate::domain::memory::tests::memory;
use crate::domain::memory::{Memory, MemoryKind, MemoryMode, MemoryScope, MemorySettings};

#[derive(Deserialize)]
struct Corpus {
    pages: BTreeMap<String, (String, String)>,
    subject_of: BTreeMap<String, usize>,
    direct: Vec<String>,
    paraphrase: Vec<String>,
}

/// The page a memory came from: its id carries a hash in front so that the order of ids, the last
/// tie-break of the ranking, is unrelated to which note is the right one (`gold-…` would otherwise
/// sort before `noise-…` and flatter every result).
fn original(id: &str) -> &str {
    id.split_once('~').map_or(id, |(_, rest)| rest)
}

fn hashed(id: &str) -> String {
    let mut h: u32 = 2_166_136_261;
    for b in id.bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(16_777_619);
    }
    format!("{h:08x}~{id}")
}

fn notes(corpus: &Corpus) -> Vec<Memory> {
    corpus
        .pages
        .iter()
        .map(|(id, (title, body))| {
            let mut note = memory(&hashed(id), MemoryScope::Workspace, Some("ws"), body);
            note.title = title.chars().take(80).collect();
            note.kind = MemoryKind::Pitfall;
            note
        })
        .collect()
}

fn measure(label: &str, corpus: &Corpus, memories: &[Memory], tasks: &[String], max_tokens: u32) {
    let settings = MemorySettings {
        workspace: MemoryMode::Propose,
        max_tokens,
        ..MemorySettings::default()
    };
    let (mut prompts, mut count, mut gold3, mut gold_any, mut foreign, mut tokens) =
        (0, 0, 0, 0, 0, 0);
    for (i, task) in tasks.iter().enumerate() {
        let got = recall(
            memories,
            &RecallRequest {
                workspace: "ws",
                agent: "a",
                task,
                project_root: "/p",
                settings,
                budget_tokens: None,
                now: 1,
            },
            &FakeProbe::default(),
        );
        let ids: Vec<&str> = got.selected.iter().map(|m| original(&m.id)).collect();
        let gold = format!("gold-{i}");
        prompts += usize::from(!ids.is_empty());
        count += ids.len();
        gold3 += usize::from(ids.iter().take(3).any(|id| *id == gold));
        gold_any += usize::from(ids.iter().any(|id| *id == gold));
        foreign += ids
            .iter()
            .filter(|id| corpus.subject_of.get(**id) != Some(&i))
            .count();
        tokens += got.tokens;
    }
    let n = tasks.len();
    println!(
        "{label:42} notes in {prompts:2}/{n} prompts ({count:3} notes, ~{} tokens/prompt) | right note in top 3: {gold3:2}/{n}, anywhere: {gold_any:2}/{n} | other-subject notes: {foreign}",
        tokens / n as u64
    );
}

#[test]
#[ignore = "needs ATLAS_BENCH_CORPUS (written by retrieval_bench.py); calls no model"]
fn native_recall_on_the_bench_corpus() {
    let path = std::env::var("ATLAS_BENCH_CORPUS").expect("ATLAS_BENCH_CORPUS");
    let corpus: Corpus = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let memories = notes(&corpus);
    for (budget, tag) in [(600, "default 600-token budget"), (150, "150-token budget")] {
        measure(
            &format!("native, direct tasks, {tag}"),
            &corpus,
            &memories,
            &corpus.direct,
            budget,
        );
        measure(
            &format!("native, paraphrased tasks, {tag}"),
            &corpus,
            &memories,
            &corpus.paraphrase,
            budget,
        );
    }
}

/// The same measure at growing sizes: the first `k` subjects, each with its right note and its five
/// lookalikes (so 6·k notes), and only the tasks for those subjects. Default 600-token budget.
#[test]
#[ignore = "needs ATLAS_BENCH_CORPUS (written by retrieval_bench.py); calls no model"]
fn native_recall_as_the_notes_grow() {
    let path = std::env::var("ATLAS_BENCH_CORPUS").expect("ATLAS_BENCH_CORPUS");
    let corpus: Corpus = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let all = notes(&corpus);
    for k in [4usize, 8, 16, 24, 40] {
        let kept: Vec<Memory> = all
            .iter()
            .filter(|m| {
                corpus
                    .subject_of
                    .get(original(&m.id))
                    .is_some_and(|s| *s < k)
            })
            .cloned()
            .collect();
        let sub = Corpus {
            pages: BTreeMap::new(),
            subject_of: corpus.subject_of.clone(),
            direct: corpus.direct[..k].to_vec(),
            paraphrase: corpus.paraphrase[..k].to_vec(),
        };
        measure(
            &format!("native, {:3} notes, direct tasks", kept.len()),
            &sub,
            &kept,
            &sub.direct,
            600,
        );
    }
}
