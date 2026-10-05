//! Skills (Optimization Layer, phase 2): the open Agent Skills format with progressive
//! disclosure, so a skill is available without being a mandatory part of every prompt.
//!
//! Format: a folder `<name>/SKILL.md` (YAML frontmatter with `name` and `description`, then
//! Markdown instructions), optionally with `references/`, `scripts/` and `assets/`. Atlas adopts it
//! as it is; its own hints live under the format's free-form `metadata` map (`atlas-priority`,
//! `atlas-personalities`, `atlas-stacks`, `atlas-files`), never in a proprietary file.
//!
//! Three levels, each costing tokens only when it is needed:
//!
//! 1. **Discovery**: name, description and metadata. Used to decide, never sent to the model (the
//!    CLI runtimes Atlas drives cannot ask for a skill later, so a list of unloaded skills would
//!    be tokens for nothing).
//! 2. **Activation**: the `SKILL.md` instructions of the few skills the task calls for.
//! 3. **Resources**: `references/` files the task and the skill's own text point at. Scripts and
//!    assets are listed, never inlined.
//!
//! Selection is deterministic (keywords, stack, personality, file types, an explicit request, the
//! skill's own priority) and explainable. Where the skills live: `<project>/.atlas/skills/` and the
//! user's `<app data>/skills/`; the project's wins on a name clash.

mod model;
mod select;
mod service;
mod validate;

pub use model::{RawSkill, ResourceEntry, ResourceKind, SkillStore};
pub use select::SelectionInput;
pub use service::{SkillBlock, SkillPlan, SkillService};

#[cfg(test)]
pub use service::memory;

#[cfg(test)]
mod tests;
