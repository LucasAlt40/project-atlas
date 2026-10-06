//! Puts the engine in front of the prompt builder. The engine never writes a prompt: it reworks
//! the *inputs* `PromptBuilder` takes (the Harness text and the task text, brief included), and
//! the builder, still the only assembly, builds the prompt from them.

use super::engine::{ContextBudget, ContextEngine, ContextPlan};
use super::item::{ContextItem, Priority};
use crate::application::optimization::skills::SkillBlock;
use crate::application::prompt::Prompt;
use crate::domain::optimization::{
    BriefParts, ContextEngineMetrics, SectionKind, TextSize, TokenSource,
};

const MAX_RECORDED_DECISIONS: usize = 40;

pub struct ContextInputs<'a> {
    /// The prompt as the builder assembled it from the unchanged inputs.
    pub prompt: &'a Prompt,
    /// Its `combined()` text.
    pub combined: &'a str,
    pub agent_instructions: &'a str,
    /// The task text exactly as given to the builder (trimmed).
    pub task_description: &'a str,
    pub brief: Option<&'a BriefParts>,
    /// The skills in the prompt, one block each (as `prompt.skills` is made of them).
    pub skills: &'a [SkillBlock],
    pub budget: ContextBudget,
}

/// What to build the prompt from instead, and what was done.
pub struct ContextOutcome {
    pub harness: Option<String>,
    /// The skills as they will be, block by block.
    pub skill_blocks: Vec<SkillBlock>,
    pub task_description: String,
    /// The brief as it will be, when it was taken apart.
    pub brief_parts: Option<BriefParts>,
    pub changed: bool,
    plan_metrics: ContextEngineMetrics,
}

impl ContextOutcome {
    /// The record of what happened, with the size of the prompt that was finally built.
    pub fn metrics(&self, final_combined: &str) -> ContextEngineMetrics {
        let final_size = TextSize::of(final_combined);
        ContextEngineMetrics {
            final_bytes: final_size.bytes,
            final_estimated_tokens: final_size.estimated_tokens(),
            ..self.plan_metrics.clone()
        }
    }
}

/// What a prompt is made of, as the items the engine and the review work on, in prompt order.
pub(crate) struct PromptItems {
    pub items: Vec<ContextItem>,
    /// The task text before the brief, when the brief could be taken apart exactly.
    pub prefix: Option<String>,
    /// Why the brief was left whole, when it was.
    pub skipped: Option<String>,
}

/// The items of a prompt: the system text, the Harness, the skills, the project, the agent's
/// instructions and the task (taken apart into task, workflow context, handoff and protocols when
/// it is a step's brief). The same list for everything that looks at a prompt, so they all agree
/// on what each part is and how far it can be trusted.
#[allow(clippy::too_many_lines)]
pub(crate) fn prompt_items(
    prompt: &Prompt,
    agent_instructions: &str,
    task_description: &str,
    brief: Option<&BriefParts>,
    skills: &[SkillBlock],
) -> PromptItems {
    let mut items: Vec<ContextItem> = Vec::new();
    items.push(ContextItem::new(
        "system",
        SectionKind::Personality,
        Priority::Required,
        "atlas",
        &prompt.system,
    ));
    let harness_kind = if prompt.task_aware {
        SectionKind::TaskContext
    } else {
        SectionKind::Harness
    };
    if let Some(harness) = &prompt.harness {
        items.push(ContextItem::new(
            if prompt.task_aware {
                "task_context"
            } else {
                "harness"
            },
            harness_kind,
            Priority::High,
            "harness",
            harness,
        ));
    }
    if !skills.is_empty() {
        items.push(ContextItem::new(
            "skills_notice",
            SectionKind::Skills,
            Priority::Required,
            "atlas",
            crate::application::optimization::skills::SkillPlan::notice(),
        ));
        for block in skills {
            // A skill the task asked for by name is relevant context; one Atlas matched is
            // the first thing a budget gives up.
            items.push(ContextItem::new(
                &format!("skill:{}", block.name),
                SectionKind::Skills,
                if block.explicit {
                    Priority::High
                } else {
                    Priority::Normal
                },
                "skills",
                &block.text,
            ));
        }
    }
    items.push(ContextItem::new(
        "project",
        SectionKind::ProjectContext,
        Priority::Required,
        "workspace",
        &prompt.context,
    ));
    if !agent_instructions.trim().is_empty() {
        items.push(ContextItem::new(
            "agent_instructions",
            SectionKind::AgentInstructions,
            Priority::Required,
            "agent",
            agent_instructions.trim(),
        ));
    }

    let description = task_description;
    let mut skipped = None;
    let split = brief.and_then(|brief| {
        let all = format!(
            "{}{}{}",
            brief.workflow_context, brief.handoff, brief.protocols
        );
        let tail = all.trim_end();
        description
            .ends_with(tail)
            .then(|| description[..description.len() - tail.len()].to_owned())
            .map(|prefix| (prefix, brief))
    });
    if brief.is_some() && split.is_none() {
        skipped = Some("the brief does not end the task text as written".to_owned());
    }
    match &split {
        Some((prefix, brief)) => {
            for (id, source, priority, text) in [
                (
                    "task",
                    SectionKind::Task,
                    Priority::Required,
                    prefix.as_str(),
                ),
                (
                    "workflow_context",
                    SectionKind::BriefWorkflowContext,
                    Priority::Required,
                    brief.workflow_context.as_str(),
                ),
                (
                    "handoff",
                    SectionKind::BriefHandoff,
                    Priority::High,
                    brief.handoff.as_str(),
                ),
                (
                    "protocols",
                    SectionKind::BriefProtocols,
                    Priority::Required,
                    brief.protocols.as_str(),
                ),
            ] {
                items.push(ContextItem::new(id, source, priority, "workflow", text));
            }
        }
        None => items.push(ContextItem::new(
            "task",
            SectionKind::Task,
            Priority::Required,
            "user",
            description,
        )),
    }
    PromptItems {
        items,
        prefix: split.map(|(prefix, _)| prefix),
        skipped,
    }
}

