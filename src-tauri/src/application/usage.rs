use std::collections::HashSet;
use std::sync::Arc;

use super::config::ConfigRepository;
use super::errors::AppError;
use crate::domain::usage::{
    AgentUsageSummary, QuotaInfo, UsageRecord, UsageTotals, UsageWindows, WorkspaceUsageSummary,
};

/// Executions older than this are dropped from the ledger when a new one is recorded.
const RETENTION_MS: u64 = 100 * 24 * 60 * 60 * 1000;

/// Use case: remember what each execution Atlas observed consumed, exactly as the runtime
/// reported it, so it can be summed later. This is Atlas-tracked usage: it says nothing about
/// what the provider's own account shows. Stored in the user config (no secrets in it).
pub struct UsageLedger {
    config: Arc<ConfigRepository>,
}

impl UsageLedger {
    pub fn new(config: Arc<ConfigRepository>) -> Self {
        Self { config }
    }

    /// Adds an observed execution and, if the runtime reported one, its latest quota.
    ///
    /// # Errors
    ///
    /// Fails if saving fails.
    pub fn record(
        &self,
        record: UsageRecord,
        quota: Option<(String, QuotaInfo)>,
    ) -> Result<(), AppError> {
        self.config.modify(|config| {
            let oldest = record.completed_at.saturating_sub(RETENTION_MS);
            config.usage.retain(|r| r.completed_at >= oldest);
            config.usage.push(record);
            if let Some((runtime_id, quota)) = quota {
                config.quotas.insert(runtime_id, quota);
            }
            Ok(())
        })
    }

    /// Drops everything recorded for a workspace (it is being deleted).
    ///
    /// # Errors
    ///
    /// Fails if saving fails.
    pub fn forget_workspace(&self, workspace_id: &str) -> Result<(), AppError> {
        self.config.modify(|config| {
            config.usage.retain(|r| r.workspace_id != workspace_id);
            Ok(())
        })
    }

    pub fn records(&self) -> Vec<UsageRecord> {
        self.config.snapshot().usage
    }

    pub fn quota_of(&self, runtime_id: &str) -> Option<QuotaInfo> {
        self.config.snapshot().quotas.get(runtime_id).cloned()
    }
}

fn totals_since(records: &[&UsageRecord], since: u64) -> UsageTotals {
    UsageTotals::from_records(records.iter().copied().filter(|r| r.completed_at >= since))
}

/// Usage of one agent in one workspace, from the records of that agent only. `conversation_executions`
/// are the executions that have messages in the agent's current conversation.
pub fn summarize_agent(
    records: &[UsageRecord],
    workspace_id: &str,
    agent_id: &str,
    conversation_executions: &HashSet<String>,
    windows: UsageWindows,
    quota: Option<QuotaInfo>,
) -> AgentUsageSummary {
    let mine: Vec<&UsageRecord> = records
        .iter()
        .filter(|r| r.workspace_id == workspace_id && r.agent_id == agent_id)
        .collect();
    let conversation: Vec<&UsageRecord> = mine
        .iter()
        .copied()
        .filter(|r| conversation_executions.contains(&r.execution_id))
        .collect();
    AgentUsageSummary {
        latest_execution: mine
            .iter()
            .max_by_key(|r| r.completed_at)
            .map(|r| (*r).clone()),
        conversation: totals_since(&conversation, 0),
        today: totals_since(&mine, windows.today_start),
        week: totals_since(&mine, windows.week_start),
        month: totals_since(&mine, windows.month_start),
        quota,
    }
}

