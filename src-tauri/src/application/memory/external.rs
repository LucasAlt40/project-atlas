//! Notes shared with a memory server on this machine that is not Atlas (ADR 0040).
//!
//! **Reading.** Atlas asks the server before a run, with a query it builds itself from the task, so
//! the agent is given no tool and no scope to get wrong. What comes back is text in the same notes
//! section as Atlas's own memories: a hint, never an instruction, marked as one nobody here approved.
//!
//! **Writing.** A note a person approved in Atlas (workspace scope) is published to the server, one
//! page per project of the workspace, so the person's other tools see it; when the note is archived,
//! goes stale or is deleted, the page is retired. A note no person decided (automatic) is never
//! published. Nothing else is written, and a page that came from elsewhere is never touched.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{mpsc, Arc, Mutex};

use serde::{Deserialize, Serialize};

use super::block::MemoryBlock;
use crate::application::config::ConfigRepository;
use crate::application::harness::secrets::redact_secrets;
use crate::application::support::LockExt;
use crate::domain::memory::{
    stem, terms, ExternalMemorySettings, Memory, MemoryKind, MemoryScope, MemoryStatus,
    MAX_CONTENT_CHARS,
};

/// The most search terms one query carries: a longer task adds noise, not recall.
const MAX_TERMS: usize = 12;
/// Where Atlas's own pages live in a project of the server; nothing else is ever written or retired.
const PAGE_DIR: &str = "atlas/";

/// Why the memory server could not be asked or told something. The code is stable and is what the
/// screen words in the person's language; the text is for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalError {
    /// The server's address is not one on this machine.
    Address,
    /// The server's data folder is not set.
    DataDir,
    /// The access token cannot be read or is not in the shape of one.
    Token,
    NotRunning,
    Timeout,
    /// The server turned the token down.
    Refused,
    /// The answer is not what the server says.
    Unreadable,
    TooLarge,
    /// The server answered with another status.
    Server(String),
    /// Atlas only writes and retires its own pages.
    OnlyAtlasPages,
    /// Sharing notes is off.
    SharingOff,
}

impl ExternalError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Address => "address",
            Self::DataDir => "data_dir",
            Self::Token => "token",
            Self::NotRunning => "not_running",
            Self::Timeout => "timeout",
            Self::Refused => "refused",
            Self::Unreadable => "unreadable",
            Self::TooLarge => "too_large",
            Self::Server(_) => "server",
            Self::OnlyAtlasPages => "only_atlas_pages",
            Self::SharingOff => "sharing_off",
        }
    }

    /// What the code leaves out, when there is something (the status the server answered).
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Server(status) => Some(status.clone()),
            _ => None,
        }
    }
}

impl std::fmt::Display for ExternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Address => "the memory server must be on this machine",
            Self::DataDir => "the memory server's data folder is not set",
            Self::Token => "the memory server's access token could not be read",
            Self::NotRunning => "the memory server is not running",
            Self::Timeout => "the memory server did not answer in time",
            Self::Refused => "the memory server refused Atlas's access token",
            Self::Unreadable => "the memory server answered with something Atlas does not read",
            Self::TooLarge => "the memory server's answer was too large",
            Self::Server(status) => return write!(f, "the memory server answered {status}"),
            Self::OnlyAtlasPages => "Atlas only writes and retires its own pages",
            Self::SharingOff => "sharing notes with the memory server is off",
        })
    }
}

/// Where in the server a project's notes are kept.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExternalScope {
    pub workspace: String,
    pub project: String,
}

/// One note the server found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalHit {
    pub id: String,
    pub path: String,
    pub title: String,
    pub snippet: String,
}

/// A page Atlas publishes: one approved note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub path: String,
    pub title: String,
    pub body: String,
    /// What the server calls it: `fact`, `rule`, `decision` or `gotcha`.
    pub kind: &'static str,
    pub tags: Vec<String>,
}

/// What Atlas needs from such a server. The adapter is in `infrastructure/`.
pub trait ExternalMemory: Send + Sync {
    /// The notes of `scope` that match `query` (terms joined by `OR`), best first.
    ///
    /// # Errors
    ///
    /// Says why the server could not answer; the run goes on without its notes.
    fn search(
        &self,
        settings: &ExternalMemorySettings,
        scope: &ExternalScope,
        query: &str,
        limit: u8,
    ) -> Result<Vec<ExternalHit>, ExternalError>;

    /// Writes (or rewrites) a page.
    ///
    /// # Errors
    ///
    /// Says why the server did not take it.
    fn publish(
        &self,
        settings: &ExternalMemorySettings,
        scope: &ExternalScope,
        page: &Page,
    ) -> Result<(), ExternalError>;

    /// Removes a page Atlas published. The server keeps its history; a page that is not there is
    /// not an error.
    ///
    /// # Errors
    ///
    /// Says why the server did not do it.
    fn retire(
        &self,
        settings: &ExternalMemorySettings,
        scope: &ExternalScope,
        path: &str,
    ) -> Result<(), ExternalError>;
}

/// What the server added to one prompt.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ExternalRecall {
    pub blocks: Vec<MemoryBlock>,
    /// The title of each block, by block id, for the manifest.
    pub titles: BTreeMap<String, String>,
    /// Why nothing could be asked of the server, when that is so (it is not an error of the run).
    pub unavailable: Option<ExternalError>,
    /// Notes left out because they looked like a secret.
    pub refused: usize,
    /// Notes left out because they are pages of notes this Atlas keeps, which it already weighs.
    pub duplicates: usize,
    /// Notes left out because their title does not share words with the task.
    pub unrelated: usize,
}

/// A name the server can hold: lowercase letters, digits and `-`, never empty.
pub fn slug(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        // A name with no letters or digits the server can hold (a folder in another alphabet):
        // its own short hash, so two such folders are not one project.
        let mut h: u32 = 2_166_136_261;
        for b in raw.bytes() {
            h = (h ^ u32::from(b)).wrapping_mul(16_777_619);
        }
        format!("p-{h:08x}")
    } else {
        trimmed.to_owned()
    }
}

fn folder_name(project_root: &str) -> &str {
    std::path::Path::new(project_root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
}

/// The scope of a project: the server's workspace from the settings (its own tools use `default`
/// unless told otherwise) and the project folder's name, which is how those tools name it too.
pub fn scope_of(settings: &ExternalMemorySettings, project_root: &str) -> ExternalScope {
    let workspace = settings.workspace.trim();
    ExternalScope {
        workspace: if workspace.is_empty() {
            "default".to_owned()
        } else {
            slug(workspace)
        },
        project: slug(folder_name(project_root)),
    }
}

/// The query for a task: its meaningful words, `OR`ed, or `None` when it has none (nothing is
/// asked, and the prompt stays as it was).
pub fn query_of(task: &str) -> Option<String> {
    let words = query_terms(task);
    (!words.is_empty()).then(|| words.join(" OR "))
}

/// The meaningful words of a task in the order the task says them (what comes first is what the
/// task is about), never alphabetical: cutting an alphabetical list would keep the words that start
/// with "a".
fn query_terms(task: &str) -> Vec<String> {
    let meaningful = terms(task);
    let mut seen = BTreeSet::new();
    task.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| meaningful.contains(w) && seen.insert(w.clone()))
        .take(MAX_TERMS)
        .collect()
}

