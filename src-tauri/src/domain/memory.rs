//! Memory: short, durable lessons that agents (and people) leave for the next session.
//!
//! A Memory is **not** project knowledge (that is the Harness), **not** an instruction (that is a
//! Rule) and **not** a transcript (that is the history). It is a hint with an owner, an origin and
//! a validity, delivered as `Untrusted` context: it can inform an agent, never bind one, and
//! nothing it says grants or lifts anything (see `ContextAuthority`).
//!
//! Everything here is pure: the same memories and the same task always give the same answer.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Whether Atlas recovers memories and lets agents add to them. `Off` is what an absent setting
/// reads as; a person who never chose gets [`MemorySettings::recommended`] instead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMode {
    /// Nothing is recovered or proposed; the prompt is the one there was before memories.
    #[default]
    Off,
    /// Memories are recovered, and agents may propose new ones that a person decides on.
    Propose,
    /// As `Propose`, and proposals that pass every check enter by themselves (ADR 0037).
    Auto,
}

/// The default ceiling, in estimated tokens, of the memories one prompt carries.
pub const DEFAULT_MAX_TOKENS: u32 = 600;

/// The settings of the Memory, one mode per scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySettings {
    #[serde(default)]
    pub user: MemoryMode,
    #[serde(default)]
    pub workspace: MemoryMode,
    #[serde(default)]
    pub agent: MemoryMode,
    /// The most the memories of a prompt may add, in estimated tokens.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// The person saved these modes in the settings. Until they do, Atlas uses
    /// [`MemorySettings::recommended`] (see `ConfigRepository::load`).
    #[serde(default)]
    pub chosen: bool,
}

fn default_max_tokens() -> u32 {
    DEFAULT_MAX_TOKENS
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            user: MemoryMode::Off,
            workspace: MemoryMode::Off,
            agent: MemoryMode::Off,
            max_tokens: DEFAULT_MAX_TOKENS,
            chosen: false,
        }
    }
}

impl MemorySettings {
    /// What Atlas uses until the person chooses (ADR 0041): every scope recalls, agents may propose
    /// notes, and nothing enters without the person. Measured (M4, ADR 0040): a note the person
    /// approved stopped four runtimes from editing a decoy file (0/3 against 3/3) and cut Claude's
    /// input by about half; with no matching note the prompt only gains the proposal rule. `Auto`
    /// is never a default: agents wrote false lessons that passed every check.
    pub const fn recommended() -> Self {
        Self {
            user: MemoryMode::Propose,
            workspace: MemoryMode::Propose,
            agent: MemoryMode::Propose,
            max_tokens: DEFAULT_MAX_TOKENS,
            chosen: false,
        }
    }

    pub const fn mode(self, scope: MemoryScope) -> MemoryMode {
        match scope {
            MemoryScope::User => self.user,
            MemoryScope::Workspace => self.workspace,
            MemoryScope::Agent => self.agent,
        }
    }

    /// Whether an agent may propose notes: only the workspace and agent scopes take proposals
    /// (what speaks for the person, the `User` scope, is theirs to write).
    pub fn proposals_on(self) -> bool {
        self.workspace != MemoryMode::Off || self.agent != MemoryMode::Off
    }

    /// Whether any scope is on; when none is, memory leaves no trace in a prompt.
    pub fn any_on(self) -> bool {
        [self.user, self.workspace, self.agent]
            .iter()
            .any(|m| *m != MemoryMode::Off)
    }
}

