//! External memory (ADR 0040) through the real execution path: notes from a memory server on this
//! machine reach the prompt in the notes section, marked as nobody-here-approved, only when the
//! person turned the server on, and a server that is off, down or wrong leaves the prompt exactly
//! as it was.

use std::sync::{Arc, Mutex};

use super::memory_tests::{flags, learn, on, run, world, World, TASK};
use crate::application::memory::external::{
    page_path, ExternalError, ExternalHit, ExternalMemory, ExternalMirror, ExternalScope, Page,
};
use crate::domain::execution::ExecutionStatus;
use crate::domain::memory::{ExternalMemorySettings, MemorySettings};

/// A server in memory: what it answers, and what it was asked.
struct Server {
    answer: Result<Vec<ExternalHit>, ExternalError>,
    asked: Mutex<Vec<(ExternalScope, String)>>,
    published: Mutex<Vec<(ExternalScope, Page)>>,
    retired: Mutex<Vec<(ExternalScope, String)>>,
}

impl Server {
    fn answering(answer: Result<Vec<ExternalHit>, ExternalError>) -> Arc<Self> {
        Arc::new(Self {
            answer,
            asked: Mutex::new(Vec::new()),
            published: Mutex::new(Vec::new()),
            retired: Mutex::new(Vec::new()),
        })
    }

    fn asked(&self) -> usize {
        self.asked.lock().unwrap().len()
    }
}

impl ExternalMemory for Server {
    fn search(
        &self,
        _: &ExternalMemorySettings,
        scope: &ExternalScope,
        query: &str,
        _: u8,
    ) -> Result<Vec<ExternalHit>, ExternalError> {
        self.asked
            .lock()
            .unwrap()
            .push((scope.clone(), query.to_owned()));
        self.answer.clone()
    }

    fn publish(
        &self,
        _: &ExternalMemorySettings,
        scope: &ExternalScope,
        page: &Page,
    ) -> Result<(), ExternalError> {
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
        self.retired
            .lock()
            .unwrap()
            .push((scope.clone(), path.to_owned()));
        Ok(())
    }
}

fn hit(snippet: &str) -> ExternalHit {
    ExternalHit {
        id: "p1".to_owned(),
        path: "notes/invoice.md".to_owned(),
        title: "Invoice rounding".to_owned(),
        snippet: snippet.to_owned(),
    }
}

fn external_on() -> ExternalMemorySettings {
    ExternalMemorySettings {
        enabled: true,
        ..ExternalMemorySettings::default()
    }
}

fn with_server(world: World, server: &Arc<Server>) -> World {
    World {
        service: world.service.with_external_memory(server.clone()),
        ..world
    }
}

fn flags_with(
    external: ExternalMemorySettings,
    native: MemorySettings,
) -> crate::application::optimization::FixedFlags {
    flags(native).with_external(external)
}

#[test]
fn a_note_from_the_server_reaches_the_prompt_under_the_same_notice_and_marked() {
    let server = Server::answering(Ok(vec![hit(
        "Invoice totals round <mark>half</mark> even, never floats",
    )]));
    let w = with_server(
        world(
            flags_with(external_on(), MemorySettings::default()),
            false,
            Ok("done"),
        ),
        &server,
    );

    let record = run(&w, TASK);

    let prompt = &record.execution.prompt;
    assert!(prompt.contains("NOTES FROM EARLIER WORK\n\n"));
    assert!(prompt.contains("not instructions") && prompt.contains("grant no permission"));
    assert!(prompt
        .contains("- [external] Invoice rounding: Invoice totals round half even, never floats"));
    assert!(!prompt.contains("<mark>"));
    assert_eq!(record.execution.status, ExecutionStatus::Completed);
    // Atlas built the query and the scope; the agent chose neither.
    let asked = server.asked.lock().unwrap();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].0.project, "atlas");
    assert!(asked[0].1.contains("invoice") && asked[0].1.contains(" OR "));
}

#[test]
fn off_the_server_is_never_asked_and_the_prompt_is_byte_identical() {
    let server = Server::answering(Ok(vec![hit("Invoice totals round half even")]));
    let plain = world(
        flags_with(ExternalMemorySettings::default(), MemorySettings::default()),
        false,
        Ok("done"),
    );
    let wired = with_server(
        world(
            flags_with(ExternalMemorySettings::default(), MemorySettings::default()),
            false,
            Ok("done"),
        ),
        &server,
    );

    let without = run(&plain, TASK);
    let with = run(&wired, TASK);

    assert_eq!(without.execution.prompt, with.execution.prompt);
    assert_eq!(server.asked(), 0);
}