/// Whether a note's title shares enough words with the task to be about it: two, or the one the
/// task has. The search ranks by every word, so a task that shares only "change" and "service" with
/// a note still finds it; the title is where a note says what it is about.
fn title_matches(title: &str, terms: &[String]) -> bool {
    let need = terms.len().min(2);
    // `stem` is the one the native memory matches on too: the server's search stems as well, and
    // without it this check would turn away the very notes the search found.
    let words: BTreeSet<String> = title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(stem)
        .collect();
    terms.iter().filter(|t| words.contains(&stem(t))).count() >= need
}

/// A server's snippet as one line of plain text: its own highlighting is removed, control and
/// direction-changing characters are dropped (nothing a terminal or a reader could be steered by),
/// and it is cut where a lesson would be. Other text, `<` included, is left as it was.
fn clean(snippet: &str) -> String {
    let without_marks = snippet.replace("<mark>", "").replace("</mark>", "");
    let plain: String = without_marks
        .chars()
        .filter(|c| {
            (!c.is_control() || c.is_whitespace())
                && !matches!(*c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
        })
        .collect();
    let one_line = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.chars().take(MAX_CONTENT_CHARS).collect()
}

fn estimate_tokens(text: &str) -> u32 {
    u32::try_from(text.chars().count().div_ceil(4)).unwrap_or(u32::MAX)
}

/// The page path of one of Atlas's notes.
pub fn page_path(memory_id: &str) -> String {
    format!("{PAGE_DIR}{memory_id}.md")
}

/// The Atlas note a page path names, when it is one of the pages Atlas publishes: `atlas/<id>.md`
/// with a short plain id. Nothing else is a page Atlas may write or retire.
fn memory_id_of(path: &str) -> Option<&str> {
    let id = path.strip_prefix(PAGE_DIR)?.strip_suffix(".md")?;
    let plain = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    plain.then_some(id)
}

/// Whether a page is not a note: the server's own bookkeeping (its lint report, for instance), any
/// path with a segment that starts with `_`; or a raw capture of a session (`sessions/…`), which
/// is a log of prompts and tool counts whose title is the prompt itself. Measured on a replayed
/// session: with no LLM the server writes no lesson, only that log, and it would pass for a note
/// on any task worded like the prompt. Never put into a prompt.
fn is_internal(path: &str) -> bool {
    path.starts_with("sessions/") || path.split('/').any(|segment| segment.starts_with('_'))
}

/// Whether `path` is a page Atlas publishes (see [`memory_id_of`]).
pub fn is_atlas_page(path: &str) -> bool {
    memory_id_of(path).is_some()
}

/// The notes the server has for this task, as blocks, inside the settings' budget. `own` are the ids
/// of every note this Atlas keeps, in any workspace and any state: the page of one of them never
/// comes back. Atlas weighs its own notes, and the server's scope is the project's folder name,
/// which two Atlas workspaces can share, so a page of one must not appear in the prompts of the
/// other. A page of a note this Atlas does not know (another machine's Atlas, say) is news and comes.
/// Never an error: a server that is down, slow or wrong is a run without its notes.
pub fn recall(
    server: &dyn ExternalMemory,
    settings: &ExternalMemorySettings,
    project_root: &str,
    task: &str,
    own: &BTreeSet<String>,
) -> ExternalRecall {
    let mut result = ExternalRecall::default();
    if !settings.enabled || settings.max_hits == 0 {
        return result;
    }
    let Some(query) = query_of(task) else {
        return result;
    };
    let scope = scope_of(settings, project_root);
    // A window wider than what is brought: raw session logs sit at the top of the results (their
    // summaries repeat the task) and the title check sets more aside, and the notes worth bringing
    // must not be buried under them.
    let asked = settings.max_hits.saturating_mul(5).clamp(25, 50);
    let terms = query_terms(task);
    let hits = match server.search(settings, &scope, &query, asked) {
        Ok(hits) => hits,
        Err(why) => {
            result.unavailable = Some(why);
            return result;
        }
    };
    let mut spent = 0u32;
    for hit in hits {
        if is_internal(&hit.path) {
            continue;
        }
        if settings.precise && !title_matches(&hit.title, &terms) {
            result.unrelated += 1;
            continue;
        }
        if result.blocks.len() >= usize::from(settings.max_hits) {
            break;
        }
        if memory_id_of(&hit.path).is_some_and(|id| own.contains(id)) {
            result.duplicates += 1;
            continue;
        }
        let title = clean(&hit.title);
        let line = format!("- [external] {title}: {}", clean(&hit.snippet));
        // A note that looks like a secret is refused whole, as Atlas's own are.
        if redact_secrets(&line).1 > 0 {
            result.refused += 1;
            continue;
        }
        let cost = estimate_tokens(&line);
        if spent.saturating_add(cost) > settings.max_tokens {
            continue;
        }
        spent += cost;
        let id = format!("ext:{}", hit.id);
        result.titles.insert(id.clone(), title);
        result.blocks.push(MemoryBlock {
            id,
            text: line,
            automatic: true,
        });
    }
    result
}

/// Whether the connection works: the server answers a search with Atlas's token.
///
/// # Errors
///
/// Says what is wrong, in words for the person.
pub fn check(
    server: &dyn ExternalMemory,
    settings: &ExternalMemorySettings,
) -> Result<(), ExternalError> {
    let scope = scope_of(settings, "atlas");
    server.search(settings, &scope, "atlas", 1).map(|_| ())
}

// ---------------------------------------------------------------------------------------------
// Writing

/// Whether a note is one Atlas shares: a person decided it, it is about a workspace, and it is in
/// use. What an agent wrote and nobody confirmed (automatic) never leaves Atlas.
pub fn shareable(memory: &Memory) -> bool {
    memory.scope == MemoryScope::Workspace
        && memory.owner.is_some()
        && memory.status == MemoryStatus::Active
        && !memory.provenance.automatic
}

/// The page for a note: the words as written, with a kind the server's own tools know.
pub fn page_of(memory: &Memory) -> Page {
    let (kind, label) = match memory.kind {
        MemoryKind::HowTo => ("fact", "how-to"),
        MemoryKind::Pitfall => ("gotcha", "pitfall"),
        MemoryKind::Preference => ("rule", "preference"),
        MemoryKind::Decision => ("decision", "decision"),
    };
    Page {
        path: page_path(&memory.id),
        title: memory.title.clone(),
        body: memory.content.clone(),
        kind,
        tags: vec!["atlas".to_owned(), label.to_owned()],
    }
}

/// Told by the Memory when a note changes or goes away.
pub trait MemoryMirror: Send + Sync {
    /// The note was written, edited or moved to another status: publish it or retire it.
    fn changed(&self, memory: &Memory);
    /// The note was deleted.
    fn removed(&self, memory: &Memory);
}

/// What the mirror needs to know about the app, read when it is needed: the settings may change
/// while the app runs.
pub trait MirrorContext: Send + Sync {
    fn settings(&self) -> ExternalMemorySettings;
    /// The folders of the projects of a workspace.
    fn project_folders(&self, workspace_id: &str) -> Vec<String>;
}

impl MirrorContext for ConfigRepository {
    fn settings(&self) -> ExternalMemorySettings {
        self.read(|config| config.settings.external_memory.clone())
    }

    fn project_folders(&self, workspace_id: &str) -> Vec<String> {
        self.read(|config| {
            config
                .workspaces
                .iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.projects.iter().map(|p| p.path.clone()).collect())
                .unwrap_or_default()
        })
    }
}

