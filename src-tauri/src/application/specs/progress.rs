//! How far an implementation of a spec has got, kept to what can be told apart: what a step
//! *reported* (it said it finished a task) and what Atlas *confirms* (the run those steps belong to
//! ended well). No test oracle exists per task, so the confirmation is only as strong as the run's
//! own end, and the screen says which of the two it is showing.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::domain::spec::{ImplementationLink, Spec};
use crate::domain::workflow::{WorkflowExecution, WorkflowExecutionStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Nobody has said anything about it.
    Pending,
    /// A step said it finished it. Nothing more is known.
    Reported,
    /// A step said it finished it and the run ended successfully.
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub id: String,
    pub text: String,
    pub state: TaskState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationProgress {
    pub workflow_execution_id: String,
    pub workflow_name: String,
    /// `None` when the run is no longer in Atlas's records.
    pub status: Option<WorkflowExecutionStatus>,
    /// The spec's documents changed since this run was given them.
    pub spec_changed: bool,
    pub tasks: Vec<TaskProgress>,
    pub reported: u32,
    pub confirmed: u32,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The progress of one implementation, from the run that carries the tasks it was given.
pub fn of_link(
    spec: &Spec,
    link: &ImplementationLink,
    execution: Option<&WorkflowExecution>,
) -> ImplementationProgress {
    let given = execution.and_then(|e| e.spec.as_ref());
    let reported: BTreeSet<&String> = execution
        .map(|e| e.state.completed_tasks.values().flatten().collect())
        .unwrap_or_default();
    let ended_well = execution.is_some_and(|e| e.status == WorkflowExecutionStatus::Completed);
    let tasks: Vec<TaskProgress> = given
        .map(|g| {
            g.tasks
                .iter()
                .map(|task| {
                    let said = reported.contains(&task.id);
                    TaskProgress {
                        id: task.id.clone(),
                        text: task.text.clone(),
                        state: match (said, ended_well) {
                            (true, true) => TaskState::Confirmed,
                            (true, false) => TaskState::Reported,
                            (false, _) => TaskState::Pending,
                        },
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    ImplementationProgress {
        workflow_execution_id: link.workflow_execution_id.clone(),
        workflow_name: link.workflow_name.clone(),
        status: execution.map(|e| e.status),
        spec_changed: link.revisions != spec.current_revisions(),
        reported: count(
            tasks
                .iter()
                .filter(|t| t.state != TaskState::Pending)
                .count(),
        ),
        confirmed: count(
            tasks
                .iter()
                .filter(|t| t.state == TaskState::Confirmed)
                .count(),
        ),
        tasks,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::domain::spec::rules::tests_support::all_approved;
    use crate::domain::workflow::{SpecLink, SpecTaskRef};

    fn run(status: WorkflowExecutionStatus, reported: &[&str]) -> WorkflowExecution {
        let workflow = crate::application::workflow::test_support::workflow(Vec::new(), Vec::new());
        let mut exec = WorkflowExecution::new("wfx".to_owned(), workflow, "task".to_owned(), 1);
        exec.status = status;
        exec.spec = Some(SpecLink {
            spec_id: "spec-1".to_owned(),
            title: "T".to_owned(),
            change_id: "t".to_owned(),
            revisions: BTreeMap::new(),
            digest: String::new(),
            tasks: ["1.1", "1.2", "2.1"]
                .iter()
                .map(|id| SpecTaskRef {
                    id: (*id).to_owned(),
                    text: format!("task {id}"),
                })
                .collect(),
            published_root: None,
            integrations: Vec::new(),
        });
        exec.state.completed_tasks.insert(
            "step".to_owned(),
            reported.iter().map(|s| (*s).to_owned()).collect(),
        );
        exec
    }

    fn link(spec: &Spec) -> ImplementationLink {
        ImplementationLink {
            workflow_execution_id: "wfx".to_owned(),
            workflow_id: "wf".to_owned(),
            workflow_name: "Feature".to_owned(),
            revisions: spec.current_revisions(),
            started_at: 1,
        }
    }

    #[test]
    fn what_a_step_says_is_reported_and_only_a_run_that_ended_well_confirms_it() {
        let spec = all_approved();
        let running = of_link(
            &spec,
            &link(&spec),
            Some(&run(WorkflowExecutionStatus::Running, &["1.1"])),
        );
        assert_eq!(
            running.tasks.iter().map(|t| t.state).collect::<Vec<_>>(),
            vec![TaskState::Reported, TaskState::Pending, TaskState::Pending]
        );
        assert_eq!((running.reported, running.confirmed), (1, 0));

        let done = of_link(
            &spec,
            &link(&spec),
            Some(&run(WorkflowExecutionStatus::Completed, &["1.1", "2.1"])),
        );
        assert_eq!(
            done.tasks.iter().map(|t| t.state).collect::<Vec<_>>(),
            vec![
                TaskState::Confirmed,
                TaskState::Pending,
                TaskState::Confirmed
            ]
        );
        assert_eq!((done.reported, done.confirmed), (2, 2));

        let failed = of_link(
            &spec,
            &link(&spec),
            Some(&run(WorkflowExecutionStatus::Failed, &["1.1"])),
        );
        assert_eq!(failed.tasks[0].state, TaskState::Reported);
    }

    #[test]
    fn a_task_nobody_gave_the_run_cannot_be_reported_into_existence() {
        let spec = all_approved();

        let progress = of_link(
            &spec,
            &link(&spec),
            Some(&run(WorkflowExecutionStatus::Completed, &["9.9"])),
        );

        assert_eq!(progress.reported, 0);
    }

    #[test]
    fn a_spec_changed_since_the_run_was_given_it_is_said_so() {
        let mut spec = all_approved();
        let mut stale = link(&spec);
        stale.revisions.insert("tasks".to_owned(), 0);

        let progress = of_link(&spec, &stale, None);

        assert!(progress.spec_changed);
        assert_eq!(
            progress.status, None,
            "a run no longer in the records is not invented"
        );
        spec.version += 1;
        assert!(!of_link(&spec, &link(&spec), None).spec_changed);
    }
}
