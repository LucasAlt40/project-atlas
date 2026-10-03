use serde::{Deserialize, Serialize};

/// Where a number came from. Missing data is `None` in the field, never a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    /// Reported by the runtime (its CLI or API) for one execution.
    RuntimeReported,
    /// Atlas added up what runtimes reported for executions Atlas observed.
    AtlasCalculated,
    /// Reported by the provider itself (for example quota windows).
    ProviderReported,
}

/// What one execution consumed. Every field is optional: `None` means the runtime did not
/// report it, which is different from `Some(0)` (it reported zero).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetrics {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cost: Option<f64>,
    /// ISO 4217 code of `cost`, when the runtime says which currency it is.
    pub currency: Option<String>,
    pub source: UsageSource,
}

impl UsageMetrics {
    pub fn has_tokens(&self) -> bool {
        self.input_tokens.is_some() || self.output_tokens.is_some() || self.total_tokens.is_some()
    }
}

/// One usage window of a provider quota (for example `five_hour`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub id: String,
    /// Share of the allowance used, 0.0 to 1.0, as the provider reported it.
    pub used_fraction: f64,
    /// Seconds since the Unix epoch when the window resets.
    pub resets_at: Option<u64>,
}

/// Quota as last reported alongside an execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaInfo {
    pub windows: Vec<QuotaWindow>,
    pub source: UsageSource,
    /// Milliseconds since the Unix epoch when Atlas observed it.
    pub observed_at: u64,
}

/// One observed execution, kept so usage can be summed later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRecord {
    pub execution_id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub runtime_id: String,
    pub model_id: String,
    /// Milliseconds since the Unix epoch.
    pub started_at: u64,
    pub completed_at: u64,
    pub succeeded: bool,
    /// `None`: the runtime reported nothing for this execution.
    pub metrics: Option<UsageMetrics>,
}

/// Sum over observed executions. A field stays `None` unless at least one execution reported
/// it, and the counters say how many executions the numbers are based on, so a partial sum is
/// never mistaken for a complete one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cost: Option<f64>,
    pub currency: Option<String>,
    /// Executions Atlas observed in the period.
    pub runs: u32,
    pub runs_with_tokens: u32,
    pub runs_with_cost: u32,
    pub source: UsageSource,
}

impl UsageTotals {
    pub fn from_records<'a>(records: impl IntoIterator<Item = &'a UsageRecord>) -> Self {
        let mut totals = Self {
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            cost: None,
            currency: None,
            runs: 0,
            runs_with_tokens: 0,
            runs_with_cost: 0,
            source: UsageSource::AtlasCalculated,
        };
        let mut currency_conflict = false;
        let mut currency_seen = false;
        for record in records {
            totals.runs += 1;
            let Some(metrics) = &record.metrics else {
                continue;
            };
            if metrics.has_tokens() {
                totals.runs_with_tokens += 1;
            }
            add(&mut totals.input_tokens, metrics.input_tokens);
            add(&mut totals.output_tokens, metrics.output_tokens);
            add(&mut totals.total_tokens, metrics.total_tokens);
            if let Some(cost) = metrics.cost {
                totals.runs_with_cost += 1;
                totals.cost = Some(totals.cost.unwrap_or(0.0) + cost);
                // A zero in an unstated currency adds nothing, so it says nothing about the
                // currency. Anything else must agree with what has been seen so far.
                let neutral = cost == 0.0 && metrics.currency.is_none();
                if !neutral {
                    if !currency_seen {
                        totals.currency.clone_from(&metrics.currency);
                        currency_seen = true;
                    } else if totals.currency != metrics.currency {
                        currency_conflict = true;
                    }
                }
            }
        }
        if currency_conflict {
            // Amounts in different currencies cannot be added: report none rather than a wrong sum.
            totals.cost = None;
            totals.currency = None;
        }
        totals
    }
}

fn add(sum: &mut Option<u64>, value: Option<u64>) {
    if let Some(value) = value {
        *sum = Some(sum.unwrap_or(0).saturating_add(value));
    }
}

/// Where the period boundaries are, in milliseconds since the Unix epoch. The caller (the UI)
/// knows the user's time zone and calendar, so it decides what "today" and "this week" are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_field_names)]
pub struct UsageWindows {
    pub today_start: u64,
    pub week_start: u64,
    pub month_start: u64,
}

/// Everything the details panel shows for one agent in one workspace.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentUsageSummary {
    /// The agent's latest execution that Atlas observed (`None` if there is none).
    pub latest_execution: Option<UsageRecord>,
    pub conversation: UsageTotals,
    pub today: UsageTotals,
    pub week: UsageTotals,
    pub month: UsageTotals,
    /// Last quota the agent's runtime reported, if it reports any.
    pub quota: Option<QuotaInfo>,
}

