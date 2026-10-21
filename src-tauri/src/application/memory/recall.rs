//! Choosing which memories a prompt carries: deterministic, under a budget, and silent when
//! nothing is relevant (a prompt with no recalled memory has no trace of the feature).

use std::collections::{BTreeMap, BTreeSet};

use super::block::MemoryBlock;
use super::ports::ProjectProbe;
use crate::domain::context::{ManifestMemory, MemoryStatusInPrompt};
use crate::domain::memory::{self, Memory, MemoryKind, MemoryMode, MemorySettings, MemoryStatus};
use crate::domain::optimization::TextSize;

const DAY_MS: u64 = 86_400_000;

/// What the recovery is asked for.
pub struct RecallRequest<'a> {
    pub workspace: &'a str,
    pub agent: &'a str,
    /// The task text, and the context query when the step has one.
    pub task: &'a str,
    /// The project's own folder, for checking what a lesson depends on.
    pub project_root: &'a str,
    pub settings: MemorySettings,
    /// What the execution's budget leaves for memories, if it has one; the smaller of this and
    /// `settings.max_tokens` is the ceiling.
    pub budget_tokens: Option<u32>,
    pub now: u64,
}

/// What happened to one memory the recovery looked at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecallOutcome {
    Selected,
    /// Shares nothing with the task.
    NoMatch,
    /// Relevant, but did not fit in what was left of the budget.
    OverBudget,
    /// Another memory of the same topic ranked higher.
    Superseded(String),
    /// Its premise no longer holds; the reason says what is gone.
    Stale(String),
    /// An automatic memory unused for longer than it was allowed.
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Considered {
    pub id: String,
    pub title: String,
    /// It entered without a person deciding.
    pub automatic: bool,
    pub outcome: RecallOutcome,
}