/// A memory kept by a server on this machine that is not Atlas (the `ai-memory` project, ADR 0040).
/// Off by default: it adds text to prompts. Atlas only reads from it before a run; the server is
/// started and kept by the person, and nothing here reaches a machine other than this one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalMemorySettings {
    #[serde(default)]
    pub enabled: bool,
    /// Where the server listens. Only a loopback address is accepted: the token never leaves the
    /// machine.
    #[serde(default = "default_external_url")]
    pub server_url: String,
    /// The server's data directory: Atlas reads its access token from there, never from a setting.
    #[serde(default)]
    pub data_dir: String,
    /// The most notes one prompt may take from it.
    #[serde(default = "default_external_hits")]
    pub max_hits: u8,
    /// The most the notes of the server may add to a prompt, in estimated tokens.
    #[serde(default = "default_external_tokens")]
    pub max_tokens: u32,
    /// The server's workspace that holds the person's projects. `default` is where its own tools
    /// put a project unless told otherwise, so notes are seen by both sides.
    #[serde(default = "default_external_workspace")]
    pub workspace: String,
    /// Also publish the notes a person approved in Atlas (workspace scope) to the server, so the
    /// person's other tools see them. Off by default: it writes outside Atlas's own data.
    #[serde(default)]
    pub share: bool,
    /// Only bring a note whose title shares words with the task (two, or the one the task has).
    /// Measured against a server of 240 lookalike notes: a task worded differently from the note
    /// otherwise got two notes about other things in every prompt. A note with a title that does not
    /// echo the task is not brought; turn this off to take whatever the search finds.
    #[serde(default = "default_external_precise")]
    pub precise: bool,
}

fn default_external_url() -> String {
    "http://127.0.0.1:49374".to_owned()
}

fn default_external_workspace() -> String {
    "default".to_owned()
}

const fn default_external_precise() -> bool {
    true
}

const fn default_external_hits() -> u8 {
    3
}

const fn default_external_tokens() -> u32 {
    400
}

impl Default for ExternalMemorySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: default_external_url(),
            data_dir: String::new(),
            max_hits: default_external_hits(),
            max_tokens: default_external_tokens(),
            workspace: default_external_workspace(),
            share: false,
            precise: default_external_precise(),
        }
    }
}

impl ExternalMemorySettings {
    /// The `host:port` of the server when the URL is plain `http` on a loopback address, and `None`
    /// for anything else. No other host is ever contacted.
    pub fn loopback_authority(&self) -> Option<String> {
        let authority = self
            .server_url
            .strip_prefix("http://")?
            .trim_end_matches('/');
        if authority.contains(['/', '@', '?']) {
            return None;
        }
        let (host, port) = authority.rsplit_once(':')?;
        port.parse::<u16>().ok()?;
        matches!(host, "127.0.0.1" | "localhost" | "[::1]").then(|| authority.to_owned())
    }
}

/// How many thrown-away runs make an agent's note one to review (see [`Memory::misleads`]).
pub const MISLED_TO_REVIEW: u32 = 2;
/// The share of words, in percent, two lessons must have in common to be the same one: of their
/// contents, or of title and content together. Tried on the notes real runs proposed (ADR 0041).
const RESEMBLES_CONTENT_PERCENT: usize = 60;
const RESEMBLES_ALL_PERCENT: usize = 66;
/// Fewer distinct words than this between two texts is too little to call them the same.
const RESEMBLES_MIN_WORDS: usize = 3;

/// Words that turn a statement into its opposite, in English, Portuguese and Spanish. They are
/// short or common, so [`terms`] drops most of them; [`Memory::resembles`] looks at them apart.
const NEGATIONS: &[&str] = &[
    "not", "no", "never", "nothing", "none", "nobody", "neither", "nor", "without", "cannot",
    "don't", "doesn't", "didn't", "isn't", "aren't", "wasn't", "weren't", "won't", "can't",
    "unused", "ignored", "ignores", "dead", "não", "nao", "nunca", "nada", "nenhum", "nenhuma",
    "sem", "jamais", "nem", "sin", "ningún", "ninguna", "tampoco",
];

/// Whether a text says that something is not so (see [`NEGATIONS`]).
fn negated(text: &str) -> bool {
    text.to_lowercase()
        .replace('\u{2019}', "'")
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .any(|word| NEGATIONS.contains(&word))
}

/// The longest a lesson may be. A lesson that does not fit is a fact about the project and belongs
/// in the Harness.
pub const MAX_CONTENT_CHARS: usize = 480;
/// The longest title.
pub const MAX_TITLE_CHARS: usize = 80;

/// Whose memory it is. Project knowledge is not a scope: it is the Harness, and what is left of
/// "project" folds into the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    /// The user, across every project.
    User,
    /// One workspace.
    Workspace,
    /// One agent; never shared with another by itself.
    Agent,
}

