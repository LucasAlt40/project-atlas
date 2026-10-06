//! The Context Engine (Optimization Layer, phase 1): decides what of the context a prompt would
//! carry is worth its tokens.
//!
//! It does not select context: the Harness's `TaskContextService` already does that, and the
//! engine takes its result as an input. What it adds is the part nobody did: noticing what the
//! prompt says twice, keeping a budget honest, and recording every change so nothing is lost
//! silently. Rules the engine follows (see ADR 0021):
//!
//! - **Required context is never touched.** Safety rules, the user's request and instructions, the
//!   step's own task and protocols, and the workflow's minimum context are
//!   [`Priority::Required`]: they can be *compared against*, never edited, dropped or truncated.
//! - **Nothing is truncated.** Duplicates are removed line by line (and only when the same words
//!   stay somewhere else in the prompt); whole items are omitted only for a budget, only below
//!   `High`, and the prompt then names what was left out. If that is not enough the engine reports
//!   an overrun; it does not cut.
//! - **Deterministic and cheap**: fingerprints, normalized text and word overlap. No model, no
//!   embeddings.

mod apply;
mod engine;
mod item;
pub(crate) mod text;

pub use apply::{optimize_prompt_inputs, ContextInputs};
pub(crate) use apply::{prompt_items, rebuild_task};
pub use engine::ContextBudget;
pub use item::{ContextItem, Priority};