/// The task text put back together from its parts: what a step's brief was before it was taken
/// apart, trimmed as the builder trims it.
pub(crate) fn rebuild_task(
    prefix: &str,
    workflow_context: &str,
    handoff: &str,
    protocols: &str,
) -> String {
    [prefix, workflow_context, handoff, protocols]
        .concat()
        .trim_end()
        .to_owned()
}

/// Runs the engine over what the prompt is made of. When the brief cannot be taken apart exactly
/// (its text no longer ends with its parts) the task text is left whole and the metrics say why:
/// the engine would rather do nothing than edit what it cannot place.
#[allow(clippy::too_many_lines)]
pub fn optimize_prompt_inputs(inputs: &ContextInputs<'_>) -> ContextOutcome {
    let prompt = inputs.prompt;
    let description = inputs.task_description;
    let PromptItems {
        items,
        prefix: split,
        skipped,
    } = prompt_items(
        prompt,
        inputs.agent_instructions,
        description,
        inputs.brief,
        inputs.skills,
    );

    let whole = TextSize::of(inputs.combined);
    let items_tokens: u64 = items.iter().map(ContextItem::estimated_tokens).sum();
    // The part of the prompt that is not an item (headings, separators) counts against the budget.
    let overhead = whole.estimated_tokens().saturating_sub(items_tokens);
    let budget = ContextBudget {
        max_tokens: inputs.budget.max_tokens.map(|m| m.saturating_sub(overhead)),
        ..inputs.budget
    };
    let plan = ContextEngine::plan(items, &budget);

    let content_of = |id: &str| {
        plan.items
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.content.clone())
    };
    let harness = if prompt.harness.is_some() {
        content_of(if prompt.task_aware {
            "task_context"
        } else {
            "harness"
        })
    } else {
        None
    };
    let task_description = match &split {
        Some(_) => {
            let rebuilt = [
                content_of("task"),
                content_of("workflow_context"),
                content_of("handoff"),
                content_of("protocols"),
            ]
            .into_iter()
            .flatten()
            .collect::<String>();
            rebuilt.trim_end().to_owned()
        }
        None => description.to_owned(),
    };
    let skill_blocks: Vec<SkillBlock> = inputs
        .skills
        .iter()
        .map(|block| SkillBlock {
            text: content_of(&format!("skill:{}", block.name)).unwrap_or_default(),
            ..block.clone()
        })
        .collect();
    let brief_parts = split.as_ref().map(|_| BriefParts {
        workflow_context: content_of("workflow_context").unwrap_or_default(),
        handoff: content_of("handoff").unwrap_or_default(),
        protocols: content_of("protocols").unwrap_or_default(),
    });
    let changed = plan.changed();
    ContextOutcome {
        harness,
        skill_blocks,
        task_description,
        brief_parts,
        changed,
        plan_metrics: metrics_of(&plan, whole, skipped),
    }
}

fn metrics_of(plan: &ContextPlan, raw: TextSize, skipped: Option<String>) -> ContextEngineMetrics {
    ContextEngineMetrics {
        raw_bytes: raw.bytes,
        final_bytes: raw.bytes,
        raw_estimated_tokens: raw.estimated_tokens(),
        final_estimated_tokens: raw.estimated_tokens(),
        token_source: TokenSource::Estimated,
        deduplicated_lines: plan.deduplicated_lines,
        compressed_items: plan.compressed_items,
        omitted_items: plan.omitted_items,
        decisions: plan
            .decisions
            .iter()
            .take(MAX_RECORDED_DECISIONS)
            .cloned()
            .collect(),
        over_budget: plan.over_budget,
        skipped,
    }
}