impl Considered {
    fn of(memory: &Memory, outcome: RecallOutcome) -> Self {
        Self {
            id: memory.id.clone(),
            title: memory.title.clone(),
            automatic: memory.provenance.automatic,
            outcome,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recall {
    /// In the order they go into the prompt.
    pub selected: Vec<Memory>,
    /// Every memory of an active scope that was looked at, so "why did the agent know this" and
    /// "why did it not" both have an answer.
    pub considered: Vec<Considered>,
    /// The estimated tokens the selected memories add.
    pub tokens: u64,
}

impl Recall {
    /// What the manifest says of the memories: every one that could have gone into the prompt and
    /// what became of it (`kept` are the notes still in the prompt after the Context Engine and
    /// the guardrails), and how many were of no use to the task.
    pub fn manifest(&self, kept: &[MemoryBlock]) -> (Vec<ManifestMemory>, u32) {
        let mut not_relevant = 0;
        let mut records = Vec::new();
        for considered in &self.considered {
            let (status, reason) = match &considered.outcome {
                RecallOutcome::NoMatch => {
                    not_relevant += 1;
                    continue;
                }
                RecallOutcome::Selected => {
                    let still_there = kept
                        .iter()
                        .any(|b| b.id == considered.id && !b.text.trim().is_empty());
                    if still_there {
                        (MemoryStatusInPrompt::Sent, None)
                    } else {
                        (MemoryStatusInPrompt::OmittedForBudget, None)
                    }
                }
                RecallOutcome::OverBudget => (MemoryStatusInPrompt::OverBudget, None),
                RecallOutcome::Superseded(by) => {
                    (MemoryStatusInPrompt::Superseded, Some(by.clone()))
                }
                RecallOutcome::Stale(why) => (MemoryStatusInPrompt::Stale, Some(why.clone())),
                RecallOutcome::Expired => (MemoryStatusInPrompt::Expired, None),
            };
            records.push(ManifestMemory {
                id: considered.id.clone(),
                title: considered.title.clone(),
                status,
                reason,
                automatic: considered.automatic,
            });
        }
        (records, not_relevant)
    }
}

/// One memory as the prompt shows it.
pub fn render_line(memory: &Memory) -> String {
    let kind = match memory.kind {
        MemoryKind::HowTo => "how-to",
        MemoryKind::Pitfall => "pitfall",
        MemoryKind::Preference => "preference",
        MemoryKind::Decision => "decision",
    };
    // A note no person decided says so, so a model weighs it as what it is.
    let unconfirmed = if memory.is_unconfirmed() {
        " (unconfirmed)"
    } else {
        ""
    };
    format!(
        "- [{kind}{unconfirmed}] {}: {}",
        memory.title.trim(),
        memory.content.trim()
    )
}

/// Why a memory's premise no longer holds, if it does not. Without a model.
fn staleness(memory: &Memory, root: &str, probe: &dyn ProjectProbe) -> Option<String> {
    if let Some(path) = memory
        .validity
        .paths
        .iter()
        .find(|p| !probe.path_exists(root, p))
    {
        return Some(format!("{path} is not in the project any more"));
    }
    if !memory.validity.scripts.is_empty() {
        if let Some(scripts) = probe.npm_scripts(root) {
            if let Some(missing) = memory
                .validity
                .scripts
                .iter()
                .find(|s| !scripts.contains(*s))
            {
                return Some(format!(
                    "the script `{missing}` is not in package.json any more"
                ));
            }
        }
    }
    None
}

fn expired(memory: &Memory, now: u64) -> bool {
    memory.provenance.automatic
        && memory.validity.expires_after_days.is_some_and(|days| {
            let since = memory.last_used_at.unwrap_or(memory.updated_at);
            now.saturating_sub(since) > u64::from(days) * DAY_MS
        })
}

/// More notes than this sharing a word with the task is a flood, not a match: below it every note
/// that shares a word is kept, as it always was.
const PRUNE_ABOVE: usize = 5;
/// A memory with fewer notes than this is never pruned: a few notes about one subject (six of
/// eight mention "build") are all relevant to a task about it, and the budget is enough for them.
const PRUNE_NEEDS_NOTES: usize = 12;
/// A note that got in on a rare word, or on one title word, must score at least this percent of the
/// best one to stay.
const KEEP_PERCENT: u64 = 60;
/// A note that shares this many uncommon words with the task, in its title or its content, stays if
/// it scores at least [`TWO_WORDS_PERCENT`] of the best. Added after trying the rules on notes the
/// project actually recorded, where the title seldom repeats the task's words: without it the
/// pruning lost 5 of the 26 notes a developer wanted (88% found became 69%).
const TWO_WORDS_MIN: usize = 2;
const TWO_WORDS_PERCENT: u64 = 40;
/// A word is rare when at most one note in this many has it (and at least one note, always).
const RARE_ONE_IN: usize = 20;
/// A word that more than one note in this many has says nothing about which note is the right one
/// ("change", "service"): it adds nothing to a note's score. Only counted in a memory big enough
/// for the share to mean something.
const GENERIC_ONE_IN: usize = 4;
const GENERIC_NEEDS_NOTES: usize = 20;

/// Keeps, out of a flood of notes that share a word with the task, the ones that are about it:
///
/// - the title carries two of its words (or the one it has): the note says it is about the task; or
/// - the note shares a word few notes have, or a word of its title that is not common, **and** its
///   score is at least [`KEEP_PERCENT`] of the best; or
/// - the note shares [`TWO_WORDS_MIN`] uncommon words with the task and scores at least
///   [`TWO_WORDS_PERCENT`] of the best.
///
/// The score weighs each word by how rare it is (title twice) and counts for nothing a word most
/// notes have. All of it is whole numbers (thousandths), so the same notes give the same answer on
/// any platform. What is left out is `NoMatch`, so a task nothing really matches gets no notes. What
/// stays is ordered in tenths of the best score, then by [`memory::tie_bonus`], so that use and
/// recency decide between notes that score alike. Measured on 240 lookalike notes
/// (`spikes/ai-memory-analysis.md`): without this the native recall filled the whole budget of every
/// prompt from about fifteen notes, nine of them about something else.
fn prune<'a>(
    candidates: Vec<(&'a Memory, memory::Fit)>,
    pool: &[&Memory],
    task_words: usize,
    result: &mut Recall,
) -> Vec<(&'a Memory, u32)> {
    struct Scored<'b> {
        memory: &'b Memory,
        weight: u64,
        /// Its title carries the words of the task.
        in_title: bool,
        /// A word of its title is not a common one.
        title_word: bool,
        /// It shares a word few notes have.
        rare: bool,
        /// How many distinct words, not common ones, it shares with the task.
        informative: usize,
    }

    let docs: Vec<BTreeSet<String>> = pool
        .iter()
        .map(|m| {
            let mut words =
                memory::stems(&format!("{} {}", m.title, m.topic.as_deref().unwrap_or("")));
            words.extend(memory::stems(&m.content));
            words
        })
        .collect();
    let n = docs.len();
    let document_frequency = |word: &str| docs.iter().filter(|d| d.contains(word)).count().max(1);
    let rare_at = (n / RARE_ONE_IN).max(1);
    let is_generic = |df: usize| n >= GENERIC_NEEDS_NOTES && df * GENERIC_ONE_IN > n;
    // ln(1 + N/df) in thousandths: whole numbers from here on.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )] // counts of a few hundred notes; the result is small and positive
    let idf = |df: usize| ((1.0 + n as f64 / df as f64).ln() * 1000.0).round() as u64;
    let need_in_title = task_words.min(2);

    let scored: Vec<Scored> = candidates
        .into_iter()
        .map(|(memory, fit)| {
            let (mut weight, mut rare, mut title_word, mut informative) = (0, false, false, 0usize);
            for (word, times, in_title) in fit
                .title
                .iter()
                .map(|w| (w, 2, true))
                .chain(fit.content.iter().map(|w| (w, 1, false)))
            {
                let df = document_frequency(word);
                // A word nearly every note has cannot tell the right note from the others: without
                // this, the notes that add "change" and "service" to the same subject outrank the
                // one that holds the answer.
                if !is_generic(df) {
                    weight += idf(df) * times;
                    title_word |= in_title;
                    // a word more than half of the notes have is not uncommon in any memory,
                    // however small
                    informative += usize::from(df * 2 <= n);
                }
                rare |= df <= rare_at;
            }
            Scored {
                memory,
                weight,
                in_title: fit.title.len() >= need_in_title,
                title_word,
                rare,
                informative,
            }
        })
        .collect();
    let best = scored.iter().map(|s| s.weight).max().unwrap_or(0);
    let mut kept = Vec::new();
    for s in scored {
        let near_best = s.weight * 100 >= KEEP_PERCENT * best;
        // A title that carries the task's words is the note saying it is about it: it stays. A note
        // that got in on one rare word, or on one word of its title, has to be near the best to
        // stay, or a single odd word would bring any note.
        // Two uncommon words of the task in the content are enough to be about it, even when the
        // title echoes nothing, if the note is not far behind the best (a looser bar than one odd
        // word has to clear).
        let two_words =
            s.informative >= TWO_WORDS_MIN && s.weight * 100 >= TWO_WORDS_PERCENT * best;
        if s.in_title || ((s.rare || s.title_word) && near_best) || two_words {
            // In tenths of the best, rounding to the nearest: a difference smaller than that is
            // noise (one more common word), and what separates two notes then is the rank's next
            // keys. The edge between two tenths is a step, not a slope.
            let tenths = (s.weight * 10 + best / 2)
                .checked_div(best)
                .map_or(0, |t| u32::try_from(t).unwrap_or(10));
            kept.push((s.memory, tenths * 10 + memory::tie_bonus(s.memory)));
        } else {
            result
                .considered
                .push(Considered::of(s.memory, RecallOutcome::NoMatch));
        }
    }
    kept
}

/// Chooses the memories for one execution. Pure: the same memories, task and project always give
/// the same answer.
pub fn recall(memories: &[Memory], req: &RecallRequest<'_>, probe: &dyn ProjectProbe) -> Recall {
    let mut result = Recall::default();
    if !req.settings.any_on() {
        return result;
    }
    let ceiling = req
        .budget_tokens
        .map_or(req.settings.max_tokens, |b| b.min(req.settings.max_tokens));
    let task_terms: BTreeSet<String> = memory::stems(req.task);

    let mut candidates: Vec<(&Memory, memory::Fit)> = Vec::new();
    // Every note that could be recovered for this execution, matching or not: the base the rarity of
    // a word is measured against.
    let mut pool: Vec<&Memory> = Vec::new();
    for item in memories {
        if item.status != MemoryStatus::Active
            || req.settings.mode(item.scope) == MemoryMode::Off
            || !item.applies_to(req.workspace, req.agent)
        {
            continue;
        }
        if expired(item, req.now) {
            result
                .considered
                .push(Considered::of(item, RecallOutcome::Expired));
        } else if let Some(reason) = staleness(item, req.project_root, probe) {
            result
                .considered
                .push(Considered::of(item, RecallOutcome::Stale(reason)));
        } else {
            pool.push(item);
            if let Some(fit) = memory::fit(item, &task_terms) {
                candidates.push((item, fit));
            } else {
                result
                    .considered
                    .push(Considered::of(item, RecallOutcome::NoMatch));
            }
        }
    }
    let mut candidates: Vec<(&Memory, u32)> =
        if candidates.len() > PRUNE_ABOVE && pool.len() >= PRUNE_NEEDS_NOTES {
            prune(candidates, &pool, task_terms.len(), &mut result)
        } else {
            candidates
                .into_iter()
                .map(|(m, fit)| (m, fit.score))
                .collect()
        };
    candidates.sort_by_key(|(item, score)| memory::rank(item, *score));

    let mut by_topic: BTreeMap<String, String> = BTreeMap::new();
    let mut spent: u64 = 0;
    for (item, _) in candidates {
        if let Some(winner) = item.topic_key().and_then(|t| by_topic.get(&t)) {
            result.considered.push(Considered::of(
                item,
                RecallOutcome::Superseded(winner.clone()),
            ));
            continue;
        }
        let cost = TextSize::of(&render_line(item)).estimated_tokens();
        if spent + cost > u64::from(ceiling) {
            result
                .considered
                .push(Considered::of(item, RecallOutcome::OverBudget));
            continue;
        }
        spent += cost;
        if let Some(topic) = item.topic_key() {
            by_topic.insert(topic, item.id.clone());
        }
        result
            .considered
            .push(Considered::of(item, RecallOutcome::Selected));
        result.selected.push(item.clone());
    }
    result.tokens = spent;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::memory::ports::FakeProbe;
    use crate::domain::memory::tests::memory;
    use crate::domain::memory::{MemoryScope, MemoryValidity};

    fn on() -> MemorySettings {
        MemorySettings {
            user: MemoryMode::Propose,
            workspace: MemoryMode::Propose,
            agent: MemoryMode::Propose,
            max_tokens: 600,
            chosen: false,
        }
    }

    fn request(task: &str, settings: MemorySettings) -> RecallRequest<'_> {
        RecallRequest {
            workspace: "w1",
            agent: "a1",
            task,
            project_root: "/p",
            settings,
            budget_tokens: None,
            now: 1_000,
        }
    }

    fn ids(recall: &Recall) -> Vec<&str> {
        recall.selected.iter().map(|m| m.id.as_str()).collect()
    }

    fn outcome<'a>(recall: &'a Recall, id: &str) -> &'a RecallOutcome {
        &recall
            .considered
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .outcome
    }

    #[test]
    fn with_every_scope_off_nothing_is_recovered_and_nothing_is_looked_at() {
        let all = [memory("m", MemoryScope::User, None, "run the check")];

        let found = recall(
            &all,
            &request("run the check", MemorySettings::default()),
            &FakeProbe::default(),
        );

        assert_eq!(found, Recall::default());
    }

    #[test]
    fn only_a_scope_that_is_on_is_recovered() {
        let all = [
            memory("u", MemoryScope::User, None, "run the check"),
            memory("w", MemoryScope::Workspace, Some("w1"), "run the check"),
        ];
        let settings = MemorySettings {
            workspace: MemoryMode::Propose,
            ..MemorySettings::default()
        };

        let found = recall(
            &all,
            &request("run the check", settings),
            &FakeProbe::default(),
        );

        assert_eq!(ids(&found), ["w"]);
        assert!(found.considered.iter().all(|c| c.id != "u"));
    }

    #[test]
    fn what_belongs_to_another_workspace_or_agent_is_never_looked_at() {
        let all = [
            memory(
                "other-ws",
                MemoryScope::Workspace,
                Some("w2"),
                "run the check",
            ),
            memory(
                "other-agent",
                MemoryScope::Agent,
                Some("a2"),
                "run the check",
            ),
        ];

        let found = recall(&all, &request("run the check", on()), &FakeProbe::default());

        assert_eq!((found.selected.len(), found.considered.len()), (0, 0));
    }

    #[test]
    fn an_irrelevant_memory_costs_nothing_and_leaves_no_trace_in_the_prompt() {
        let all = [memory("m", MemoryScope::User, None, "run the check")];

        let found = recall(
            &all,
            &request("translate the screen", on()),
            &FakeProbe::default(),
        );

        assert_eq!(*outcome(&found, "m"), RecallOutcome::NoMatch);
        assert_eq!(found.tokens, 0);
    }

    #[test]
    fn candidates_archived_and_stale_memories_are_not_recovered() {
        let mut candidate = memory("c", MemoryScope::User, None, "run the check");
        candidate.status = MemoryStatus::Candidate;
        let mut archived = memory("x", MemoryScope::User, None, "run the check");
        archived.status = MemoryStatus::Archived;
        let mut stale = memory("s", MemoryScope::User, None, "run the check");
        stale.status = MemoryStatus::Stale;

        let found = recall(
            &[candidate, archived, stale],
            &request("run the check", on()),
            &FakeProbe::default(),
        );

        assert_eq!(found, Recall::default());
    }

    #[test]
    fn a_memory_whose_path_is_gone_is_left_out_with_the_reason() {
        let mut lesson = memory("m", MemoryScope::Workspace, Some("w1"), "run the check");
        lesson.validity = MemoryValidity {
            paths: vec!["scripts/check.sh".to_owned()],
            ..MemoryValidity::default()
        };
        let probe = FakeProbe::default();

        let found = recall(&[lesson.clone()], &request("run the check", on()), &probe);
        assert!(
            matches!(outcome(&found, "m"), RecallOutcome::Stale(reason) if reason.contains("scripts/check.sh"))
        );

        probe
            .paths
            .lock()
            .unwrap()
            .insert("scripts/check.sh".to_owned());
        assert_eq!(
            ids(&recall(&[lesson], &request("run the check", on()), &probe)),
            ["m"]
        );
    }

    #[test]
    fn a_script_that_is_no_longer_declared_makes_the_memory_stale_but_no_package_json_does_not() {
        let mut lesson = memory("m", MemoryScope::Workspace, Some("w1"), "run the check");
        lesson.validity.scripts = vec!["check".to_owned()];
        let probe = FakeProbe::default();

        // No package.json: nothing to check against, so the lesson is kept.
        assert_eq!(
            ids(&recall(
                &[lesson.clone()],
                &request("run the check", on()),
                &probe
            )),
            ["m"]
        );

        *probe.scripts.lock().unwrap() = Some(["build".to_owned()].into());
        let found = recall(&[lesson.clone()], &request("run the check", on()), &probe);
        assert!(matches!(outcome(&found, "m"), RecallOutcome::Stale(r) if r.contains("`check`")));

        *probe.scripts.lock().unwrap() = Some(["check".to_owned()].into());
        assert_eq!(
            ids(&recall(&[lesson], &request("run the check", on()), &probe)),
            ["m"]
        );
    }

    #[test]
    fn an_automatic_memory_expires_when_unused_but_one_the_person_wrote_does_not() {
        let mut automatic = memory("auto", MemoryScope::User, None, "run the check");
        automatic.provenance.automatic = true;
        automatic.validity.expires_after_days = Some(1);
        let mut mine = memory("mine", MemoryScope::User, None, "run the check");
        mine.validity.expires_after_days = Some(1);
        let mut req = request("run the check", on());
        req.now = 3 * DAY_MS;

        let found = recall(&[automatic, mine], &req, &FakeProbe::default());

        assert_eq!(*outcome(&found, "auto"), RecallOutcome::Expired);
        assert_eq!(ids(&found), ["mine"]);
    }

    #[test]
    fn the_budget_keeps_the_best_and_says_what_it_left_out() {
        let mut pitfall = memory("p", MemoryScope::User, None, &"windows escape ".repeat(8));
        pitfall.kind = MemoryKind::Pitfall;
        let big = memory(
            "big",
            MemoryScope::User,
            None,
            &"windows escape ".repeat(30),
        );
        let task = "windows escape";
        let one = TextSize::of(&render_line(&pitfall)).estimated_tokens();
        let mut req = request(task, on());
        req.budget_tokens = Some(u32::try_from(one).unwrap() + 2);

        let found = recall(&[big, pitfall], &req, &FakeProbe::default());

        assert_eq!(ids(&found), ["p"]);
        assert_eq!(*outcome(&found, "big"), RecallOutcome::OverBudget);
        assert!(found.tokens <= u64::from(req.budget_tokens.unwrap()));
    }

    #[test]
    fn the_smaller_of_the_settings_and_the_execution_budget_is_the_ceiling() {
        let all = [memory("m", MemoryScope::User, None, "run the check")];
        let mut req = request("run the check", on());
        req.budget_tokens = Some(0);

        assert_eq!(recall(&all, &req, &FakeProbe::default()).selected, []);
        req.budget_tokens = Some(10_000);
        req.settings.max_tokens = 0;
        assert_eq!(recall(&all, &req, &FakeProbe::default()).selected, []);
    }

    #[test]
    fn two_memories_on_one_topic_send_only_the_better_one() {
        let mut first = memory("first", MemoryScope::User, None, "run the check windows");
        first.topic = Some("Testing".to_owned());
        first.uses = 5;
        let mut second = memory("second", MemoryScope::User, None, "run the check windows");
        second.topic = Some(" testing ".to_owned());

        let found = recall(
            &[second, first],
            &request("run the check windows", on()),
            &FakeProbe::default(),
        );

        assert_eq!(ids(&found), ["first"]);
        assert_eq!(
            *outcome(&found, "second"),
            RecallOutcome::Superseded("first".to_owned())
        );
    }

    #[test]
    fn the_same_input_gives_the_same_answer_in_any_order() {
        let a = memory("a", MemoryScope::User, None, "run the check");
        let b = memory("b", MemoryScope::User, None, "run the check");

        let forward = recall(
            &[a.clone(), b.clone()],
            &request("run the check", on()),
            &FakeProbe::default(),
        );
        let backward = recall(
            &[b, a],
            &request("run the check", on()),
            &FakeProbe::default(),
        );

        assert_eq!(ids(&forward), ids(&backward));
    }

    // ----- a flood of notes that share a word (measured on 240 lookalike notes; see the analysis)

    /// A note of the workspace with a title and a content of its own.
    fn note(id: &str, title: &str, content: &str) -> Memory {
        let mut m = memory(id, MemoryScope::Workspace, Some("w1"), content);
        m.title = title.to_owned();
        m
    }

    /// `count` notes that are about other things and share only common words with the task.
    fn lookalikes(count: usize) -> Vec<Memory> {
        (0..count)
            .map(|i| {
                note(
                    &format!("other-{i}"),
                    &format!("About topic{i} handling"),
                    &format!("When you change topic{i}, restart the service: its limit is read at start."),
                )
            })
            .collect()
    }

    const TASK: &str = "Change the retry limit of this service";

    #[test]
    fn with_a_handful_of_matches_every_note_that_shares_a_word_is_kept_as_it_always_was() {
        let notes = lookalikes(5);

        let got = recall(&notes, &request(TASK, on()), &FakeProbe::default());

        assert_eq!(got.selected.len(), 5);
    }

    #[test]
    fn in_a_flood_only_the_notes_about_the_task_stay() {
        let mut notes = lookalikes(14);
        notes.push(note(
            "answer",
            "Retry limit lives in deploy/limits.cfg",
            "The MAXTRY key is the real one; the retries key in config/app.toml is not read.",
        ));

        let got = recall(&notes, &request(TASK, on()), &FakeProbe::default());

        assert_eq!(ids(&got), ["answer"]);
        // the rest are said to be of no use, so the manifest counts them as not relevant
        assert!(matches!(outcome(&got, "other-3"), RecallOutcome::NoMatch));
    }

    #[test]
    fn a_note_that_shares_a_word_few_notes_have_survives_the_flood() {
        let mut notes = lookalikes(14);
        notes.push(note(
            "pm",
            "Package manager",
            "Use pnpm; npm is not installed on the build machines.",
        ));

        let got = recall(
            &notes,
            &request(
                "Add the npm audit step to the service and change its limit",
                on(),
            ),
            &FakeProbe::default(),
        );

        assert_eq!(ids(&got), ["pm"]);
    }

    #[test]
    fn when_nothing_really_matches_a_flood_gets_no_notes_at_all() {
        let notes = lookalikes(14);

        let got = recall(&notes, &request(TASK, on()), &FakeProbe::default());

        assert_eq!(ids(&got), [] as [&str; 0]);
        assert_eq!(got.tokens, 0);
        assert_eq!(got.considered.len(), 14);
    }

    #[test]
    fn a_word_meets_its_other_forms() {
        let notes = [note(
            "n",
            "Retries are capped",
            "The limits live in deploy/limits.cfg.",
        )];

        let got = recall(
            &notes,
            &request("Raise the retry limit", on()),
            &FakeProbe::default(),
        );

        assert_eq!(ids(&got), ["n"]);
    }

    #[test]
    fn what_stays_after_a_flood_is_ordered_by_how_much_the_task_is_about_it() {
        let mut notes = lookalikes(10);
        notes.push(note("weak", "Limit of the service", "Mentioned once."));
        notes.push(note(
            "strong",
            "Retry limit of the service",
            "Retry limit is MAXTRY.",
        ));

        let got = recall(&notes, &request(TASK, on()), &FakeProbe::default());

        assert_eq!(ids(&got).first(), Some(&"strong"));
    }

    #[test]
    fn words_nearly_every_note_has_do_not_lift_a_note_above_the_one_that_was_used() {
        // ten notes on the same subject whose content also says "change" and "service", and the
        // one that was used in three completed steps, which says neither. 51 notes in all, so those
        // two words are in nearly every note while the subject is in a fifth of them.
        let mut notes: Vec<Memory> = (0..10)
            .map(|i| {
                note(
                    &format!("advice-{i}"),
                    "About retry limit",
                    "When you change it, restart the service: it is read once at start.",
                )
            })
            .collect();
        let mut answer = note(
            "answer",
            "Retry limit lives in deploy/limits.cfg",
            "The key is MAXTRY; the retries key in config/app.toml is not read.",
        );
        answer.uses = 3;
        notes.push(answer);
        notes.extend((0..40).map(|i| {
            note(
                &format!("filler-{i}"),
                &format!("Topic{i}"),
                "Restart the service after you change it.",
            )
        }));

        let got = recall(&notes, &request(TASK, on()), &FakeProbe::default());

        // the words they add do not count, so what separates them is what the rank says next
        assert_eq!(ids(&got).first(), Some(&"answer"));
        assert_eq!(got.selected.len(), 11);
    }

    #[test]
    fn between_notes_that_score_alike_the_one_that_has_been_used_comes_first() {
        // Eight rare words in both titles; the other note adds one common word in its content, a
        // few percent of the score. 60 notes in all, and the extra word is in 14 of them.
        let eight = "alfa bravo charlie delta echo foxtrot golf hotel";
        let mut notes: Vec<Memory> = (0..58)
            .map(|i| {
                let content = if i < 13 { "india" } else { "Unrelated words." };
                note(&format!("filler-{i}"), &format!("Topic{i}"), content)
            })
            .collect();
        let mut tried = note("tried", &format!("Notes on {eight}"), "MAXTRY.");
        tried.uses = 3;
        notes.push(tried);
        notes.push(note("higher", &format!("Also {eight}"), "india"));
        // enough other notes about the subject for this to be a flood
        notes.extend((0..4).map(|i| note(&format!("side-{i}"), "alfa bravo only", "x")));

        let got = recall(
            &notes,
            &request(&format!("Change {eight} and india"), on()),
            &FakeProbe::default(),
        );

        let order = ids(&got);
        let position = |id| order.iter().position(|x| *x == id).unwrap();
        assert!(position("tried") < position("higher"), "{order:?}");
    }

    #[test]
    fn a_small_memory_about_one_subject_does_not_call_its_own_subject_generic() {
        // eight notes, six of which mention "build": here that word is the subject, not noise
        let mut notes: Vec<Memory> = (0..6)
            .map(|i| {
                note(
                    &format!("b{i}"),
                    &format!("Build tip {i}"),
                    "Keep the build cache warm.",
                )
            })
            .collect();
        notes.push(note("x", "Deploy", "Use the blue slot."));
        notes.push(note("y", "Review", "Two approvals."));

        let got = recall(
            &notes,
            &request("Speed up the build", on()),
            &FakeProbe::default(),
        );

        // every build note shares the title word and the weight is not zeroed
        assert_eq!(got.selected.len(), 6);
    }

    // ----- the edges of the pruning (each of these would pass with the pruning removed only if the
    // pool or the number of matches were on the safe side of the edge, which is what they check)

    /// `matching` notes that share only the word "service" with the task, and `others` that share
    /// nothing: a memory of `matching + others` notes in which nothing is about the task.
    fn only_common_words(matching: usize, others: usize) -> Vec<Memory> {
        let mut notes: Vec<Memory> = (0..matching)
            .map(|i| {
                note(
                    &format!("m{i}"),
                    &format!("Topic{i}"),
                    "Restart the service after deploy.",
                )
            })
            .collect();
        notes.extend(
            (0..others).map(|i| note(&format!("o{i}"), &format!("Other{i}"), "Unrelated words.")),
        );
        notes
    }

    #[test]
    fn five_matches_are_not_a_flood_and_six_are() {
        let ask = |n| {
            recall(
                &only_common_words(n, 14 - n),
                &request("Check the service", on()),
                &FakeProbe::default(),
            )
        };

        assert_eq!(ask(5).selected.len(), 5);
        assert_eq!(ask(6).selected.len(), 0);
    }

    #[test]
    fn a_memory_of_eleven_notes_is_never_pruned_and_one_of_twelve_is() {
        let ask = |others| {
            recall(
                &only_common_words(6, others),
                &request("Check the service", on()),
                &FakeProbe::default(),
            )
        };

        assert_eq!(ask(5).selected.len(), 6, "11 notes");
        assert_eq!(ask(6).selected.len(), 0, "12 notes");
    }

    #[test]
    fn in_a_flood_a_pitfall_comes_before_a_decision_that_scores_the_same() {
        use crate::domain::memory::MemoryKind;
        let mut notes: Vec<Memory> = (0..3)
            .map(|i| {
                let mut m = note(&format!("decision-{i}"), "Retry limit choice", "Chosen.");
                m.kind = MemoryKind::Decision;
                m
            })
            .collect();
        notes.extend((0..3).map(|i| {
            let mut m = note(&format!("pitfall-{i}"), "Retry limit trap", "Careful.");
            m.kind = MemoryKind::Pitfall;
            m
        }));
        notes.extend(
            (0..8).map(|i| note(&format!("o{i}"), &format!("Other{i}"), "Unrelated words.")),
        );

        let got = recall(
            &notes,
            &request("Change the retry limit", on()),
            &FakeProbe::default(),
        );

        let order = ids(&got);
        assert_eq!(order.len(), 6);
        assert!(
            order[..3].iter().all(|id| id.starts_with("pitfall")),
            "{order:?}"
        );
    }

    #[test]
    fn a_note_with_one_uncommon_word_in_its_title_survives_in_a_small_flood() {
        // 15 notes; six share "build" or "service" with the task, and the docker note (one title
        // word, not rare: three notes mention it) must not be turned away for being short of two
        let mut notes: Vec<Memory> = (0..5)
            .map(|i| {
                note(
                    &format!("b{i}"),
                    &format!("Build tip {i}"),
                    "Run it on the service.",
                )
            })
            .collect();
        notes.push(note(
            "docker",
            "Docker caching",
            "Layer cache, docker layers.",
        ));
        notes.push(note("d2", "CI", "We use docker there."));
        notes.push(note("d3", "Registry", "Push the docker image."));
        notes.extend(
            (0..7).map(|i| note(&format!("o{i}"), &format!("Other{i}"), "Unrelated words.")),
        );

        let got = recall(
            &notes,
            &request("Add the docker build step to the service", on()),
            &FakeProbe::default(),
        );

        assert!(ids(&got).contains(&"docker"), "{:?}", ids(&got));
    }

    #[test]
    #[allow(clippy::too_many_lines)] // the notes of the memory are the test
    fn a_hand_written_memory_finds_what_each_task_needs_and_little_else() {
        // 24 notes written the way a team would, not generated: the check that the rules were not
        // only tuned to the benchmark's templates
        let corpus = [
            (
                "pnpm",
                "Package manager",
                "Use pnpm, never npm: the lockfile is pnpm-lock.yaml.",
            ),
            (
                "dev",
                "Run the app",
                "Start the desktop app with npm run tauri dev; vite serves port 1420.",
            ),
            (
                "fmt",
                "Rust formatting",
                "Run cargo fmt before committing; CI fails on unformatted files.",
            ),
            (
                "clippy",
                "Clippy is strict",
                "cargo clippy runs with -D warnings, so fix every warning.",
            ),
            (
                "i18n",
                "Translations",
                "Every user-visible string needs a key in pt-BR and en-US.",
            ),
            (
                "wt",
                "Git worktrees",
                "Agents work in a worktree under .atlas/worktrees; never edit main directly.",
            ),
            (
                "mig",
                "Config migration",
                "Old config.json files are migrated on load; keep the serde defaults.",
            ),
            (
                "sbx",
                "macOS sandbox",
                "sandbox-exec profiles allow writes only to the worktree and tmp.",
            ),
            (
                "flaky",
                "Flaky timing tests",
                "live_workspace timing tests fail when two cargo runs overlap; rerun alone.",
            ),
            (
                "mcp",
                "MCP servers",
                "A stdio MCP server needs an executable and args; secrets live in the keychain.",
            ),
            (
                "prompt",
                "Prompt sections",
                "Memory notes are untrusted context under a notice, never instructions.",
            ),
            (
                "usage",
                "Usage ledger",
                "Token usage is recorded per execution in the usage ledger.",
            ),
            (
                "remote",
                "Mobile access",
                "The phone connects over the LAN with a QR code that is valid once.",
            ),
            (
                "css",
                "Styling",
                "Components use CSS modules and design tokens, not inline styles.",
            ),
            (
                "fe",
                "Frontend tests",
                "Vitest with testing-library; mock services with vi.mock.",
            ),
            (
                "ids",
                "Identifiers",
                "Ids come from new_id and are never reused.",
            ),
            (
                "adr",
                "ADR numbering",
                "Architecture decisions are numbered in docs/architecture/adr.",
            ),
            (
                "secrets",
                "Secrets",
                "Never log tokens; redact_secrets runs before anything is stored.",
            ),
            (
                "log",
                "Logging",
                "Use eprintln only for degraded paths; there is no tracing crate.",
            ),
            (
                "release",
                "Release",
                "The version lives in tauri.conf.json and Cargo.toml.",
            ),
            (
                "perf",
                "Performance",
                "Avoid cloning the whole config; read through ConfigRepository::read.",
            ),
            (
                "errors",
                "Error codes",
                "A new error needs an ErrorCode and a message in both languages.",
            ),
            ("hooks", "Git hooks", "Do not skip hooks with --no-verify."),
            (
                "design",
                "Design source",
                "The UI follows the Stitch design system tokens.",
            ),
        ];
        let notes: Vec<Memory> = corpus.iter().map(|(id, t, c)| note(id, t, c)).collect();
        let cases: [(&str, &[&str]); 6] = [
            ("Add a new error code for a failed export", &["errors"]),
            (
                "Format the Rust code and fix the clippy warnings",
                &["fmt", "clippy"],
            ),
            (
                "Install the dependencies with the package manager",
                &["pnpm"],
            ),
            ("Add the pt-BR translation for the new string", &["i18n"]),
            (
                "Make the timing tests stable in the live workspace",
                &["flaky"],
            ),
            ("Let agents edit files safely in a worktree", &["wt", "sbx"]),
        ];

        for (task, expected) in cases {
            let got = recall(&notes, &request(task, on()), &FakeProbe::default());
            let found = ids(&got);
            for id in expected {
                assert!(
                    found.contains(id),
                    "{task:?} should find {id}, found {found:?}"
                );
            }
            assert!(found.len() <= 6, "{task:?} brought {found:?}");
        }
    }

    #[test]
    fn a_note_with_two_uncommon_words_in_its_content_stays_but_one_with_a_single_word_does_not() {
        // 14 notes; six mention "service"; the note about tests shares "timing" and "fail" with the
        // task but not in its title, and the unrelated one shares only "fail"
        let mut notes: Vec<Memory> = (0..6)
            .map(|i| {
                note(
                    &format!("s{i}"),
                    &format!("Topic{i}"),
                    "Restart the service after deploy.",
                )
            })
            .collect();
        notes.push(note(
            "about",
            "Cargo runs",
            "Timing tests fail when two runs overlap.",
        ));
        notes.push(note("single", "Release", "A release can fail."));
        // neither "timing" nor "fail" is rare, because two other notes have each: so only the
        // two-word rule can keep "about"
        notes.push(note("t", "Clock", "Timing is logged."));
        notes.push(note("f", "Build", "Some builds fail."));
        notes.extend(
            (0..6).map(|i| note(&format!("o{i}"), &format!("Other{i}"), "Unrelated words.")),
        );

        let got = recall(
            &notes,
            &request("Why do the timing checks fail on the service", on()),
            &FakeProbe::default(),
        );

        let found = ids(&got);
        assert!(found.contains(&"about"), "{found:?}");
        assert!(!found.contains(&"single"), "{found:?}");
    }
}
