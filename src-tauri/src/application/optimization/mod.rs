//! The Optimization Layer, phase 0: observability only.
//!
//! This module measures what `ExecutionService` already does. It never decides anything: a prompt,
//! a context selection, a runtime request and a permission are the same whether metrics are on or
//! off. Later phases build on the numbers recorded here (see ADR 0020).

#[cfg(test)]
pub mod benchmark;
pub mod context;
pub mod skills;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::time::Instant;

use super::config::ConfigRepository;
use super::runtimes::RuntimeEvent;
use crate::domain::optimization::{
    ContextEngineMetrics, ContextMetrics, HandoffMetrics, LatencyMetrics, OptimizationCounters,
    OptimizationMetrics, PromptBreakdown, RuntimeExtensions, SkillMetrics, ToolMetrics, ToolUse,
};
use crate::domain::task_context::ContextRecord;

/// Which parts of the layer are switched on. Read for every execution, so a change applies to the
/// next one. Switching metrics off removes the measuring and nothing else.
pub trait OptimizationFlags: Send + Sync {
    /// `optimization.metrics.enabled`
    fn metrics_enabled(&self) -> bool;

    /// `optimization.context.enabled`: the Context Engine reworks what a prompt says twice.
    fn context_enabled(&self) -> bool {
        false
    }

    /// `optimization.skills.enabled`: skills are discovered and the ones a task calls for are
    /// sent with it.
    fn skills_enabled(&self) -> bool {
        false
    }

    /// `optimization.context.maxTokens`: a budget for the prompt, in estimated tokens. `None`: no
    /// budget, nothing is ever left out for size.
    fn context_max_tokens(&self) -> Option<u64> {
        None
    }
}

impl OptimizationFlags for ConfigRepository {
    fn metrics_enabled(&self) -> bool {
        self.read(|config| config.settings.optimization.metrics_enabled)
    }

    fn context_enabled(&self) -> bool {
        self.read(|config| config.settings.optimization.context_enabled)
    }

    fn context_max_tokens(&self) -> Option<u64> {
        self.read(|config| config.settings.optimization.context_max_tokens)
    }

    fn skills_enabled(&self) -> bool {
        self.read(|config| config.settings.optimization.skills_enabled)
    }
}

/// A fixed answer, for tests.
#[cfg(test)]
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct FixedFlags {
    pub metrics: bool,
    pub context: bool,
    pub max_tokens: Option<u64>,
    pub skills: bool,
}

#[cfg(test)]
impl FixedFlags {
    pub fn metrics(metrics: bool) -> Self {
        Self {
            metrics,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_skills(mut self) -> Self {
        self.skills = true;
        self
    }

    #[must_use]
    pub fn with_metrics(mut self) -> Self {
        self.metrics = true;
        self
    }

    pub fn context(metrics: bool, max_tokens: Option<u64>) -> Self {
        Self {
            metrics,
            context: true,
            max_tokens,
            skills: false,
        }
    }
}

#[cfg(test)]
impl OptimizationFlags for FixedFlags {
    fn metrics_enabled(&self) -> bool {
        self.metrics
    }

    fn context_enabled(&self) -> bool {
        self.context
    }

    fn context_max_tokens(&self) -> Option<u64> {
        self.max_tokens
    }

    fn skills_enabled(&self) -> bool {
        self.skills
    }
}

/// Milliseconds since `since`, to the microsecond, or `None` when nothing was being timed.
pub fn elapsed_ms(since: Option<Instant>) -> Option<f64> {
    since.map(|start| ms(start.elapsed()))
}

fn ms(duration: std::time::Duration) -> f64 {
    (duration.as_secs_f64() * 1_000_000.0).round() / 1_000.0
}

/// Watches a runtime's progress reports while it runs, without taking part in them: the report is
/// passed on untouched by the caller. Costs two clock reads and a counter.
pub struct RuntimeProbe {
    started: Instant,
    waiting_at: Cell<Option<Instant>>,
    tool_calls: Cell<u32>,
    tool_names: std::cell::RefCell<BTreeMap<String, u32>>,
}

/// What a [`RuntimeProbe`] saw.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeObservation {
    pub startup_ms: Option<f64>,
    pub execution_ms: Option<f64>,
    pub total_ms: f64,
    pub tool_calls: Option<u32>,
    /// Calls per tool, most used first.
    pub tools_used: Vec<(String, u32)>,
}

impl RuntimeProbe {
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
            waiting_at: Cell::new(None),
            tool_calls: Cell::new(0),
            tool_names: std::cell::RefCell::default(),
        }
    }

    pub fn observe(&self, event: &RuntimeEvent) {
        match event {
            RuntimeEvent::Waiting if self.waiting_at.get().is_none() => {
                self.waiting_at.set(Some(Instant::now()));
            }
            RuntimeEvent::ToolStarted(name) => {
                self.tool_calls.set(self.tool_calls.get() + 1);
                *self
                    .tool_names
                    .borrow_mut()
                    .entry(name.clone())
                    .or_default() += 1;
            }
            _ => {}
        }
    }

    pub fn finish(&self) -> RuntimeObservation {
        let now = Instant::now();
        let waiting = self.waiting_at.get();
        let calls = self.tool_calls.get();
        let mut tools_used: Vec<(String, u32)> = self
            .tool_names
            .borrow()
            .iter()
            .map(|(name, count)| (name.clone(), *count))
            .collect();
        tools_used.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        RuntimeObservation {
            startup_ms: waiting.map(|at| ms(at.duration_since(self.started))),
            execution_ms: waiting.map(|at| ms(now.duration_since(at))),
            total_ms: ms(now.duration_since(self.started)),
            // No report is not "no calls": a runtime that does not stream tool events looks
            // exactly like one that used none.
            tool_calls: (calls > 0).then_some(calls),
            tools_used,
        }
    }
}