/// A page Atlas has published, as the server holds it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Published {
    pub workspace: String,
    pub project: String,
    pub path: String,
}

impl Published {
    fn of(scope: &ExternalScope, path: &str) -> Self {
        Self {
            workspace: scope.workspace.clone(),
            project: scope.project.clone(),
            path: path.to_owned(),
        }
    }

    fn scope(&self) -> ExternalScope {
        ExternalScope {
            workspace: self.workspace.clone(),
            project: self.project.clone(),
        }
    }
}

/// What Atlas has published, kept between runs of the app so a page whose note was deleted while
/// the server was off, or whose project left the workspace, can still be found and retired.
pub trait PublishedLedger: Send + Sync {
    fn load(&self) -> BTreeSet<Published>;

    /// # Errors
    ///
    /// Says why it could not be kept; the next sync rebuilds what it can.
    fn save(&self, entries: &BTreeSet<Published>) -> Result<(), String>;
}

/// A ledger that forgets, for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryLedger(Mutex<BTreeSet<Published>>);

#[cfg(test)]
impl PublishedLedger for MemoryLedger {
    fn load(&self) -> BTreeSet<Published> {
        self.0.lock_or_recover().clone()
    }

    fn save(&self, entries: &BTreeSet<Published>) -> Result<(), String> {
        self.0.lock_or_recover().clone_from(entries);
        Ok(())
    }
}

/// What a full sync did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SyncReport {
    pub published: usize,
    pub retired: usize,
    pub failed: usize,
}

impl SyncReport {
    fn add(&mut self, other: Self) {
        self.published += other.published;
        self.retired += other.retired;
        self.failed += other.failed;
    }
}

/// The work of the mirror, one piece at a time and in the order it was asked.
enum Job {
    Changed(Memory),
    Removed(Memory),
    /// Answers when everything before it is done (for tests).
    #[cfg(test)]
    Flush(mpsc::Sender<()>),
}

struct Inner {
    server: Arc<dyn ExternalMemory>,
    context: Arc<dyn MirrorContext>,
    ledger: Arc<dyn PublishedLedger>,
    published: Mutex<BTreeSet<Published>>,
    /// Held while the server is being brought up to date, by every queued job and by a whole sync:
    /// a sync takes its snapshot of the notes only after it holds this, so a note approved while it
    /// runs is neither retired as an orphan nor published from a stale list.
    work: Mutex<()>,
    /// The newest version of each note this mirror has dealt with (`updated_at`), so a job that was
    /// queued before a later edit, or before a sync, cannot put an older text back.
    seen: Mutex<BTreeMap<String, u64>>,
    /// Notes deleted: a change that was queued before the deletion must not bring the page back.
    gone: Mutex<BTreeSet<String>>,
}

/// Keeps the server's pages in step with the notes a person approved in Atlas. Silent on failure:
/// a server that is off never blocks a person from saving a note, and the next sync catches up.
/// In the app the work happens on its own thread, in order, so saving a note or starting a run
/// never waits for the network; in tests it happens at once.
pub struct ExternalMirror {
    inner: Arc<Inner>,
    queue: Option<mpsc::Sender<Job>>,
}

impl Inner {
    fn active(&self) -> Option<ExternalMemorySettings> {
        let settings = self.context.settings();
        (settings.enabled && settings.share).then_some(settings)
    }

    /// The scopes a note of a workspace is published to: one per project, without repeats.
    fn targets(
        &self,
        settings: &ExternalMemorySettings,
        memory: &Memory,
    ) -> BTreeSet<ExternalScope> {
        let Some(owner) = memory.owner.as_deref() else {
            return BTreeSet::new();
        };
        self.context
            .project_folders(owner)
            .iter()
            .map(|folder| scope_of(settings, folder))
            .collect()
    }

    fn remember(&self, change: impl FnOnce(&mut BTreeSet<Published>)) {
        let mut entries = self.published.lock_or_recover();
        change(&mut entries);
        // Best effort: what could not be kept is rebuilt by the next sync.
        let _ = self.ledger.save(&entries);
    }

    fn publish_to(
        &self,
        settings: &ExternalMemorySettings,
        scope: &ExternalScope,
        page: &Page,
    ) -> SyncReport {
        let mut report = SyncReport::default();
        match self.server.publish(settings, scope, page) {
            Ok(()) => {
                self.remember(|e| {
                    e.insert(Published::of(scope, &page.path));
                });
                report.published += 1;
            }
            Err(why) => {
                report.failed += 1;
                eprintln!("external memory not updated ({why}); a sync will catch up");
            }
        }
        report
    }

    fn retire_from(
        &self,
        settings: &ExternalMemorySettings,
        scope: &ExternalScope,
        path: &str,
    ) -> SyncReport {
        let mut report = SyncReport::default();
        match self.server.retire(settings, scope, path) {
            Ok(()) => {
                self.remember(|e| {
                    e.remove(&Published::of(scope, path));
                });
                report.retired += 1;
            }
            Err(why) => {
                report.failed += 1;
                eprintln!("external memory not updated ({why}); a sync will catch up");
            }
        }
        report
    }

    /// Where a page of this path is known to be on the server.
    fn known_scopes(&self, path: &str) -> BTreeSet<ExternalScope> {
        self.published
            .lock_or_recover()
            .iter()
            .filter(|p| p.path == path)
            .map(Published::scope)
            .collect()
    }

    /// Brings the server to what `memory` is now: its page written when shareable; retired, from
    /// everywhere it is known to be (and from its projects, when a person owned the note), when
    /// not and `retire_when_not_shareable`.
    fn bring_up_to_date(
        &self,
        settings: &ExternalMemorySettings,
        memory: &Memory,
        retire_when_not_shareable: bool,
    ) -> SyncReport {
        let mut report = SyncReport::default();
        let page = page_of(memory);
        if shareable(memory) {
            // A note that looks like a secret never leaves; the Memory refuses to keep one, so this
            // is a second look at what a hand-edited file could hold.
            if redact_secrets(&format!("{} {}", page.title, page.body)).1 > 0 {
                report.failed += 1;
                return report;
            }
            let targets = self.targets(settings, memory);
            for scope in &targets {
                report.add(self.publish_to(settings, scope, &page));
            }
            // A project the workspace no longer has keeps no page of this note.
            for scope in self.known_scopes(&page.path).difference(&targets) {
                report.add(self.retire_from(settings, scope, &page.path));
            }
        } else if retire_when_not_shareable {
            let mut scopes = self.known_scopes(&page.path);
            // An automatic note was never published; for any other, the projects are tried too,
            // in case the ledger was lost.
            if !memory.provenance.automatic {
                scopes.extend(self.targets(settings, memory));
            }
            for scope in &scopes {
                report.add(self.retire_from(settings, scope, &page.path));
            }
        }
        report
    }

