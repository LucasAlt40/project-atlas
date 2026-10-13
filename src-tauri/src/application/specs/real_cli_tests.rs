//! The real `OpenSpec` command, through the real guard, on a change Atlas published itself. Free
//! (no model is involved) and needs the command installed, so it runs only when asked for, by exact
//! name: `cargo test real_openspec_cli_accepts_what_atlas_publishes -- --ignored --exact --nocapture`.

use std::sync::Arc;

use super::cli::OpenSpecCli;
use super::framework::SpecFramework;
use super::openspec::validate::tests_support::{DESIGN, PROPOSAL, SPEC, TASKS};
use super::openspec::OpenSpecAdapter;
use super::ports::ProjectFiles;
use crate::application::config::memory::MemoryStore;
use crate::application::config::ConfigRepository;
use crate::application::process::ProcessRunner;
use crate::application::runtimes::RUNTIME_PROGRAMS;
use crate::application::security::{
    ApprovalBroker, AuditLog, GuardedProcessRunner, NoSandbox, SecurityService,
};
use crate::domain::spec::SpecFile;
use crate::infrastructure::{FsProjectFiles, SystemProcessRunner};

fn doc(path: &str, content: &str) -> Vec<SpecFile> {
    vec![SpecFile {
        path: path.to_owned(),
        content: content.to_owned(),
    }]
}

#[test]
#[ignore = "needs the openspec command installed; run by exact name"]
fn real_openspec_cli_accepts_what_atlas_publishes() {
    let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
    let runner: Arc<dyn ProcessRunner> = Arc::new(
        GuardedProcessRunner::new(
            Arc::new(SystemProcessRunner::new()),
            Arc::new(SecurityService::new(config)),
            Arc::new(ApprovalBroker::new()),
            Arc::new(AuditLog::default()),
            Arc::new(NoSandbox),
            RUNTIME_PROGRAMS.map(str::to_owned).to_vec(),
        )
        .with_tools(vec!["openspec".to_owned()]),
    );
    let root = std::env::temp_dir().join(format!("atlas-real-openspec-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root = root.to_string_lossy().into_owned();

    // What Atlas publishes, written by Atlas's own writer.
    let documents = [
        ("proposal", doc("proposal.md", PROPOSAL)),
        ("specs", doc("specs/dark-mode/spec.md", SPEC)),
        ("design", doc("design.md", DESIGN)),
        ("tasks", doc("tasks.md", TASKS)),
    ]
    .into_iter()
    .map(|(id, files)| (id.to_owned(), files))
    .collect();
    let files = FsProjectFiles;
    for file in OpenSpecAdapter
        .publish_files(&documents, 1_791_849_600_000)
        .unwrap()
    {
        files
            .create(
                &root,
                &format!("openspec/changes/dark-mode/{}", file.path),
                &file.content,
            )
            .unwrap();
    }

    let cli = OpenSpecCli::new(runner);
    let status = cli.status(&root);
    println!("status: {status:?}");
    assert!(status.available && status.compatible, "{status:?}");

    let checked = cli.validate(&root, "dark-mode").unwrap();
    println!("validate: {checked:?}");
    assert!(checked.valid, "{checked:?}");

    let archived = cli.archive(&root, "dark-mode").unwrap();
    println!("archive: {archived:?}");
    let name = archived
        .archived_as
        .expect("the command says where it went");
    assert!(files.change_exists(&root, "openspec/changes/archive", &name));
    assert!(files
        .read(&root, "openspec/specs/dark-mode/spec.md")
        .is_some());
    assert!(!files.change_exists(&root, "openspec/changes", "dark-mode"));
}
