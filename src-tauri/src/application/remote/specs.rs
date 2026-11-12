//! What a phone sees of the specifications (ADR 0035 §15, ADR 0034): it follows them, never moves
//! them. Each document's state and the text of its current revision, with what looks like a
//! secret taken out and every text bounded; no folder of the project, no worktree, no diff.

use std::collections::BTreeMap;

use serde::Serialize;

use super::view::safe;
use crate::application::specs::service::SpecService;
use crate::domain::spec::rules::ArtifactPhase;
use crate::domain::spec::{RevisionOrigin, Spec, SpecStatus};

/// The request, as the phone shows it.
const MAX_REQUEST: usize = 2_000;
/// One file of a document.
const MAX_FILE_TEXT: usize = 20_000;
/// Specifications in one listing.
const MAX_SPECS: usize = 60;

/// A spec as Atlas has it now, with what the screens say about it.
pub struct SpecState {
    pub spec: Spec,
    pub status: SpecStatus,
    pub phases: BTreeMap<String, ArtifactPhase>,
}

/// Where the gateway reads specifications from.
pub trait SpecSource: Send + Sync {
    /// The specs of a workspace, as they are now.
    fn of_workspace(&self, workspace_id: &str) -> Vec<SpecState>;
    fn one(&self, id: &str) -> Option<SpecState>;
}

impl SpecSource for SpecService {
    fn of_workspace(&self, workspace_id: &str) -> Vec<SpecState> {
        self.list(workspace_id)
            .into_iter()
            .filter_map(|summary| self.one(&summary.id))
            .collect()
    }