    /// Whether this version of the note is old news: it was deleted, or a newer one was handled.
    fn is_stale(&self, memory: &Memory) -> bool {
        self.gone.lock_or_recover().contains(&memory.id)
            || self
                .seen
                .lock_or_recover()
                .get(&memory.id)
                .is_some_and(|newest| *newest > memory.updated_at)
    }

    fn note_seen(&self, memory: &Memory) {
        let mut seen = self.seen.lock_or_recover();
        let newest = seen.entry(memory.id.clone()).or_insert(0);
        *newest = (*newest).max(memory.updated_at);
    }

    fn changed(&self, memory: &Memory) {
        if memory.scope != MemoryScope::Workspace {
            return;
        }
        let _work = self.work.lock_or_recover();
        if self.is_stale(memory) {
            return;
        }
        self.note_seen(memory);
        if let Some(settings) = self.active() {
            // Only a note that has gone out of use can have a page to retire; a candidate or an
            // automatic note was never published.
            let retire = matches!(memory.status, MemoryStatus::Archived | MemoryStatus::Stale);
            self.bring_up_to_date(&settings, memory, retire);
        }
    }

    fn removed(&self, memory: &Memory) {
        if memory.scope != MemoryScope::Workspace {
            return;
        }
        let _work = self.work.lock_or_recover();
        self.gone.lock_or_recover().insert(memory.id.clone());
        if let Some(settings) = self.active() {
            let mut gone = memory.clone();
            gone.status = MemoryStatus::Archived;
            self.bring_up_to_date(&settings, &gone, true);
        }
    }

    /// `notes` is asked for only once the work lock is held, so it is the list as it is when the
    /// sync starts, with no queued job halfway.
    fn sync_all(&self, notes: impl FnOnce() -> Vec<Memory>) -> Result<SyncReport, ExternalError> {
        let settings = self.active().ok_or(ExternalError::SharingOff)?;
        let _work = self.work.lock_or_recover();
        let memories = notes();
        let mut total = SyncReport::default();
        let mut wanted = BTreeSet::new();
        for memory in memories
            .iter()
            .filter(|m| m.scope == MemoryScope::Workspace)
        {
            self.note_seen(memory);
            if shareable(memory) {
                for scope in self.targets(&settings, memory) {
                    wanted.insert(Published::of(&scope, &page_path(&memory.id)));
                }
            }
            let retire = matches!(memory.status, MemoryStatus::Archived | MemoryStatus::Stale);
            total.add(self.bring_up_to_date(&settings, memory, retire));
        }
        // What Atlas published and no note asks for any more: a deleted note, a project that left,
        // a server workspace that was changed.
        let orphans: Vec<Published> = self
            .published
            .lock_or_recover()
            .iter()
            .filter(|p| !wanted.contains(p))
            .cloned()
            .collect();
        for orphan in orphans {
            total.add(self.retire_from(&settings, &orphan.scope(), &orphan.path));
        }
        Ok(total)
    }
}

impl ExternalMirror {
    /// A mirror that does its work as soon as it is asked (tests, and anything that wants the
    /// result at once).
    pub fn new(
        server: Arc<dyn ExternalMemory>,
        context: Arc<dyn MirrorContext>,
        ledger: Arc<dyn PublishedLedger>,
    ) -> Self {
        let published = Mutex::new(ledger.load());
        Self {
            inner: Arc::new(Inner {
                server,
                context,
                ledger,
                published,
                work: Mutex::new(()),
                seen: Mutex::new(BTreeMap::new()),
                gone: Mutex::new(BTreeSet::new()),
            }),
            queue: None,
        }
    }

    /// A mirror that does its work on a thread of its own, in order. The thread ends with the
    /// mirror.
    pub fn queued(
        server: Arc<dyn ExternalMemory>,
        context: Arc<dyn MirrorContext>,
        ledger: Arc<dyn PublishedLedger>,
    ) -> Self {
        let mut mirror = Self::new(server, context, ledger);
        let (sender, jobs) = mpsc::channel::<Job>();
        let inner = mirror.inner.clone();
        std::thread::Builder::new()
            .name("atlas-external-memory".to_owned())
            .spawn(move || {
                for job in jobs {
                    // A panic in one job (a bug in an adapter, say) must not end the worker: after
                    // that every later change would be dropped without a word.
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match job {
                        Job::Changed(memory) => inner.changed(&memory),
                        Job::Removed(memory) => inner.removed(&memory),
                        #[cfg(test)]
                        Job::Flush(done) => {
                            let _ = done.send(());
                        }
                    }));
                    if outcome.is_err() {
                        eprintln!("external memory: a job failed unexpectedly; the next sync will catch up");
                    }
                }
            })
            .expect("a thread for the external memory mirror");
        mirror.queue = Some(sender);
        mirror
    }

    /// Waits until everything asked so far is done.
    #[cfg(test)]
    pub fn flush(&self) {
        if let Some(queue) = &self.queue {
            let (done, wait) = mpsc::channel();
            if queue.send(Job::Flush(done)).is_ok() {
                let _ = wait.recv();
            }
        }
    }

    /// Publishes every shareable note, retires the ones out of use and the pages no note asks for
    /// any more, for notes approved before sharing was switched on or while the server was off.
    ///
    /// # Errors
    ///
    /// When sharing is off.
    pub fn sync_all(
        &self,
        notes: impl FnOnce() -> Vec<Memory>,
    ) -> Result<SyncReport, ExternalError> {
        self.inner.sync_all(notes)
    }
}

impl MemoryMirror for ExternalMirror {
    fn changed(&self, memory: &Memory) {
        match &self.queue {
            Some(queue) => {
                let _ = queue.send(Job::Changed(memory.clone()));
            }
            None => self.inner.changed(memory),
        }
    }

    fn removed(&self, memory: &Memory) {
        match &self.queue {
            Some(queue) => {
                let _ = queue.send(Job::Removed(memory.clone()));
            }
            None => self.inner.removed(memory),
        }
    }
}

/// What the settings screen asks of the memory server: whether it answers, and to bring it up to
/// date with the notes a person approved.
pub struct ExternalMemoryService {
    server: Arc<dyn ExternalMemory>,
    context: Arc<dyn MirrorContext>,
    mirror: Arc<ExternalMirror>,
    memory: Arc<super::MemoryService>,
}

impl ExternalMemoryService {
    pub fn new(
        server: Arc<dyn ExternalMemory>,
        context: Arc<dyn MirrorContext>,
        mirror: Arc<ExternalMirror>,
        memory: Arc<super::MemoryService>,
    ) -> Self {
        Self {
            server,
            context,
            mirror,
            memory,
        }
    }

    /// Whether the server answers with Atlas's token, using the saved settings.
    ///
    /// # Errors
    ///
    /// Says what is wrong, in words for the person.
    pub fn check(&self) -> Result<(), ExternalError> {
        let settings = self.context.settings();
        settings
            .loopback_authority()
            .ok_or(ExternalError::Address)?;
        check(self.server.as_ref(), &settings)
    }

