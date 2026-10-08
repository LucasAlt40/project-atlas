//! Context & Tooling Platform, Phase B: the contract of an execution's context.
//!
//! - [`ExecutionBudget`] / [`ModelLimits`]: what one runtime + model accepts and how much of it a
//!   prompt uses, every figure with its source and precision (`budget`, `figure`).
//! - [`ContextPlan`]: what Atlas meant to assemble, before the guardrails had their say.
//! - [`ContextManifest`]: what Atlas prepared and delivered, with the hash of the payload.
//! - [`RuntimeSurface`]: what else can shape the execution and who controls it (`surface`).
//!
//! Plain data and pure rules. Building them from a running execution lives in
//! `application/optimization`; see `docs/architecture/context-and-tooling-platform.md`.

mod authority;
mod budget;
mod figure;
mod manifest;
mod plan;
mod surface;

pub use authority::ContextAuthority;
pub use budget::{
    defaults, ExecutionBudget, LimitOverride, ModelLimits, OverrideLayer, ReportedLimits,
};
pub use figure::Figure;
pub use manifest::{ContextManifest, DeliveryRecord, ManifestIds, ManifestRule, RuleStatus};
pub use plan::ContextPlan;
pub use surface::{Observation, RuntimeSurface, SurfaceControl, SurfaceEntry, SurfaceKind};

// Named by tests only, outside the module.
#[cfg_attr(not(test), allow(unused_imports))]
pub use figure::{FigureSource, Precision};

use serde::{Deserialize, Serialize};

/// Something a reader of the budget or the manifest should not overlook. A closed set the UI
/// translates; none of them blocks an execution (blocking is the guardrails' job).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextWarning {
    /// Token figures are Atlas's estimate, not a count.
    TokensEstimated,
    /// Nothing states what this runtime + model accepts.
    ModelLimitUnknown,
    /// An override asked for more than a known limit allows and was held to it.
    LimitClamped,
    /// The prompt is estimated above the input limit.
    OverBudget,
    /// The guardrails stopped or paused the step: the prompt was not handed to the runtime.
    NotDelivered,
    /// The guardrails changed the prompt after the plan was made.
    DivergedFromPlan,
    /// Part of what shapes the execution is outside Atlas's observation or control.
    SurfacePartlyUnobserved,
}
