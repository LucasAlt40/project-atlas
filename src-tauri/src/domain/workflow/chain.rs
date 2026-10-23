//! A task that changes several projects of a workspace (ADR 0043). A run still works in exactly one
//! project, with one worktree, one review and one merge (ADR 0033): a task for the frontend and the
//! backend is one run per project, in order, each started when the one before it completed. This is
//! the record those runs share, so each knows where it stands and what the earlier ones changed.

use serde::{Deserialize, Serialize};

use super::WorkflowExecutionStatus;

/// How many changed files of an earlier run the next one is told about.
pub const MAX_CHAIN_FILES: usize = 40;
/// How much of an earlier run's diff the next one is given.
pub const MAX_CHAIN_DIFF_CHARS: usize = 6_000;
/// How much of an earlier run's last summary the next one is given.
pub const MAX_CHAIN_SUMMARY_CHARS: usize = 1_200;

/// A project of the chain, as it was named when the chain began.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainProject {
    pub id: String,
    pub name: String,
}

/// What a run of the chain that came before this one left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainStep {
    pub project_id: String,
    pub execution_id: String,
    pub status: WorkflowExecutionStatus,
    /// The branch its changes are on, when it had a worktree.
    #[serde(default)]
    pub branch: Option<String>,
    /// The files it changed, by Git; at most [`MAX_CHAIN_FILES`].
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub files_changed: u32,
    /// The start of its diff, at most [`MAX_CHAIN_DIFF_CHARS`].
    #[serde(default)]
    pub diff: String,
    /// What its last step said it did, at most [`MAX_CHAIN_SUMMARY_CHARS`].
    #[serde(default)]
    pub summary: String,
}

/// The runs of one task over several projects. Every run of the chain carries it, with its own
/// `position`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectChain {
    /// The id of the chain's first run, shared by all of them.
    pub id: String,
    /// The projects the task changes, in the order they are worked in. At least two.
    pub projects: Vec<ChainProject>,
    /// Which of them this run works in.
    pub position: usize,
    /// Projects every run of the chain consults and none changes.
    #[serde(default)]
    pub read_project_ids: Vec<String>,
    /// The runs before this one, in order.
    #[serde(default)]
    pub earlier: Vec<ChainStep>,
}

impl ProjectChain {
    pub fn current(&self) -> Option<&ChainProject> {
        self.projects.get(self.position)
    }

    pub fn next(&self) -> Option<&ChainProject> {
        self.projects.get(self.position + 1)
    }

    pub fn later(&self) -> &[ChainProject] {
        self.projects.get(self.position + 1..).unwrap_or_default()
    }

    /// What the run of the next project carries, given what this run left.
    pub fn advanced(&self, step: ChainStep) -> Option<Self> {
        self.next()?;
        let mut earlier = self.earlier.clone();
        earlier.push(step);
        Some(Self {
            id: self.id.clone(),
            projects: self.projects.clone(),
            position: self.position + 1,
            read_project_ids: self.read_project_ids.clone(),
            earlier,
        })
    }

    /// What a run of the chain consults: the chain's other projects (the earlier ones as they are
    /// in the person's checkout, the later ones before they change) and the chain's own reads.
    pub fn consulted_ids(&self) -> Vec<String> {
        let current = self.current().map(|p| p.id.as_str());
        let mut ids: Vec<String> = self
            .projects
            .iter()
            .filter(|p| Some(p.id.as_str()) != current)
            .map(|p| p.id.clone())
            .collect();
        for id in &self.read_project_ids {
            if !ids.contains(id) && Some(id.as_str()) != current {
                ids.push(id.clone());
            }
        }
        ids
    }

    pub fn name_of<'a>(&'a self, id: &'a str) -> &'a str {
        self.projects
            .iter()
            .find(|p| p.id == id)
            .map_or(id, |p| p.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain() -> ProjectChain {
        ProjectChain {
            id: "wfx-1".to_owned(),
            projects: vec![
                ChainProject {
                    id: "back".to_owned(),
                    name: "API".to_owned(),
                },
                ChainProject {
                    id: "front".to_owned(),
                    name: "Web".to_owned(),
                },
            ],
            position: 0,
            read_project_ids: vec!["docs".to_owned(), "front".to_owned()],
            earlier: Vec::new(),
        }
    }

    fn step() -> ChainStep {
        ChainStep {
            project_id: "back".to_owned(),
            execution_id: "wfx-1".to_owned(),
            status: WorkflowExecutionStatus::Completed,
            branch: Some("atlas/wfx-1".to_owned()),
            files: vec!["src/api.rs".to_owned()],
            files_changed: 1,
            diff: String::new(),
            summary: "Added the endpoint".to_owned(),
        }
    }

    #[test]
    fn a_run_consults_the_other_projects_of_the_chain_and_its_reads_once() {
        assert_eq!(chain().consulted_ids(), ["front", "docs"]);
    }

    #[test]
    fn advancing_moves_to_the_next_project_and_keeps_what_the_run_left() {
        let next = chain().advanced(step()).unwrap();
        assert_eq!(next.position, 1);
        assert_eq!(next.current().unwrap().id, "front");
        assert_eq!(next.earlier, vec![step()]);
        assert_eq!(next.consulted_ids(), ["back", "docs"]);
        assert_eq!(next.later(), &[] as &[ChainProject]);
        assert!(next.advanced(step()).is_none(), "the last run has no next");
    }
}