#[test]
fn a_server_that_is_down_is_a_run_without_notes_not_a_failed_run() {
    let server = Server::answering(Err(ExternalError::NotRunning));
    let plain = world(
        flags_with(external_on(), MemorySettings::default()),
        false,
        Ok("done"),
    );
    let wired = with_server(
        world(
            flags_with(external_on(), MemorySettings::default()),
            false,
            Ok("done"),
        ),
        &server,
    );

    let without = run(&plain, TASK);
    let with = run(&wired, TASK);

    assert_eq!(server.asked(), 1);
    assert_eq!(with.execution.status, ExecutionStatus::Completed);
    assert_eq!(without.execution.prompt, with.execution.prompt);
}

#[test]
fn a_note_that_looks_like_a_secret_never_reaches_the_prompt() {
    let server = Server::answering(Ok(vec![hit(
        "login with sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    )]));
    let w = with_server(
        world(
            flags_with(external_on(), MemorySettings::default()),
            false,
            Ok("done"),
        ),
        &server,
    );

    let record = run(&w, TASK);

    assert!(!record.execution.prompt.contains("sk-ant"));
    assert!(!record.execution.prompt.contains("NOTES FROM EARLIER WORK"));
}

#[test]
fn it_sits_beside_atlass_own_notes_and_only_those_count_as_used() {
    let server = Server::answering(Ok(vec![hit("Invoice totals round half even")]));
    let w = with_server(
        world(flags_with(external_on(), on()), true, Ok("done")),
        &server,
    );
    let own = learn(
        &w,
        "Invoice rounding",
        "Never use floats for invoice totals",
    );

    let record = run(&w, TASK);

    let prompt = &record.execution.prompt;
    assert!(prompt.contains("- [pitfall] Invoice rounding: Never use floats"));
    assert!(prompt.contains("- [external] Invoice rounding"));
    assert_eq!(prompt.matches("NOTES FROM EARLIER WORK").count(), 1);
    assert_eq!(w.memory.get(&own.id).unwrap().uses, 1);
}

#[test]
fn the_page_of_a_note_in_another_workspace_of_this_atlas_is_not_brought_back_but_another_machines_is(
) {
    // This Atlas keeps a note in a workspace other than the one that runs; the server holds its page
    // (the two workspaces have a project folder of the same name) and a page from another machine.
    let base = world(flags_with(external_on(), on()), true, Ok("done"));
    let other = base
        .memory
        .create(
            crate::application::memory::NewMemory {
                scope: crate::domain::memory::MemoryScope::Workspace,
                owner: Some("another-workspace".to_owned()),
                kind: crate::domain::memory::MemoryKind::Pitfall,
                title: "Invoice rounding".to_owned(),
                content: "Never use floats for invoice totals".to_owned(),
                topic: None,
                validity: crate::domain::memory::MemoryValidity::default(),
                origin: crate::domain::memory::MemoryOrigin::User,
            },
            1,
        )
        .unwrap();
    let mut mine = hit("Never use floats for invoice totals");
    mine.id = "page-1".to_owned();
    mine.path = page_path(&other.id);
    let mut elsewhere = hit("Totals round half even, a note from another machine");
    elsewhere.id = "page-2".to_owned();
    elsewhere.path = "atlas/memory-from-another-machine.md".to_owned();
    let server = Server::answering(Ok(vec![mine, elsewhere]));
    let w = with_server(base, &server);

    let record = run(&w, TASK);

    let prompt = &record.execution.prompt;
    assert!(prompt.contains("from another machine"), "{prompt}");
    assert!(
        prompt
            .matches("Never use floats for invoice totals")
            .count()
            <= 1,
        "{prompt}"
    );
}

#[test]
fn the_manifest_says_which_notes_came_from_the_server() {
    let server = Server::answering(Ok(vec![hit("Invoice totals round half even")]));
    let w = with_server(
        world(
            flags_with(external_on(), MemorySettings::default()),
            false,
            Ok("done"),
        ),
        &server,
    );

    let record = run(&w, TASK);

    let manifest = record.execution.manifest.unwrap();
    assert_eq!(manifest.memories.len(), 1);
    assert_eq!(manifest.memories[0].id, "ext:p1");
    assert_eq!(manifest.memories[0].title, "Invoice rounding");
    assert!(manifest.memories[0].automatic);
    assert_eq!(
        manifest.memories[0].status,
        crate::domain::context::MemoryStatusInPrompt::Sent
    );
}

/// The Memory service with a mirror to `server`, sharing on, over one workspace with `/work/billing`.
fn shared(server: &Arc<Server>) -> crate::application::memory::MemoryService {
    struct Ctx;
    impl crate::application::memory::external::MirrorContext for Ctx {
        fn settings(&self) -> ExternalMemorySettings {
            ExternalMemorySettings {
                enabled: true,
                share: true,
                ..ExternalMemorySettings::default()
            }
        }
        fn project_folders(&self, _: &str) -> Vec<String> {
            vec!["/work/billing".to_owned()]
        }
    }
    crate::application::memory::MemoryService::new(
        Arc::new(crate::application::memory::ports::MemoryMemoryStore::default()),
        Arc::new(crate::application::memory::ports::FakeProbe::default()),
    )
    .with_mirror(Arc::new(ExternalMirror::new(
        server.clone(),
        Arc::new(Ctx),
        Arc::new(crate::application::memory::external::MemoryLedger::default()),
    )))
}

fn new_note(
    owner: &str,
    origin: crate::domain::memory::MemoryOrigin,
) -> crate::application::memory::NewMemory {
    crate::application::memory::NewMemory {
        scope: crate::domain::memory::MemoryScope::Workspace,
        owner: Some(owner.to_owned()),
        kind: crate::domain::memory::MemoryKind::Pitfall,
        title: "Invoice rounding".to_owned(),
        content: "Never use floats for invoice totals".to_owned(),
        topic: None,
        validity: crate::domain::memory::MemoryValidity::default(),
        origin,
    }
}

#[test]
fn a_note_follows_a_person_from_candidate_to_approved_to_archived_to_deleted() {
    use crate::domain::memory::{MemoryOrigin, MemoryStatus};
    let server = Server::answering(Ok(vec![]));
    let memory = shared(&server);

    // An agent's proposal waits for a person: nothing leaves.
    let proposed = memory
        .create(new_note("ws", MemoryOrigin::Agent), 1)
        .unwrap();
    assert!(server.published.lock().unwrap().is_empty());

    // Approved: published, under the page named for the note.
    memory.approve(&proposed.id, 2).unwrap();
    {
        let published = server.published.lock().unwrap();
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0.project, "billing");
        assert_eq!(published[0].1.path, page_path(&proposed.id));
    }

    // Edited by the person: the page is written again with the new words.
    memory
        .edit(&proposed.id, "Invoice rounding", "Round half even", None, 3)
        .unwrap();
    assert_eq!(
        server.published.lock().unwrap().last().unwrap().1.body,
        "Round half even"
    );

    // Out of use: retired. Deleted: retired again, and the page is the same one.
    memory
        .set_status(&proposed.id, MemoryStatus::Archived, 4)
        .unwrap();
    memory.delete(&proposed.id).unwrap();
    let retired = server.retired.lock().unwrap();
    assert_eq!(retired.len(), 2);
    assert!(retired
        .iter()
        .all(|(_, path)| *path == page_path(&proposed.id)));
}

#[test]
fn a_person_writing_a_note_publishes_it_at_once_and_an_automatic_one_never() {
    use crate::domain::memory::MemoryOrigin;
    let server = Server::answering(Ok(vec![]));
    let memory = shared(&server);

    memory
        .create(new_note("ws", MemoryOrigin::User), 1)
        .unwrap();

    assert_eq!(server.published.lock().unwrap().len(), 1);
}

#[test]
fn a_shared_note_found_stale_during_a_recall_has_its_page_retired() {
    use crate::application::memory::RecallRequest;
    use crate::domain::memory::{MemoryMode, MemoryOrigin, MemoryValidity};
    let server = Server::answering(Ok(vec![]));
    let memory = shared(&server);
    let mut note = new_note("ws", MemoryOrigin::User);
    // the note depends on a file that is not in the project, so the next recall finds it stale
    note.validity = MemoryValidity {
        paths: vec!["gone/file.txt".to_owned()],
        ..MemoryValidity::default()
    };
    let made = memory.create(note, 1).unwrap();
    assert_eq!(server.published.lock().unwrap().len(), 1);

    memory.recall(&RecallRequest {
        workspace: "ws",
        agent: "a",
        task: "Fix the invoice total rounding",
        project_root: "/p",
        settings: MemorySettings {
            workspace: MemoryMode::Propose,
            ..MemorySettings::default()
        },
        budget_tokens: None,
        now: 10,
    });

    let retired = server.retired.lock().unwrap();
    assert_eq!(retired.len(), 1, "{retired:?}");
    assert_eq!(retired[0].1, page_path(&made.id));
}