/// Usage of every agent in a workspace.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUsageSummary {
    pub today: UsageTotals,
    pub week: UsageTotals,
    pub month: UsageTotals,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(metrics: Option<UsageMetrics>) -> UsageRecord {
        UsageRecord {
            execution_id: "e".to_owned(),
            workspace_id: "w".to_owned(),
            agent_id: "a".to_owned(),
            runtime_id: "r".to_owned(),
            model_id: "m".to_owned(),
            started_at: 1,
            completed_at: 2,
            succeeded: true,
            metrics,
        }
    }

    fn metrics(input: Option<u64>, output: Option<u64>, cost: Option<f64>) -> UsageMetrics {
        UsageMetrics {
            input_tokens: input,
            output_tokens: output,
            total_tokens: input.zip(output).map(|(i, o)| i + o),
            cost,
            currency: cost.map(|_| "USD".to_owned()),
            source: UsageSource::RuntimeReported,
        }
    }

    #[test]
    fn nothing_observed_is_unavailable_not_zero() {
        let totals = UsageTotals::from_records(&[]);

        assert_eq!(
            (totals.input_tokens, totals.cost, totals.runs),
            (None, None, 0)
        );
    }

    #[test]
    fn executions_that_reported_nothing_leave_the_fields_unavailable() {
        let records = [record(None), record(None)];

        let totals = UsageTotals::from_records(&records);

        assert_eq!(totals.runs, 2);
        assert_eq!((totals.total_tokens, totals.cost), (None, None));
        assert_eq!((totals.runs_with_tokens, totals.runs_with_cost), (0, 0));
    }

    #[test]
    fn a_reported_zero_is_zero_not_unavailable() {
        let records = [record(Some(metrics(Some(0), Some(0), Some(0.0))))];

        let totals = UsageTotals::from_records(&records);

        assert_eq!(totals.total_tokens, Some(0));
        assert_eq!(totals.cost, Some(0.0));
        assert_eq!(totals.runs_with_cost, 1);
    }

    #[test]
    fn sums_what_was_reported_and_says_how_many_runs_it_covers() {
        let records = [
            record(Some(metrics(Some(100), Some(10), Some(0.25)))),
            record(Some(metrics(Some(50), Some(5), Some(0.5)))),
            record(None),
        ];

        let totals = UsageTotals::from_records(&records);

        assert_eq!(totals.input_tokens, Some(150));
        assert_eq!(totals.output_tokens, Some(15));
        assert_eq!(totals.total_tokens, Some(165));
        assert_eq!(totals.cost, Some(0.75));
        assert_eq!(totals.currency.as_deref(), Some("USD"));
        assert_eq!(
            (totals.runs, totals.runs_with_tokens, totals.runs_with_cost),
            (3, 2, 2)
        );
        assert_eq!(totals.source, UsageSource::AtlasCalculated);
    }

    #[test]
    fn a_zero_cost_in_an_unstated_currency_does_not_hide_a_real_one() {
        let mut free = metrics(Some(1), Some(1), Some(0.0));
        free.currency = None;
        let records = [
            record(Some(free)),
            record(Some(metrics(Some(1), Some(1), Some(0.04)))),
        ];

        let totals = UsageTotals::from_records(&records);

        assert_eq!(totals.cost, Some(0.04));
        assert_eq!(totals.currency.as_deref(), Some("USD"));
        assert_eq!(totals.runs_with_cost, 2);
    }

    #[test]
    fn a_nonzero_cost_in_an_unstated_currency_cannot_be_added_to_dollars() {
        let mut unknown = metrics(Some(1), Some(1), Some(2.0));
        unknown.currency = None;
        let records = [
            record(Some(metrics(Some(1), Some(1), Some(1.0)))),
            record(Some(unknown)),
        ];

        let totals = UsageTotals::from_records(&records);

        assert_eq!((totals.cost, totals.currency), (None, None));
    }

    #[test]
    fn costs_in_different_currencies_are_not_added() {
        let mut other = metrics(Some(1), Some(1), Some(1.0));
        other.currency = Some("EUR".to_owned());
        let records = [
            record(Some(metrics(Some(1), Some(1), Some(1.0)))),
            record(Some(other)),
        ];

        let totals = UsageTotals::from_records(&records);

        assert_eq!((totals.cost, totals.currency), (None, None));
        assert_eq!(totals.total_tokens, Some(4), "tokens are unaffected");
    }
}