    /// Publishes the approved notes and retires the ones out of use.
    ///
    /// # Errors
    ///
    /// When sharing is off.
    pub fn sync(&self) -> Result<SyncReport, ExternalError> {
        self.mirror.sync_all(|| self.memory.list())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::memory::tests::memory;
    use crate::domain::memory::MemoryProvenance;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Fake {
        hits: Vec<ExternalHit>,
        fail: Option<ExternalError>,
        searched: Mutex<Vec<(ExternalScope, String, u8)>>,
        published: Mutex<Vec<(ExternalScope, Page)>>,
        retired: Mutex<Vec<(ExternalScope, String)>>,
        log: Mutex<Vec<String>>,
    }

    impl ExternalMemory for Fake {
        fn search(
            &self,
            _: &ExternalMemorySettings,
            scope: &ExternalScope,
            query: &str,
            limit: u8,
        ) -> Result<Vec<ExternalHit>, ExternalError> {
            self.searched
                .lock()
                .unwrap()
                .push((scope.clone(), query.to_owned(), limit));
            self.fail.clone().map_or_else(|| Ok(self.hits.clone()), Err)
        }

        fn publish(
            &self,
            _: &ExternalMemorySettings,
            scope: &ExternalScope,
            page: &Page,
        ) -> Result<(), ExternalError> {
            if let Some(why) = &self.fail {
                return Err(why.clone());
            }
            self.log
                .lock()
                .unwrap()
                .push(format!("publish {}", page.path));
            self.published
                .lock()
                .unwrap()
                .push((scope.clone(), page.clone()));
            Ok(())
        }

        fn retire(
            &self,
            _: &ExternalMemorySettings,
            scope: &ExternalScope,
            path: &str,
        ) -> Result<(), ExternalError> {
            if let Some(why) = &self.fail {
                return Err(why.clone());
            }
            self.log.lock().unwrap().push(format!("retire {path}"));
            self.retired
                .lock()
                .unwrap()
                .push((scope.clone(), path.to_owned()));
            Ok(())
        }
    }

    #[derive(Clone)]
    struct Context {
        settings: ExternalMemorySettings,
        folders: Vec<String>,
    }

    impl MirrorContext for Context {
        fn settings(&self) -> ExternalMemorySettings {
            self.settings.clone()
        }

        fn project_folders(&self, workspace_id: &str) -> Vec<String> {
            if workspace_id == "ws" {
                self.folders.clone()
            } else {
                Vec::new()
            }
        }
    }

    fn on() -> ExternalMemorySettings {
        ExternalMemorySettings {
            enabled: true,
            ..ExternalMemorySettings::default()
        }
    }

    fn sharing() -> ExternalMemorySettings {
        ExternalMemorySettings {
            share: true,
            ..on()
        }
    }

    fn none() -> BTreeSet<String> {
        BTreeSet::new()
    }

    fn hit(id: &str, snippet: &str) -> ExternalHit {
        ExternalHit {
            id: id.to_owned(),
            path: format!("notes/{id}.md"),
            title: "Retry limit".to_owned(),
            snippet: snippet.to_owned(),
        }
    }

    // ----- reading

    #[test]
    fn off_asks_nothing_and_adds_nothing() {
        let server = Fake {
            hits: vec![hit("1", "x")],
            ..Fake::default()
        };

        let got = recall(
            &server,
            &ExternalMemorySettings::default(),
            "/p/app",
            "fix retries",
            &none(),
        );

        assert_eq!(got, ExternalRecall::default());
        assert!(server.searched.lock().unwrap().is_empty());
    }

    #[test]
    fn a_task_with_no_meaningful_word_asks_nothing() {
        let server = Fake {
            hits: vec![hit("1", "x")],
            ..Fake::default()
        };

        let got = recall(&server, &on(), "/p/app", "do it as the of", &none());

        assert_eq!(got.blocks, []);
        assert!(server.searched.lock().unwrap().is_empty());
    }

    #[test]
    fn the_query_and_scope_are_built_by_atlas_from_the_servers_workspace_and_the_folder() {
        let server = Fake::default();
        let mut settings = on();
        settings.workspace = "My Work".to_owned();

        recall(
            &server,
            &settings,
            "/home/me/My App/",
            "Change the retry limit",
            &none(),
        );
        recall(
            &server,
            &on(),
            "/home/me/billing",
            "Change the retry limit",
            &none(),
        );

        let asked = server.searched.lock().unwrap();
        assert_eq!(asked[0].0.workspace, "my-work");
        assert_eq!(asked[0].0.project, "my-app");
        assert_eq!(asked[1].0.workspace, "default");
        assert_eq!(asked[1].0.project, "billing");
        assert!(asked[0].1.contains(" OR "));
        assert_eq!(asked[0].2, 25);
    }

    #[test]
    fn a_hit_becomes_an_automatic_block_without_the_servers_markup() {
        let server = Fake {
            hits: vec![hit(
                "01a1",
                "The MAXTRY <mark>retry</mark>\n limit is in deploy/limits.cfg",
            )],
            ..Fake::default()
        };

        let got = recall(&server, &on(), "/p/app", "retry limit", &none());

        assert_eq!(got.blocks.len(), 1);
        assert_eq!(got.blocks[0].id, "ext:01a1");
        assert!(got.blocks[0].automatic);
        assert_eq!(
            got.blocks[0].text,
            "- [external] Retry limit: The MAXTRY retry limit is in deploy/limits.cfg"
        );
        assert_eq!(got.blocks[0].provenance(), "memory:ext:01a1:automatic");
        assert_eq!(got.titles["ext:01a1"], "Retry limit");
    }

    #[test]
    fn a_note_that_looks_like_a_secret_is_refused_whole() {
        let server = Fake {
            hits: vec![
                hit(
                    "1",
                    "use API_KEY=sk-live-abcdefghijklmnopqrstuvwxyz123456 for it",
                ),
                hit("2", "limit is in deploy/limits.cfg"),
            ],
            ..Fake::default()
        };

        let got = recall(&server, &on(), "/p/app", "retry limit", &none());

        assert_eq!(got.refused, 1);
        assert_eq!(got.blocks.len(), 1);
        assert_eq!(got.blocks[0].id, "ext:2");
    }

    #[test]
    fn the_budget_is_a_ceiling() {
        let long = "word ".repeat(400);
        let server = Fake {
            hits: vec![hit("1", &long), hit("2", "short note about retry")],
            ..Fake::default()
        };
        let settings = ExternalMemorySettings {
            max_tokens: 40,
            ..on()
        };

        let got = recall(&server, &settings, "/p/app", "retry limit", &none());

        // the first (480 chars, ~120 tokens) does not fit; the short one does
        assert_eq!(got.blocks.len(), 1);
        assert_eq!(got.blocks[0].id, "ext:2");
    }

    #[test]
    fn a_server_that_fails_is_a_run_without_notes() {
        let server = Fake {
            fail: Some(ExternalError::NotRunning),
            ..Fake::default()
        };

        let got = recall(&server, &on(), "/p/app", "retry limit", &none());

        assert_eq!(got.blocks, []);
        assert_eq!(got.unavailable, Some(ExternalError::NotRunning));
    }

    #[test]
    fn the_page_of_any_note_this_atlas_keeps_is_left_out_whichever_workspace_it_is_in() {
        // two Atlas workspaces can have a project folder of the same name, so they share a scope on
        // the server: the page of one must not appear in the prompt of the other
        let mut mine = hit("1", "limit is in deploy/limits.cfg");
        mine.path = page_path("memory-1");
        let mut other_workspace = hit("2", "limit is in deploy/limits.cfg");
        other_workspace.path = page_path("memory-of-another-workspace");
        let mut other_machine = hit("3", "limit is in deploy/limits.cfg");
        other_machine.path = page_path("memory-from-another-atlas");
        let mut written_by_a_person = hit("4", "limit is in deploy/limits.cfg");
        written_by_a_person.path = "notes/limits.md".to_owned();
        let server = Fake {
            hits: vec![mine, other_workspace, other_machine, written_by_a_person],
            ..Fake::default()
        };
        // this Atlas keeps both notes, in two workspaces, in whatever state
        let own = BTreeSet::from([
            "memory-1".to_owned(),
            "memory-of-another-workspace".to_owned(),
        ]);

        let got = recall(&server, &on(), "/p/app", "retry limit", &own);

        assert_eq!(got.duplicates, 2);
        let ids: Vec<&str> = got.blocks.iter().map(|b| b.id.as_str()).collect();
        // the page of a note this Atlas does not know (another machine's) is news; so is a person's page
        assert_eq!(ids, ["ext:3", "ext:4"]);
    }

    #[test]
    fn a_folder_with_no_name_the_server_can_hold_gets_a_name_of_its_own() {
        assert_ne!(slug("日本語"), slug("한국어"));
        assert!(slug("日本語").starts_with("p-"));
        assert_eq!(slug("日本語"), slug("日本語"));
        assert_eq!(slug("  "), slug("  "));
    }

    #[test]
    fn a_slug_is_never_empty() {
        assert_ne!(slug("  "), "");
        assert_eq!(slug("Meu App_2"), "meu-app-2");
    }

    #[test]
    fn the_connection_check_reports_why_in_the_servers_own_words() {
        let down = Fake {
            fail: Some(ExternalError::NotRunning),
            ..Fake::default()
        };
        assert_eq!(check(&down, &on()).unwrap_err(), ExternalError::NotRunning);
        assert!(check(&Fake::default(), &on()).is_ok());
    }

    // ----- writing

    fn approved(id: &str) -> Memory {
        let mut note = memory(
            id,
            MemoryScope::Workspace,
            Some("ws"),
            "Never use floats for totals",
        );
        note.status = MemoryStatus::Active;
        note.kind = MemoryKind::Pitfall;
        note.provenance = MemoryProvenance {
            automatic: false,
            ..note.provenance
        };
        note
    }

    fn mirror(server: &Arc<Fake>, settings: ExternalMemorySettings) -> ExternalMirror {
        mirror_over(
            server,
            settings,
            &["/work/billing", "/work/web"],
            &Arc::new(MemoryLedger::default()),
        )
    }

    fn mirror_over(
        server: &Arc<Fake>,
        settings: ExternalMemorySettings,
        folders: &[&str],
        ledger: &Arc<MemoryLedger>,
    ) -> ExternalMirror {
        ExternalMirror::new(
            server.clone(),
            Arc::new(Context {
                settings,
                folders: folders.iter().map(|f| (*f).to_owned()).collect(),
            }),
            ledger.clone(),
        )
    }

    #[test]
    fn an_approved_note_is_published_to_every_project_of_its_workspace() {
        let server = Arc::new(Fake::default());

        mirror(&server, sharing()).changed(&approved("m1"));

        let published = server.published.lock().unwrap();
        assert_eq!(published.len(), 2);
        let projects: Vec<&str> = published.iter().map(|(s, _)| s.project.as_str()).collect();
        assert_eq!(projects, ["billing", "web"]);
        assert!(published.iter().all(|(s, _)| s.workspace == "default"));
        let page = &published[0].1;
        assert_eq!(page.path, "atlas/m1.md");
        assert_eq!(page.kind, "gotcha");
        assert_eq!(page.body, "Never use floats for totals");
        assert_eq!(page.tags, ["atlas", "pitfall"]);
    }

    #[test]
    fn nothing_is_written_unless_sharing_is_on() {
        let server = Arc::new(Fake::default());

        mirror(&server, on()).changed(&approved("m1"));
        mirror(
            &server,
            ExternalMemorySettings {
                share: true,
                ..ExternalMemorySettings::default()
            },
        )
        .changed(&approved("m1"));

        assert!(server.published.lock().unwrap().is_empty());
        assert!(server.retired.lock().unwrap().is_empty());
    }

    #[test]
    fn what_no_person_decided_is_never_published_and_has_no_page_to_retire() {
        let server = Arc::new(Fake::default());
        let mut automatic = approved("a");
        automatic.provenance.automatic = true;
        let mut candidate = approved("c");
        candidate.status = MemoryStatus::Candidate;
        let mut user_scope = approved("u");
        user_scope.scope = MemoryScope::User;
        user_scope.owner = None;
        let mut agent_scope = approved("g");
        agent_scope.scope = MemoryScope::Agent;

        for note in [&automatic, &candidate, &user_scope, &agent_scope] {
            mirror(&server, sharing()).changed(note);
        }

        assert!(server.published.lock().unwrap().is_empty());
        assert!(server.retired.lock().unwrap().is_empty());
    }

    #[test]
    fn a_note_that_goes_out_of_use_or_is_deleted_has_its_page_retired() {
        let server = Arc::new(Fake::default());
        let mut archived = approved("m1");
        archived.status = MemoryStatus::Archived;
        let mut stale = approved("m2");
        stale.status = MemoryStatus::Stale;

        mirror(&server, sharing()).changed(&archived);
        mirror(&server, sharing()).changed(&stale);
        mirror(&server, sharing()).removed(&approved("m3"));

        let retired = server.retired.lock().unwrap();
        let paths: Vec<&str> = retired.iter().map(|(_, p)| p.as_str()).collect();
        assert_eq!(
            paths,
            [
                "atlas/m1.md",
                "atlas/m1.md",
                "atlas/m2.md",
                "atlas/m2.md",
                "atlas/m3.md",
                "atlas/m3.md"
            ]
        );
        assert!(server.published.lock().unwrap().is_empty());
    }

    #[test]
    fn a_note_that_looks_like_a_secret_is_never_published() {
        let server = Arc::new(Fake::default());
        let mut leaked = approved("m1");
        leaked.content =
            "login with sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned();

        mirror(&server, sharing()).changed(&leaked);

        assert!(server.published.lock().unwrap().is_empty());
    }

    #[test]
    fn a_server_that_is_off_never_blocks_saving_and_a_sync_catches_up() {
        let down = Arc::new(Fake {
            fail: Some(ExternalError::NotRunning),
            ..Fake::default()
        });
        mirror(&down, sharing()).changed(&approved("m1"));

        let up = Arc::new(Fake::default());
        let mut archived = approved("m2");
        archived.status = MemoryStatus::Archived;
        let mut candidate = approved("m3");
        candidate.status = MemoryStatus::Candidate;
        let report = mirror(&up, sharing())
            .sync_all(|| vec![approved("m1"), archived, candidate])
            .unwrap();

        assert_eq!(
            report,
            SyncReport {
                published: 2,
                retired: 2,
                failed: 0
            }
        );
        assert_eq!(up.published.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_sync_with_sharing_off_says_so() {
        let server = Arc::new(Fake::default());

        assert!(mirror(&server, on())
            .sync_all(|| vec![approved("m1")])
            .is_err());
    }

    // ----- what the review found

    fn down() -> Arc<Fake> {
        Arc::new(Fake {
            fail: Some(ExternalError::NotRunning),
            ..Fake::default()
        })
    }

    #[test]
    fn a_page_whose_note_was_deleted_while_the_server_was_off_is_found_and_retired_later() {
        let ledger = Arc::new(MemoryLedger::default());
        let up = Arc::new(Fake::default());
        mirror_over(&up, sharing(), &["/work/billing", "/work/web"], &ledger)
            .changed(&approved("m1"));
        assert_eq!(ledger.load().len(), 2);

        // Deleted while the server is off: the retirement fails, and the ledger still knows.
        mirror_over(&down(), sharing(), &["/work/billing", "/work/web"], &ledger)
            .removed(&approved("m1"));
        assert_eq!(ledger.load().len(), 2);

        // Next sync (the app start does one) finds the page nobody asks for any more.
        let later = Arc::new(Fake::default());
        let report = mirror_over(&later, sharing(), &["/work/billing", "/work/web"], &ledger)
            .sync_all(Vec::new)
            .unwrap();

        assert_eq!(report.retired, 2);
        assert!(ledger.load().is_empty());
        let retired = later.retired.lock().unwrap();
        assert!(retired.iter().all(|(_, path)| path == "atlas/m1.md"));
    }

    #[test]
    fn a_project_that_leaves_the_workspace_loses_the_page_the_next_time_the_note_changes() {
        let ledger = Arc::new(MemoryLedger::default());
        let server = Arc::new(Fake::default());
        mirror_over(&server, sharing(), &["/work/billing", "/work/web"], &ledger)
            .changed(&approved("m1"));

        // `web` is no longer a project of the workspace.
        mirror_over(&server, sharing(), &["/work/billing"], &ledger).changed(&approved("m1"));

        let retired = server.retired.lock().unwrap();
        assert_eq!(retired.len(), 1);
        assert_eq!(retired[0].0.project, "web");
        assert_eq!(ledger.load().len(), 1);
    }

    #[test]
    fn an_automatic_note_that_expires_has_no_pages_to_retire() {
        let server = Arc::new(Fake::default());
        let mut expired = approved("m1");
        expired.provenance.automatic = true;
        expired.status = MemoryStatus::Archived;

        mirror(&server, sharing()).changed(&expired);

        assert!(server.log.lock().unwrap().is_empty());
    }

    #[test]
    fn the_queued_mirror_keeps_the_order_and_never_makes_the_caller_wait() {
        let server = Arc::new(Fake::default());
        let mirror = ExternalMirror::queued(
            server.clone(),
            Arc::new(Context {
                settings: sharing(),
                folders: vec!["/work/billing".to_owned()],
            }),
            Arc::new(MemoryLedger::default()),
        );
        let mut archived = approved("m1");
        archived.status = MemoryStatus::Archived;

        // Archived, then reactivated: the server must end with the page.
        mirror.changed(&approved("m1"));
        mirror.changed(&archived);
        mirror.changed(&approved("m1"));
        mirror.flush();

        let log = server.log.lock().unwrap();
        assert_eq!(
            *log,
            [
                "publish atlas/m1.md",
                "retire atlas/m1.md",
                "publish atlas/m1.md"
            ]
        );
    }

    #[test]
    fn the_query_keeps_the_words_the_task_opens_with_not_the_alphabetical_first_ones() {
        let task = "retry webhook alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima";

        let query = query_of(task).unwrap();

        assert!(query.starts_with("retry OR webhook OR alpha"));
        assert_eq!(query.matches(" OR ").count(), MAX_TERMS - 1);
        assert!(!query.contains("lima"));
    }

    #[test]
    fn a_snippet_loses_control_and_direction_characters_but_not_its_text() {
        let dirty = "a < b <b>bold</b> \u{1b}[31mred\u{202e}x\u{7}";

        let cleaned = clean(dirty);

        assert_eq!(cleaned, "a < b <b>bold</b> [31mredx");
    }

    #[test]
    fn only_atlass_own_short_plain_pages_are_pages_atlas_may_touch() {
        assert!(is_atlas_page("atlas/memory-1a_b.md"));
        for path in [
            "notes/x.md",
            "atlas/../notes/x.md",
            "atlas/a b.md",
            "atlas/.md",
            "atlas/x.txt",
            "atlas/sub/x.md",
            &format!("atlas/{}.md", "a".repeat(65)),
        ] {
            assert!(!is_atlas_page(path), "{path}");
        }
    }

    #[test]
    fn the_servers_own_pages_are_never_notes() {
        let mut report = hit("1", "lint report: 3 stale pages about the retry limit");
        report.path = "_lint/report.md".to_owned();
        let mut nested = hit("2", "internal retry limit page");
        nested.path = "notes/_drafts/x.md".to_owned();
        let server = Fake {
            hits: vec![
                report,
                nested,
                hit("3", "the retry limit is in deploy/limits.cfg"),
            ],
            ..Fake::default()
        };

        let got = recall(&server, &on(), "/p/app", "retry limit", &none());

        assert_eq!(got.blocks.len(), 1);
        assert_eq!(got.blocks[0].id, "ext:3");
    }

    // ----- precision

    fn titled(id: &str, title: &str) -> ExternalHit {
        ExternalHit {
            id: id.to_owned(),
            path: format!("notes/{id}.md"),
            title: title.to_owned(),
            snippet: "the value lives in deploy/limits.cfg".to_owned(),
        }
    }

    #[test]
    fn a_note_whose_title_does_not_echo_the_task_is_not_brought_unless_precision_is_off() {
        let server = Fake {
            hits: vec![
                titled("a", "Retry limit lives in deploy/limits.cfg"),
                titled("b", "About rate limit"),
                titled("c", "Service restart checklist"),
            ],
            ..Fake::default()
        };
        let task = "Change the retry limit of this service from 3 to 5";

        let precise = recall(&server, &on(), "/p/app", task, &none());
        let loose = recall(
            &server,
            &ExternalMemorySettings {
                precise: false,
                ..on()
            },
            "/p/app",
            task,
            &none(),
        );

        let ids = |r: &ExternalRecall| r.blocks.iter().map(|b| b.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&precise), ["ext:a"]);
        assert_eq!(precise.unrelated, 2);
        assert_eq!(ids(&loose), ["ext:a", "ext:b", "ext:c"]);
    }

    #[test]
    fn a_task_with_one_meaningful_word_needs_that_one_word_in_the_title() {
        let server = Fake {
            hits: vec![titled("a", "Login is flaky"), titled("b", "Deploy order")],
            ..Fake::default()
        };

        let got = recall(&server, &on(), "/p/app", "login", &none());

        assert_eq!(got.blocks.len(), 1);
        assert_eq!(got.blocks[0].id, "ext:a");
    }

    #[test]
    fn the_search_is_asked_for_a_wider_window_than_what_is_brought() {
        let server = Fake::default();

        recall(&server, &on(), "/p/app", "retry limit", &none());
        recall(
            &server,
            &ExternalMemorySettings {
                precise: false,
                ..on()
            },
            "/p/app",
            "retry limit",
            &none(),
        );

        let asked = server.searched.lock().unwrap();
        assert_eq!(asked[0].2, 25);
        assert_eq!(asked[1].2, 25);
    }

    #[test]
    fn a_raw_session_log_is_not_a_note_even_when_its_title_is_the_prompt() {
        let mut log = hit(
            "1",
            "Session metadata - prompts: change the retry limit - 7 observations",
        );
        log.path = "sessions/2b0d277d-a237.md".to_owned();
        log.title = "Change the retry limit of this service from 3 to 5".to_owned();
        let server = Fake {
            hits: vec![log, hit("2", "the retry limit is in deploy/limits.cfg")],
            ..Fake::default()
        };

        let got = recall(&server, &on(), "/p/app", "Change the retry limit", &none());

        assert_eq!(got.blocks.len(), 1);
        assert_eq!(got.blocks[0].id, "ext:2");
    }

    #[test]
    fn a_title_meets_the_task_across_plurals_and_endings() {
        let task =
            query_terms("Change the retry limit of this service, and tell me what you changed");
        // the title the consolidation of a real correction wrote
        assert!(title_matches(
            "MAXTRY in deploy/limits.cfg is the real source; config/app.toml retries key is not read",
            &task
        ));
        assert!(!title_matches(
            "config/app.toml retries key is not used",
            &task
        ));
        assert_eq!(stem("retries"), stem("retry"));
        assert_eq!(stem("limits"), stem("limit"));
        assert_eq!(stem("changed"), stem("change"));
        assert_eq!(stem("class"), "class");
    }

    // ----- ordering and concurrency (second review)

    #[test]
    fn a_change_that_was_queued_before_a_newer_one_cannot_put_the_old_text_back() {
        let server = Arc::new(Fake::default());
        let mirror = mirror(&server, sharing());
        let mut newer = approved("m1");
        newer.updated_at = 20;
        newer.content = "new text".to_owned();
        let mut older = approved("m1");
        older.updated_at = 10;
        older.content = "old text".to_owned();

        mirror.changed(&newer);
        mirror.changed(&older);

        let published = server.published.lock().unwrap();
        assert!(published.iter().all(|(_, page)| page.body == "new text"));
        assert_eq!(
            published.len(),
            2,
            "one per project, and the old text never"
        );
    }

    #[test]
    fn a_change_queued_before_a_deletion_cannot_bring_the_page_back() {
        let server = Arc::new(Fake::default());
        let mirror = mirror(&server, sharing());

        mirror.removed(&approved("m1"));
        mirror.changed(&approved("m1"));

        assert!(server.published.lock().unwrap().is_empty());
    }

    /// A server whose first publication holds until the gate is opened.
    struct Gated {
        open: Mutex<bool>,
        opened: std::sync::Condvar,
        inside: std::sync::atomic::AtomicBool,
        published: Mutex<Vec<String>>,
        retired: Mutex<Vec<String>>,
    }

    impl ExternalMemory for Gated {
        fn search(
            &self,
            _: &ExternalMemorySettings,
            _: &ExternalScope,
            _: &str,
            _: u8,
        ) -> Result<Vec<ExternalHit>, ExternalError> {
            Ok(Vec::new())
        }

        fn publish(
            &self,
            _: &ExternalMemorySettings,
            _: &ExternalScope,
            page: &Page,
        ) -> Result<(), ExternalError> {
            self.inside.store(true, std::sync::atomic::Ordering::SeqCst);
            let mut open = self.open.lock().unwrap();
            while !*open {
                open = self.opened.wait(open).unwrap();
            }
            self.published.lock().unwrap().push(page.path.clone());
            Ok(())
        }

        fn retire(
            &self,
            _: &ExternalMemorySettings,
            _: &ExternalScope,
            path: &str,
        ) -> Result<(), ExternalError> {
            self.retired.lock().unwrap().push(path.to_owned());
            Ok(())
        }
    }

    #[test]
    fn a_sync_takes_its_list_of_notes_only_after_the_queued_job_in_progress_is_done() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let server = Arc::new(Gated {
            open: Mutex::new(false),
            opened: std::sync::Condvar::new(),
            inside: AtomicBool::new(false),
            published: Mutex::new(Vec::new()),
            retired: Mutex::new(Vec::new()),
        });
        let mirror = Arc::new(ExternalMirror::queued(
            server.clone(),
            Arc::new(Context {
                settings: sharing(),
                folders: vec!["/work/billing".to_owned()],
            }),
            Arc::new(MemoryLedger::default()),
        ));
        // m1 was approved, so the queue starts publishing it and is held inside the server
        mirror.changed(&approved("m1"));
        while !server.inside.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        let asked = Arc::new(AtomicBool::new(false));
        let syncing = {
            let (mirror, asked) = (mirror.clone(), asked.clone());
            std::thread::spawn(move || {
                mirror
                    .sync_all(|| {
                        asked.store(true, Ordering::SeqCst);
                        vec![approved("m1"), approved("m2")]
                    })
                    .unwrap()
            })
        };
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(
            !asked.load(Ordering::SeqCst),
            "the sync asked for the notes while a queued job was still running"
        );

        *server.open.lock().unwrap() = true;
        server.opened.notify_all();
        let report = syncing.join().unwrap();

        assert!(asked.load(Ordering::SeqCst));
        // neither note was taken for an orphan and retired
        assert_eq!(report.retired, 0);
        assert!(server.retired.lock().unwrap().is_empty());
        let published = server.published.lock().unwrap();
        assert!(published.contains(&"atlas/m1.md".to_owned()));
        assert!(published.contains(&"atlas/m2.md".to_owned()));
    }

    #[test]
    fn a_job_that_panics_does_not_end_the_queue() {
        struct PanicsOnce(std::sync::atomic::AtomicBool, Mutex<Vec<String>>);
        impl ExternalMemory for PanicsOnce {
            fn search(
                &self,
                _: &ExternalMemorySettings,
                _: &ExternalScope,
                _: &str,
                _: u8,
            ) -> Result<Vec<ExternalHit>, ExternalError> {
                Ok(Vec::new())
            }
            fn publish(
                &self,
                _: &ExternalMemorySettings,
                _: &ExternalScope,
                page: &Page,
            ) -> Result<(), ExternalError> {
                assert!(
                    self.0.swap(true, std::sync::atomic::Ordering::SeqCst),
                    "a bug in an adapter"
                );
                self.1.lock().unwrap().push(page.path.clone());
                Ok(())
            }
            fn retire(
                &self,
                _: &ExternalMemorySettings,
                _: &ExternalScope,
                _: &str,
            ) -> Result<(), ExternalError> {
                Ok(())
            }
        }
        let server = Arc::new(PanicsOnce(
            std::sync::atomic::AtomicBool::new(false),
            Mutex::new(Vec::new()),
        ));
        let mirror = ExternalMirror::queued(
            server.clone(),
            Arc::new(Context {
                settings: sharing(),
                folders: vec!["/work/billing".to_owned()],
            }),
            Arc::new(MemoryLedger::default()),
        );

        mirror.changed(&approved("m1")); // panics inside the adapter
        mirror.changed(&approved("m2"));
        mirror.flush();

        assert_eq!(*server.1.lock().unwrap(), ["atlas/m2.md"]);
    }
}
