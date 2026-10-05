//! The smallest base for comparing two runs of the same scenario: the figures Atlas keeps about
//! each, side by side. In phase 0 "baseline" and "optimized" are the same code path, so their
//! prompts must be byte-identical; later phases are judged against these rows.

use std::fmt::Write;

use crate::domain::optimization::{OptimizationMetrics, PromptVersusRuntime, SectionKind};
use crate::domain::usage::UsageMetrics;

/// What is compared for one run of a scenario. Runtime figures are `None` when the runtime did
/// not report them; they are never filled in from the estimate.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkRow {
    pub scenario: String,
    pub prompt_bytes: usize,
    pub estimated_prompt_tokens: u64,
    pub runtime_input_tokens: Option<u64>,
    pub runtime_output_tokens: Option<u64>,
    pub runtime_cached_input_tokens: Option<u64>,
    pub total_ms: Option<f64>,
    pub tool_calls: Option<u32>,
    pub handoff_bytes: Option<u64>,
    /// The largest prompt sections, biggest first (`Framing` included), with their bytes.
    pub largest_sections: Vec<(SectionKind, usize)>,
}

impl BenchmarkRow {
    pub fn of(scenario: &str, metrics: &OptimizationMetrics, usage: Option<&UsageMetrics>) -> Self {
        let runtime = PromptVersusRuntime::of(metrics, usage);
        let mut sections: Vec<_> = metrics
            .prompt
            .sections
            .iter()
            .map(|s| (s.section, s.bytes))
            .collect();
        sections.sort_by_key(|section| std::cmp::Reverse(section.1));
        sections.truncate(3);
        Self {
            scenario: scenario.to_owned(),
            prompt_bytes: metrics.prompt.total_bytes,
            estimated_prompt_tokens: runtime.atlas_estimated_prompt_tokens,
            runtime_input_tokens: runtime.runtime_input_tokens,
            runtime_output_tokens: runtime.runtime_output_tokens,
            runtime_cached_input_tokens: runtime.runtime_cached_input_tokens,
            total_ms: metrics.latency.total_ms,
            tool_calls: metrics.tools.calls,
            handoff_bytes: metrics.handoff.bytes,
            largest_sections: sections,
        }
    }
}

/// Two runs of one scenario. The prompt text of each is kept so "identical" is a fact, not a
/// reading of the numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub baseline: BenchmarkRow,
    pub optimized: BenchmarkRow,
    pub prompts_identical: bool,
}

impl Comparison {
    pub fn new(
        baseline: BenchmarkRow,
        baseline_prompt: &str,
        optimized: BenchmarkRow,
        optimized_prompt: &str,
    ) -> Self {
        Self {
            baseline,
            optimized,
            prompts_identical: baseline_prompt.as_bytes() == optimized_prompt.as_bytes(),
        }
    }
}

fn cell<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "n/a".to_owned(), |v| v.to_string())
}

/// A plain-text table of rows, for reports and for reading a test failure.
pub fn render(rows: &[BenchmarkRow]) -> String {
    let mut out = String::from(
        "scenario | prompt bytes | est. tokens | runtime in | runtime out | cached | total ms | tools | handoff bytes | largest sections\n",
    );
    for row in rows {
        let largest = row
            .largest_sections
            .iter()
            .map(|(kind, bytes)| format!("{kind:?}={bytes}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            out,
            "{} | {} | {} | {} | {} | {} | {} | {} | {} | {}",
            row.scenario,
            row.prompt_bytes,
            row.estimated_prompt_tokens,
            cell(row.runtime_input_tokens),
            cell(row.runtime_output_tokens),
            cell(row.runtime_cached_input_tokens),
            cell(row.total_ms),
            cell(row.tool_calls),
            cell(row.handoff_bytes),
            largest
        );
    }
    out
}
