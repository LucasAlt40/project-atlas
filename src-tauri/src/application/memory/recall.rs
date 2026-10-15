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
    format!(
        "- [{kind}] {}: {}",
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
    let task_terms: BTreeSet<String> = memory::terms(req.task);

    let mut candidates: Vec<(&Memory, u32)> = Vec::new();
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
        } else if let Some(score) = memory::relevance(item, &task_terms) {
            candidates.push((item, score));
        } else {
            result
                .considered
                .push(Considered::of(item, RecallOutcome::NoMatch));
        }
    }
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
}