    fn one(&self, id: &str) -> Option<SpecState> {
        let view = self.view(id).ok()?;
        Some(SpecState {
            spec: view.spec,
            status: view.status,
            phases: view.phases,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSpecSummary {
    pub id: String,
    pub title: String,
    pub workspace_name: String,
    pub status: SpecStatus,
    /// Documents approved so far, out of `documents`.
    pub approved: usize,
    pub documents: usize,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFile {
    /// `proposal.md`, or the capability for a specification file. Never a folder of the project.
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDocument {
    pub id: String,
    pub phase: Option<ArtifactPhase>,
    pub revision: Option<u32>,
    pub origin: Option<RevisionOrigin>,
    pub files: Vec<RemoteFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSpecDetail {
    #[serde(flatten)]
    pub summary: RemoteSpecSummary,
    pub request: String,
    pub documents: Vec<RemoteDocument>,
    /// Always false: approving, asking for changes and everything else stays at the computer.
    pub can_decide: bool,
}

pub fn summary(state: &SpecState, workspace_name: &str) -> RemoteSpecSummary {
    RemoteSpecSummary {
        id: state.spec.id.clone(),
        title: safe(&state.spec.title, 200),
        workspace_name: workspace_name.to_owned(),
        status: state.status,
        approved: state
            .spec
            .artifacts
            .iter()
            .filter(|a| a.is_approved())
            .count(),
        documents: state.spec.artifacts.len(),
        updated_at: state.spec.updated_at,
    }
}

pub fn detail(state: &SpecState, workspace_name: &str) -> RemoteSpecDetail {
    let documents = state
        .spec
        .artifacts
        .iter()
        .map(|artifact| {
            let latest = artifact.latest();
            RemoteDocument {
                id: artifact.id.clone(),
                phase: state.phases.get(&artifact.id).copied(),
                revision: latest.map(|r| r.number),
                origin: latest.map(|r| r.origin),
                files: latest
                    .map(|r| {
                        r.files
                            .iter()
                            .map(|f| RemoteFile {
                                name: file_name(&f.path),
                                text: safe(&f.content, MAX_FILE_TEXT),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect();
    RemoteSpecDetail {
        summary: summary(state, workspace_name),
        request: safe(&state.spec.request, MAX_REQUEST),
        documents,
        can_decide: super::policy::SPECS.decide,
    }
}

/// The name a person knows a file by: the capability of a specification file, the file name of
/// the others. Nothing of where it is.
fn file_name(path: &str) -> String {
    path.strip_prefix("specs/")
        .and_then(|rest| rest.strip_suffix("/spec.md"))
        .map_or_else(
            || path.rsplit('/').next().unwrap_or(path).to_owned(),
            str::to_owned,
        )
}

/// The newest first, bounded.
pub fn newest_first(mut specs: Vec<RemoteSpecSummary>) -> Vec<RemoteSpecSummary> {
    specs.sort_by_key(|spec| std::cmp::Reverse(spec.updated_at));
    specs.truncate(MAX_SPECS);
    specs
}

#[cfg(test)]
pub(crate) mod fake {
    use super::{SpecSource, SpecState};

    /// Specs held in memory, for the gateway's tests.
    pub struct FakeSpecs(pub Vec<SpecState>);

    impl SpecSource for FakeSpecs {
        fn of_workspace(&self, workspace_id: &str) -> Vec<SpecState> {
            self.0
                .iter()
                .filter(|s| s.spec.workspace_id == workspace_id)
                .map(copy)
                .collect()
        }

        fn one(&self, id: &str) -> Option<SpecState> {
            self.0.iter().find(|s| s.spec.id == id).map(copy)
        }
    }

    fn copy(state: &SpecState) -> SpecState {
        SpecState {
            spec: state.spec.clone(),
            status: state.status,
            phases: state.phases.clone(),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::domain::spec::rules::{create, NewSpec};
    use crate::domain::spec::{ArtifactKind, FrameworkKind, Revision, SpecFile, SpecOrigin};

    /// A spec with a proposal (holding a token-looking line) and a specification file.
    pub fn state(workspace_id: &str) -> SpecState {
        let mut spec = create(
            NewSpec {
                id: "spec-1".to_owned(),
                title: "Dark mode".to_owned(),
                request: "Add a dark mode".to_owned(),
                context: String::new(),
                workspace_id: workspace_id.to_owned(),
                project_id: "p1".to_owned(),
                framework: FrameworkKind::Openspec,
                framework_schema: "spec-driven/1".to_owned(),
                change_id: "dark-mode".to_owned(),
                author_agent_id: "a1".to_owned(),
                reviewer_agent_id: "a1".to_owned(),
                origin: SpecOrigin::Created,
                skip_specs: false,
            },
            &[
                (ArtifactKind::Proposal, vec![]),
                (ArtifactKind::Specs, vec![ArtifactKind::Proposal]),
            ],
            1,
        );
        let revision = |files: Vec<SpecFile>| Revision {
            number: 1,
            files,
            origin: RevisionOrigin::Generated,
            run_id: None,
            agent_id: None,
            based_on: BTreeMap::new(),
            created_at: 1,
        };
        spec.artifacts[0].revisions.push(revision(vec![SpecFile {
            path: "proposal.md".to_owned(),
            content: "# Proposal\n\nUse key sk-ant-api03-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n"
                .to_owned(),
        }]));
        spec.artifacts[1].revisions.push(revision(vec![SpecFile {
            path: "specs/ui/dark-mode/spec.md".to_owned(),
            content: "## ADDED Requirements\n".to_owned(),
        }]));
        SpecState {
            spec,
            status: SpecStatus::AwaitingReview,
            phases: BTreeMap::from([
                ("proposal".to_owned(), ArtifactPhase::AwaitingReview),
                ("specs".to_owned(), ArtifactPhase::Blocked),
            ]),
        }
    }

    #[test]
    fn the_phone_reads_each_document_without_folders_secrets_or_a_way_to_decide() {
        let detail = detail(&state("w1"), "Atlas");

        assert!(!detail.can_decide);
        assert_eq!(detail.summary.documents, 2);
        let names: Vec<&str> = detail
            .documents
            .iter()
            .flat_map(|d| d.files.iter().map(|f| f.name.as_str()))
            .collect();
        assert_eq!(names, ["proposal.md", "ui/dark-mode"]);
        let text = &detail.documents[0].files[0].text;
        assert!(!text.contains("sk-ant-api03"), "{text}");
        assert_eq!(
            detail.documents[0].phase,
            Some(ArtifactPhase::AwaitingReview)
        );
        let json = serde_json::to_string(&detail).unwrap();
        assert!(!json.contains("openspec/changes") && !json.contains("specs/ui"));
    }
}