/// What kind of lesson it is. It decides the order under a budget and the way the prompt frames
/// it; it decides no permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// A command, an order of steps or a tool that works.
    HowTo,
    /// What failed and why.
    Pitfall,
    /// How the person wants the result.
    Preference,
    /// A choice that was made, and the reason.
    Decision,
}

impl MemoryKind {
    /// Lower first: what saves the most retries comes first when the budget is short.
    const fn rank(self) -> u8 {
        match self {
            Self::Pitfall => 0,
            Self::HowTo => 1,
            Self::Preference => 2,
            Self::Decision => 3,
        }
    }
}

/// Who wrote it. It limits how far the memory is believed, never what it says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOrigin {
    /// Written, or approved, by the person.
    User,
    /// Proposed by an agent and accepted (by the person or by the automatic mode).
    Agent,
    /// Brought in from a file the person imported.
    Imported,
}

/// Where a memory came from, for the person who wonders why the agent knew it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryProvenance {
    pub origin: MemoryOrigin,
    /// The execution and step that produced it, when an agent did. A name, never a secret.
    #[serde(default)]
    pub execution_id: Option<String>,
    /// `true` when no person decided it (automatic mode): the note is **unconfirmed**. It stays so
    /// until a person approves, edits or promotes it, whatever else vouched for it.
    #[serde(default)]
    pub automatic: bool,
    /// The agent whose step proposed it. A validator must be someone else.
    #[serde(default)]
    pub agent_id: Option<String>,
    /// The execution of the independent validator whose `pass` let the note in by itself (see
    /// `MemoryService::confirm_held`). `None` for a note a person decided, and for one still held.
    #[serde(default)]
    pub validated_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    /// Waiting for a person to approve, edit or discard it. Never recovered.
    Candidate,
    /// May be recovered.
    Active,
    /// Its premise no longer holds (a path or a script is gone). Not recovered; the person may
    /// renew or delete it.
    Stale,
    /// Retired by age, quota or the person. Kept for a while, never recovered.
    Archived,
    /// A person turned this proposal down. Never recovered; kept (up to [`MAX_REJECTED`] per
    /// owner) so that an agent proposing the same lesson again is not asked about twice.
    Rejected,
}

/// The most turned-down proposals kept per scope and owner; the oldest go first.
pub const MAX_REJECTED: usize = 100;

/// How a memory is checked against the project before it is recovered, when that needs no model.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryValidity {
    /// Project-relative paths the lesson depends on.
    #[serde(default)]
    pub paths: Vec<String>,
    /// `package.json` scripts the lesson depends on (`check` for `npm run check`).
    #[serde(default)]
    pub scripts: Vec<String>,
    /// After this many days without use an automatic memory is archived. `None`: never.
    #[serde(default)]
    pub expires_after_days: Option<u32>,
}

/// One lesson.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    pub id: String,
    pub scope: MemoryScope,
    /// What the scope is about: the workspace or agent id. `None` for `User`.
    #[serde(default)]
    pub owner: Option<String>,
    pub kind: MemoryKind,
    pub title: String,
    pub content: String,
    /// Memories on one topic compete (exact, case-insensitive key); Atlas does not judge meaning.
    #[serde(default)]
    pub topic: Option<String>,
    pub provenance: MemoryProvenance,
    pub status: MemoryStatus,
    #[serde(default)]
    pub validity: MemoryValidity,
    /// How many completed steps received it in their prompt.
    #[serde(default)]
    pub uses: u32,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub last_used_at: Option<u64>,
    /// Runs that carried it and that the person then took (merged, or kept applied).
    #[serde(default)]
    pub helped: u32,
    /// Runs that carried it and that the person then threw away (discarded, or took back).
    #[serde(default)]
    pub misled: u32,
}

/// Why a memory may not be stored as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    EmptyTitle,
    EmptyContent,
    TitleTooLong,
    ContentTooLong,
    /// A `Workspace` or `Agent` memory needs the id of its owner; `User` has none.
    OwnerMismatch,
}