/// Usage of every agent in one workspace.
pub fn summarize_workspace(
    records: &[UsageRecord],
    workspace_id: &str,
    windows: UsageWindows,
) -> WorkspaceUsageSummary {
    let mine: Vec<&UsageRecord> = records
        .iter()
        .filter(|r| r.workspace_id == workspace_id)
        .collect();
    WorkspaceUsageSummary {
        today: totals_since(&mine, windows.today_start),
        week: totals_since(&mine, windows.week_start),
        month: totals_since(&mine, windows.month_start),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::config::memory::MemoryStore;
    use crate::domain::usage::{UsageMetrics, UsageSource};

    const DAY: u64 = 24 * 60 * 60 * 1000;
    const NOW: u64 = 1_000 * DAY;

    #[allow(clippy::unnecessary_wraps)]
    fn metrics(total: u64, cost: Option<f64>) -> Option<UsageMetrics> {
        Some(UsageMetrics {
            input_tokens: Some(total - 1),
            output_tokens: Some(1),
            total_tokens: Some(total),
            cost,
            currency: cost.map(|_| "USD".to_owned()),
            source: UsageSource::RuntimeReported,
        })
    }

    fn record(
        execution: &str,
        workspace: &str,
        agent: &str,
        completed_at: u64,
        metrics: Option<UsageMetrics>,
    ) -> UsageRecord {
        UsageRecord {
            execution_id: execution.to_owned(),
            workspace_id: workspace.to_owned(),
            agent_id: agent.to_owned(),
            runtime_id: "rt".to_owned(),
            model_id: "m".to_owned(),
            started_at: completed_at - 1,
            completed_at,
            succeeded: true,
            metrics,
        }
    }

    fn windows() -> UsageWindows {
        UsageWindows {
            today_start: NOW - DAY / 2,
            week_start: NOW - 3 * DAY,
            month_start: NOW - 20 * DAY,
        }
    }

    #[test]
    fn an_agents_usage_counts_only_that_agents_executions_in_that_workspace() {
        let records = [
            record("e1", "w1", "a1", NOW - 10, metrics(100, Some(0.5))),
            record("e2", "w1", "a2", NOW - 10, metrics(900, Some(9.0))),
            record("e3", "w2", "a1", NOW - 10, metrics(700, Some(7.0))),
        ];
        let conversation: HashSet<String> = ["e1".to_owned()].into();

        let summary = summarize_agent(&records, "w1", "a1", &conversation, windows(), None);

        assert_eq!(summary.today.total_tokens, Some(100));
        assert_eq!(summary.today.cost, Some(0.5));
        assert_eq!(summary.today.runs, 1);
        assert_eq!(summary.latest_execution.unwrap().execution_id, "e1");
    }

    #[test]
    fn a_workspaces_usage_counts_only_its_own_executions() {
        let records = [
            record("e1", "w1", "a1", NOW - 10, metrics(100, Some(0.5))),
            record("e2", "w1", "a2", NOW - 10, metrics(50, Some(0.25))),
            record("e3", "w2", "a1", NOW - 10, metrics(700, Some(7.0))),
        ];

        let w1 = summarize_workspace(&records, "w1", windows());
        let w2 = summarize_workspace(&records, "w2", windows());

        assert_eq!(
            (w1.today.total_tokens, w1.today.cost),
            (Some(150), Some(0.75))
        );
        assert_eq!(
            (w2.today.total_tokens, w2.today.cost),
            (Some(700), Some(7.0))
        );
        assert_eq!(
            summarize_workspace(&records, "none", windows()).today.runs,
            0
        );
    }

    #[test]
    fn periods_include_only_executions_inside_them() {
        let records = [
            record("today", "w", "a", NOW - 1000, metrics(1, Some(1.0))),
            record(
                "this-week",
                "w",
                "a",
                NOW - 2 * DAY,
                metrics(10, Some(10.0)),
            ),
            record(
                "this-month",
                "w",
                "a",
                NOW - 10 * DAY,
                metrics(100, Some(100.0)),
            ),
            record("old", "w", "a", NOW - 50 * DAY, metrics(1000, Some(1000.0))),
        ];

        let summary = summarize_agent(&records, "w", "a", &HashSet::new(), windows(), None);

        assert_eq!(summary.today.cost, Some(1.0));
        assert_eq!(summary.week.cost, Some(11.0));
        assert_eq!(summary.month.cost, Some(111.0));
        assert_eq!(
            summary.conversation.runs, 0,
            "no execution belongs to the conversation"
        );
        assert_eq!(summary.conversation.cost, None);
    }

    #[test]
    fn the_conversation_is_the_executions_that_have_messages_in_it() {
        let records = [
            record("in", "w", "a", NOW - 50 * DAY, metrics(5, Some(0.5))),
            record("out", "w", "a", NOW - 10, metrics(9, Some(9.0))),
        ];
        let conversation: HashSet<String> = ["in".to_owned()].into();

        let summary = summarize_agent(&records, "w", "a", &conversation, windows(), None);

        assert_eq!(summary.conversation.cost, Some(0.5));
        assert_eq!(summary.conversation.runs, 1);
    }

    #[test]
    fn executions_without_reported_usage_stay_unavailable_not_zero() {
        let records = [record("e", "w", "a", NOW - 10, None)];

        let summary = summarize_agent(&records, "w", "a", &HashSet::new(), windows(), None);

        assert_eq!(summary.today.runs, 1);
        assert_eq!(
            (summary.today.total_tokens, summary.today.cost),
            (None, None)
        );
        assert_eq!(summary.latest_execution.unwrap().metrics, None);
    }

    #[test]
    fn the_ledger_keeps_records_and_the_latest_quota_and_prunes_old_ones() {
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        let ledger = UsageLedger::new(config);
        let quota = QuotaInfo {
            windows: vec![],
            source: UsageSource::ProviderReported,
            observed_at: 5,
        };

        ledger
            .record(record("old", "w", "a", 1, None), None)
            .unwrap();
        ledger
            .record(
                record("new", "w", "a", RETENTION_MS + 10, metrics(1, None)),
                Some(("rt".to_owned(), quota.clone())),
            )
            .unwrap();

        let ids: Vec<_> = ledger
            .records()
            .into_iter()
            .map(|r| r.execution_id)
            .collect();
        assert_eq!(ids, ["new"]);
        assert_eq!(ledger.quota_of("rt"), Some(quota));
        assert_eq!(ledger.quota_of("other"), None);
        ledger.forget_workspace("w").unwrap();
        assert_eq!(ledger.records(), []);
    }
}
