//! Rules (`domain::rules`): where they come from, how they are resolved and how they reach the
//! prompt. They enter the existing pipeline as `ContextItem`s; nothing here builds a prompt, reads
//! a file Atlas does not already read, or decides what an agent may do (that stays in `security/`).

mod parse;
mod render;
mod resolve;
mod service;

pub use render::{notice, RuleBlock};
pub use resolve::{resolve, AppliedRule, ConflictKind, Resolution, RuleConflict, RuleContext};
pub use service::RuleService;