impl MemoryError {
    /// A stable code the interface words in the person's language.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EmptyTitle => "empty_title",
            Self::EmptyContent => "empty_content",
            Self::TitleTooLong => "title_too_long",
            Self::ContentTooLong => "content_too_long",
            Self::OwnerMismatch => "owner_mismatch",
        }
    }
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::EmptyTitle => "a memory needs a title",
            Self::EmptyContent => "a memory needs content",
            Self::TitleTooLong => "the title of a memory is too long",
            Self::ContentTooLong => {
                "a memory is a short lesson: what does not fit is knowledge of the project and belongs in the Harness"
            }
            Self::OwnerMismatch => "the owner does not match the scope of the memory",
        })
    }
}

impl Memory {
    /// Whether the memory is in the shape it may be stored in.
    ///
    /// # Errors
    ///
    /// Says what is wrong with it.
    pub fn validate(&self) -> Result<(), MemoryError> {
        if self.title.trim().is_empty() {
            return Err(MemoryError::EmptyTitle);
        }
        if self.content.trim().is_empty() {
            return Err(MemoryError::EmptyContent);
        }
        if self.title.chars().count() > MAX_TITLE_CHARS {
            return Err(MemoryError::TitleTooLong);
        }
        if self.content.chars().count() > MAX_CONTENT_CHARS {
            return Err(MemoryError::ContentTooLong);
        }
        let has_owner = self.owner.as_deref().is_some_and(|o| !o.trim().is_empty());
        if has_owner != (self.scope != MemoryScope::User) {
            return Err(MemoryError::OwnerMismatch);
        }
        Ok(())
    }

    pub fn topic_key(&self) -> Option<String> {
        self.topic
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_lowercase)
    }

    /// Waiting for an independent validator: proposed in automatic mode, not yet let in, not yet
    /// decided by a person.
    pub fn is_held_for_validator(&self) -> bool {
        self.status == MemoryStatus::Candidate && self.provenance.automatic
    }

    /// No person has decided it. It may still be recalled (a validator vouched for it), but it is
    /// shown as unconfirmed and read with less trust.
    pub fn is_unconfirmed(&self) -> bool {
        self.provenance.automatic
    }

    /// What the person's verdicts on the runs that carried it say, kept small: a run they took
    /// counts one, a run they threw away counts two (being wrong costs more than being right
    /// earns). Decides between notes as relevant as each other, before use and recency.
    pub fn trust(&self) -> i64 {
        i64::from(self.helped) - 2 * i64::from(self.misled)
    }

    /// An agent's note that was in runs the person threw away at least [`MISLED_TO_REVIEW`] times
    /// and in more of those than of runs they took: likely wrong, so it stops being used until the
    /// person looks at it. A note a person wrote is only ranked lower, never moved by this.
    pub fn misleads(&self) -> bool {
        self.provenance.origin != MemoryOrigin::User
            && self.misled >= MISLED_TO_REVIEW
            && self.misled > self.helped
    }

    /// Whether `title` and `content` say the lesson this one says: most of the words of the content
    /// (stems, see [`stems`]) are the same, or most of the words of title and content together.
    /// A title alone never decides, since "Use pnpm" and "Use npm" under one title are opposite
    /// lessons a person must see; and a text of fewer than [`RESEMBLES_MIN_WORDS`] words is judged
    /// on nothing. Recognises a lesson already turned down or kept; not a judge of meaning.
    pub fn resembles(&self, title: &str, content: &str) -> bool {
        let share = |a: &BTreeSet<String>, b: &BTreeSet<String>, percent: usize| {
            let all = a.union(b).count();
            all >= RESEMBLES_MIN_WORDS && a.intersection(b).count() * 100 >= percent * all
        };
        let (mine, theirs) = (stems(&self.content), stems(content));
        let whole = |t: &str, c: &BTreeSet<String>| -> BTreeSet<String> {
            stems(t).union(c).cloned().collect()
        };
        // "is read" and "is not read" share every word but the one the matching leaves out: a
        // lesson and its negation are never the same one.
        if negated(&format!("{} {}", self.title, self.content))
            != negated(&format!("{title} {content}"))
        {
            return false;
        }
        share(&mine, &theirs, RESEMBLES_CONTENT_PERCENT)
            || share(
                &whole(&self.title, &mine),
                &whole(title, &theirs),
                RESEMBLES_ALL_PERCENT,
            )
    }

    /// Whether this memory applies to an execution of `workspace` by `agent`.
    pub fn applies_to(&self, workspace: &str, agent: &str) -> bool {
        match self.scope {
            MemoryScope::User => true,
            MemoryScope::Workspace => self.owner.as_deref() == Some(workspace),
            MemoryScope::Agent => self.owner.as_deref() == Some(agent),
        }
    }
}