/// What `ExecutionService` measured on the way to the runtime, before the runtime ran.
pub struct PreRuntime {
    pub prompt: PromptBreakdown,
    pub context: Option<ContextMetrics>,
    pub context_engine: Option<ContextEngineMetrics>,
    pub skills: Option<SkillMetrics>,
    pub handoff_bytes: Option<u64>,
    pub context_build_ms: Option<f64>,
    pub prompt_build_ms: Option<f64>,
    /// Time spent measuring so far.
    pub instrumentation_ms: f64,
}

/// The harness part of the picture, from the record the Task Context service already makes.
pub fn context_metrics(record: Option<&ContextRecord>) -> Option<ContextMetrics> {
    record.map(|record| ContextMetrics {
        selected_items: u32::try_from(record.selected_items).unwrap_or(u32::MAX),
        omitted_items: u32::try_from(record.omitted_items).unwrap_or(u32::MAX),
        selected_characters: record.selected_context_characters as u64,
        total_harness_characters: record.total_harness_characters as u64,
    })
}

/// Puts what was measured before and after the runtime together.
pub fn finish_metrics(
    pre: PreRuntime,
    runtime: Option<&RuntimeObservation>,
    total_ms: Option<f64>,
    instrumentation_ms: f64,
) -> OptimizationMetrics {
    let dropped_items = pre.context.map(|c| c.omitted_items);
    OptimizationMetrics {
        prompt: pre.prompt,
        context: pre.context,
        latency: LatencyMetrics {
            context_build_ms: pre.context_build_ms,
            prompt_build_ms: pre.prompt_build_ms,
            runtime_startup_ms: runtime.and_then(|r| r.startup_ms),
            runtime_execution_ms: runtime.and_then(|r| r.execution_ms),
            runtime_ms: runtime.map(|r| r.total_ms),
            total_ms,
            instrumentation_ms: Some(pre.instrumentation_ms + instrumentation_ms),
        },
        tools: ToolMetrics {
            calls: runtime.and_then(|r| r.tool_calls),
            total_output_bytes: None,
            exposed: None,
            used: runtime
                .as_ref()
                .map(|r| {
                    r.tools_used
                        .iter()
                        .map(|(name, calls)| ToolUse {
                            name: name.clone(),
                            calls: *calls,
                        })
                        .collect()
                })
                .unwrap_or_default(),
        },
        handoff: HandoffMetrics {
            bytes: pre.handoff_bytes,
        },
        optimization: OptimizationCounters {
            cache_hits: pre.skills.as_ref().map(|s| s.cache_hits),
            cache_misses: pre.skills.as_ref().map(|s| s.cache_misses),
            deduplicated_items: pre
                .context_engine
                .as_ref()
                .map_or(0, |c| c.deduplicated_lines),
            compressed_items: pre
                .context_engine
                .as_ref()
                .map_or(0, |c| c.compressed_items),
            dropped_items,
        },
        context_engine: pre.context_engine,
        skills: pre.skills,
        extensions: None,
    }
}

/// What the runtime said about its own surface when it started, taken from the facts it reported
/// (`toolsExposed`, `mcpServers`, `skillsLoaded`, `slashCommands`, `pluginsLoaded`). Nothing is
/// guessed: a runtime that reports none of them gives `None`.
pub fn runtime_reported_surface(
    metadata: &BTreeMap<String, String>,
) -> (Option<Vec<String>>, Option<RuntimeExtensions>) {
    let list = |key: &str| -> Option<Vec<String>> {
        metadata.get(key).map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
    };
    let count = |key: &str| metadata.get(key).and_then(|v| v.parse::<u32>().ok());
    let exposed = list("toolsExposed");
    let reported = [
        "mcpServers",
        "skillsLoaded",
        "slashCommands",
        "pluginsLoaded",
    ]
    .iter()
    .any(|k| metadata.contains_key(*k));
    let extensions = reported.then(|| RuntimeExtensions {
        mcp_servers: list("mcpServers").unwrap_or_default(),
        skills: count("skillsLoaded").unwrap_or(0),
        slash_commands: count("slashCommands").unwrap_or(0),
        plugins: count("pluginsLoaded").unwrap_or(0),
    });
    (exposed, extensions)
}

/// The facts an event carries, as text (events carry `metadata` strings only).
pub fn prompt_event_metadata(
    prompt: &PromptBreakdown,
    build_ms: Option<f64>,
) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::from([
        ("totalBytes".to_owned(), prompt.total_bytes.to_string()),
        (
            "estimatedTokens".to_owned(),
            prompt.estimated_tokens.to_string(),
        ),
        ("tokenSource".to_owned(), "estimated".to_owned()),
    ]);
    if let Some(ms) = build_ms {
        metadata.insert("buildMs".to_owned(), ms.to_string());
    }
    metadata
}