/// Words that say nothing about a task in English, Portuguese and Spanish. A lesson that matched
/// "the" or "para" would be recovered for every task and cost tokens for nothing.
const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "any", "can", "had", "her", "was",
    "one", "our", "out", "has", "have", "with", "this", "that", "from", "they", "will", "would",
    "there", "their", "what", "when", "where", "which", "while", "your", "then", "than", "into",
    "should", "must", "does", "did", "its", "also", "use", "using", "run", "before", "after",
    "para", "com", "uma", "uns", "umas", "dos", "das", "nos", "nas", "por", "que", "não", "nao",
    "como", "mais", "mas", "foi", "são", "sao", "tem", "ser", "seu", "sua", "isso", "esta", "este",
    "esse", "essa", "ele", "ela", "quando", "onde", "antes", "depois", "sem", "sobre", "los",
    "las", "una", "unos", "unas", "del", "con", "por", "para", "pero", "como", "más", "mas", "fue",
    "son", "esta", "este", "eso", "esa", "cuando", "donde", "antes", "después", "sin", "sobre",
];

/// The words of a text that carry meaning for matching: lowercase alphanumerics of three or more
/// characters that are not [`STOP_WORDS`].
pub fn terms(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .map(str::to_lowercase)
        .filter(|w| !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

/// A word without its common endings, the same way for a note and for a task, so `retry` meets
/// `retries`, `limit` meets `limits`, `change` meets `changed`, `log` meets `logging`. The memory
/// server's own search does this too. Crude on purpose: both sides go through it, so a bad cut
/// still compares equal. It cuts only when a stem of four letters or more is left (`string` is not
/// `str`) and keeps a final `e` on short words (`pipe` is not `pip`, `rate` is not `rat`); a word
/// it does not know how to cut is left alone, which costs a missed match and never a false one.
pub fn stem(word: &str) -> String {
    let mut w = word.to_lowercase();
    // plural: retries -> retry, limits -> limit; not class, status, analysis
    if w.len() > 4 && w.ends_with("ies") {
        w.truncate(w.len() - 3);
        w.push('y');
    } else if w.len() > 3
        && w.ends_with('s')
        && !(w.ends_with("ss") || w.ends_with("us") || w.ends_with("is"))
    {
        w.pop();
    }
    // verb endings: changed -> chang, logging -> log, settings -> setting -> set
    for suffix in ["ing", "ed"] {
        if let Some(rest) = w.strip_suffix(suffix) {
            if rest.chars().count() >= 4 {
                w = undouble(rest);
                break;
            }
        }
    }
    // change -> chang, to meet changed; short words keep it (pipe, rate)
    if w.len() >= 5 && w.ends_with('e') {
        w.pop();
    }
    w
}

/// `logg` -> `log`, `stopp` -> `stop`; not `install` or `process`, whose double letter is the word's.
fn undouble(stem: &str) -> String {
    let mut chars = stem.chars().rev();
    match (chars.next(), chars.next()) {
        (Some(a), Some(b)) if a == b && a.is_alphabetic() && !"lszaeiou".contains(a) => {
            stem[..stem.len() - a.len_utf8()].to_owned()
        }
        _ => stem.to_owned(),
    }
}

/// [`terms`] of a text, each through [`stem`]: what a note and a task are matched on.
pub fn stems(text: &str) -> BTreeSet<String> {
    terms(text).iter().map(|t| stem(t)).collect()
}

/// What separates two notes that share the same words: a pitfall before a how-to before a
/// decision, and what a person wrote before what an agent proposed. Always below 10.
pub fn tie_bonus(memory: &Memory) -> u32 {
    u32::from(3 - memory.kind.rank()) + u32::from(memory.provenance.origin == MemoryOrigin::User)
}

/// What a memory shares with a task: the words, and how they add up to a relevance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fit {
    /// Higher is better; see [`relevance`].
    pub score: u32,
    /// The task's words found in the title or topic.
    pub title: BTreeSet<String>,
    /// The task's words found only in the content.
    pub content: BTreeSet<String>,
}

/// What `memory` shares with a task (`task_stems` come from [`stems`]), or `None` when nothing.
/// A memory that shares no term is never recovered: a lesson that has nothing to do with the task
/// is a cost with no benefit.
pub fn fit(memory: &Memory, task_stems: &BTreeSet<String>) -> Option<Fit> {
    let title_terms = stems(&format!(
        "{} {}",
        memory.title,
        memory.topic.as_deref().unwrap_or("")
    ));
    let content_terms = stems(&memory.content);
    let title: BTreeSet<String> = title_terms.intersection(task_stems).cloned().collect();
    let content: BTreeSet<String> = content_terms
        .difference(&title_terms)
        .filter(|t| task_stems.contains(*t))
        .cloned()
        .collect();
    if title.len() + content.len() == 0 {
        return None;
    }
    let mut score = u32::try_from(title.len() * 2 + content.len()).unwrap_or(u32::MAX);
    score = score.saturating_mul(10);
    score += tie_bonus(memory);
    Some(Fit {
        score,
        title,
        content,
    })
}

/// What [`rank`] sorts by.
pub type RankKey = (
    std::cmp::Reverse<u32>,
    std::cmp::Reverse<i64>,
    std::cmp::Reverse<u32>,
    std::cmp::Reverse<u64>,
    String,
);

/// The sort key of a recovered memory: relevance first, then what the person's verdicts on past
/// runs say ([`Memory::trust`]), then use, then recency, then id so the order is total and the
/// same on every run.
pub fn rank(memory: &Memory, relevance: u32) -> RankKey {
    (
        std::cmp::Reverse(relevance),
        std::cmp::Reverse(memory.trust()),
        std::cmp::Reverse(memory.uses),
        std::cmp::Reverse(memory.last_used_at.unwrap_or(memory.updated_at)),
        memory.id.clone(),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn memory(
        id: &str,
        scope: MemoryScope,
        owner: Option<&str>,
        content: &str,
    ) -> Memory {
        Memory {
            id: id.to_owned(),
            scope,
            owner: owner.map(str::to_owned),
            kind: MemoryKind::HowTo,
            title: id.to_owned(),
            content: content.to_owned(),
            topic: None,
            provenance: MemoryProvenance {
                origin: MemoryOrigin::User,
                execution_id: None,
                automatic: false,
                agent_id: None,
                validated_by: None,
            },
            status: MemoryStatus::Active,
            validity: MemoryValidity::default(),
            uses: 0,
            created_at: 1,
            updated_at: 1,
            last_used_at: None,
            helped: 0,
            misled: 0,
        }
    }

    fn task(text: &str) -> BTreeSet<String> {
        stems(text)
    }

    fn relevance(memory: &Memory, task_stems: &BTreeSet<String>) -> Option<u32> {
        fit(memory, task_stems).map(|f| f.score)
    }

    #[test]
    fn a_lesson_that_does_not_fit_is_refused_and_says_where_it_belongs() {
        let mut long = memory(
            "m",
            MemoryScope::User,
            None,
            &"x".repeat(MAX_CONTENT_CHARS + 1),
        );
        assert_eq!(long.validate(), Err(MemoryError::ContentTooLong));
        long.content = "x".repeat(MAX_CONTENT_CHARS);
        assert_eq!(long.validate(), Ok(()));
        assert!(MemoryError::ContentTooLong.to_string().contains("Harness"));
    }

    #[test]
    fn the_owner_must_match_the_scope() {
        assert_eq!(
            memory("m", MemoryScope::Workspace, None, "c").validate(),
            Err(MemoryError::OwnerMismatch)
        );
        assert_eq!(
            memory("m", MemoryScope::User, Some("w"), "c").validate(),
            Err(MemoryError::OwnerMismatch)
        );
        assert_eq!(
            memory("m", MemoryScope::Agent, Some("a"), "c").validate(),
            Ok(())
        );
    }

    #[test]
    fn a_memory_applies_only_to_its_own_workspace_and_agent() {
        let user = memory("u", MemoryScope::User, None, "c");
        let workspace = memory("w", MemoryScope::Workspace, Some("w1"), "c");
        let agent = memory("a", MemoryScope::Agent, Some("a1"), "c");

        assert!(user.applies_to("any", "any"));
        assert!(workspace.applies_to("w1", "other") && !workspace.applies_to("w2", "a1"));
        assert!(agent.applies_to("other", "a1") && !agent.applies_to("w1", "a2"));
    }

    #[test]
    fn a_lesson_with_nothing_in_common_with_the_task_is_never_recovered() {
        let lesson = memory(
            "m",
            MemoryScope::User,
            None,
            "Run npm run check before committing",
        );

        assert_eq!(
            relevance(&lesson, &task("Translate the settings screen")),
            None
        );
        assert!(relevance(&lesson, &task("Fix the failing check before the commit")).is_some());
    }

    #[test]
    fn the_title_weighs_more_than_the_content_and_a_pitfall_more_than_a_decision() {
        let mut in_title = memory("m1", MemoryScope::User, None, "unrelated words here");
        in_title.title = "Windows escape".to_owned();
        let in_content = memory("m2", MemoryScope::User, None, "Windows escape breaks");
        let wanted = task("windows escape problem");
        assert!(relevance(&in_title, &wanted) > relevance(&in_content, &wanted));

        let mut pitfall = memory("p", MemoryScope::User, None, "windows escape");
        pitfall.kind = MemoryKind::Pitfall;
        let mut decision = pitfall.clone();
        decision.id = "d".to_owned();
        decision.kind = MemoryKind::Decision;
        assert!(relevance(&pitfall, &wanted) > relevance(&decision, &wanted));
    }

    #[test]
    fn words_that_say_nothing_never_make_a_lesson_relevant_in_any_of_the_three_languages() {
        let lesson = memory(
            "m",
            MemoryScope::User,
            None,
            "the build com uma tarea para los tests",
        );

        for task in ["the and for", "para com uma", "los del con"] {
            assert_eq!(relevance(&lesson, &stems(task)), None, "{task}");
        }
        assert!(relevance(&lesson, &stems("fix the tests")).is_some());
    }

    #[test]
    fn the_ranking_is_total_so_the_same_input_gives_the_same_order() {
        let a = memory("a", MemoryScope::User, None, "same");
        let b = memory("b", MemoryScope::User, None, "same");

        assert!(rank(&a, 10) < rank(&b, 10));
        let mut used = b.clone();
        used.uses = 3;
        assert!(rank(&used, 10) < rank(&a, 10));
    }

    #[test]
    fn a_stored_memory_without_the_optional_fields_loads() {
        let stored: Memory = serde_json::from_str(
            r#"{"id":"m","scope":"user","kind":"how_to","title":"T","content":"c",
                "provenance":{"origin":"user"},"status":"active","createdAt":1,"updatedAt":1}"#,
        )
        .unwrap();

        assert_eq!((stored.uses, stored.owner, stored.topic), (0, None, None));
        assert!(!stored.provenance.automatic);
    }

    #[test]
    fn the_stem_joins_forms_of_a_word_and_keeps_unrelated_words_apart() {
        for (a, b) in [
            ("retry", "retries"),
            ("limit", "limits"),
            ("limit", "limited"),
            ("change", "changed"),
            ("change", "changes"),
            ("change", "changing"),
            ("log", "logging"),
            ("log", "logs"),
            ("setting", "settings"),
            ("cache", "cached"),
            ("cache", "caches"),
        ] {
            assert_eq!(stem(a), stem(b), "{a} and {b}");
        }
        for (a, b) in [
            ("pipe", "pip"),
            ("rate", "rat"),
            ("node", "nod"),
            ("site", "sit"),
            ("case", "cas"),
            ("string", "str"),
            ("status", "statu"),
            ("class", "clas"),
        ] {
            assert_ne!(stem(a), stem(b), "{a} and {b} are different words");
        }
        assert_eq!(stem("install"), "install");
        assert_eq!(stem("process"), "process");
    }

    #[test]
    fn a_pitfall_and_a_note_a_person_wrote_win_a_tie() {
        let mut pitfall = memory("p", MemoryScope::User, None, "windows escape");
        pitfall.kind = MemoryKind::Pitfall;
        let mut decision = pitfall.clone();
        decision.kind = MemoryKind::Decision;
        let mut proposed = pitfall.clone();
        proposed.provenance.origin = MemoryOrigin::Agent;

        assert!(tie_bonus(&pitfall) > tie_bonus(&decision));
        assert!(tie_bonus(&pitfall) > tie_bonus(&proposed));
        assert!(tie_bonus(&pitfall) < 10);
    }

    #[test]
    fn a_lesson_resembles_one_with_the_same_title_words_or_most_of_the_same_words() {
        let mut rejected = memory(
            "r",
            MemoryScope::User,
            None,
            "Change retries in config/app.toml.",
        );
        rejected.title = "Retry limit lives in config/app.toml".to_owned();

        assert!(rejected.resembles("retry limits live in config/app.toml", "anything"));
        assert!(rejected.resembles(
            "Where the retry limit is",
            "Change retries in config/app.toml to change the retry limit."
        ));
        assert!(!rejected.resembles(
            "Retry limit lives in deploy/limits.cfg",
            "MAXTRY in deploy/limits.cfg is the limit; app.toml is not read."
        ));
        assert!(!rejected.resembles("Log level", "LOG_LEVEL in deploy/logging.env"));

        // One title, opposite lessons: not the same one.
        let mut pnpm = memory("p", MemoryScope::User, None, "Use pnpm for installs");
        pnpm.title = "Package manager".to_owned();
        assert!(!pnpm.resembles("Package manager", "Use npm for installs"));
        // A lesson and its negation share every word the matching keeps.
        let mut read = memory(
            "r2",
            MemoryScope::User,
            None,
            "The retries key in config/app.toml is read by the service",
        );
        read.title = "Retries key".to_owned();
        assert!(!read.resembles(
            "Retries key",
            "The retries key in config/app.toml is not read by the service"
        ));
        assert!(!read.resembles(
            "Retries key",
            "The retries key in config/app.toml isn\u{2019}t read by the service"
        ));
        assert!(read.resembles(
            "Retries key",
            "The service reads the retries key in config/app.toml"
        ));
        // Two words in common out of two is too little to judge.
        let short = memory("s", MemoryScope::User, None, "lesson number 1");
        assert!(!short.resembles("x", "lesson number 2"));
    }

    #[test]
    fn being_wrong_weighs_more_than_being_right_and_only_an_agents_note_is_put_away() {
        let mut note = memory("n", MemoryScope::User, None, "x");
        note.provenance.origin = MemoryOrigin::Agent;
        note.helped = 3;
        note.misled = 1;
        assert_eq!(note.trust(), 1);
        assert!(!note.misleads());
        note.helped = 1;
        note.misled = MISLED_TO_REVIEW;
        assert!(note.misleads());
        note.provenance.origin = MemoryOrigin::User;
        assert!(!note.misleads());

        // Between two notes as relevant as each other, the one runs were kept with goes first.
        let mut kept = memory("b", MemoryScope::User, None, "x");
        kept.helped = 2;
        let mut thrown = memory("a", MemoryScope::User, None, "x");
        thrown.misled = 1;
        thrown.uses = 9;
        assert!(rank(&kept, 50) < rank(&thrown, 50));
        assert!(
            rank(&thrown, 60) < rank(&kept, 50),
            "relevance still comes first"
        );
    }
}
