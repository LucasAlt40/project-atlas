//! What a piece of context may do. The question is not "is it true?" but "may it instruct?".
//!
//! | Authority       | May it change behaviour?                    | Examples                                        |
//! | --------------- | ------------------------------------------- | ----------------------------------------------- |
//! | `Authoritative` | Yes; it binds the agent                     | Atlas's own instructions; a mandatory/preferred Rule |
//! | `Instructional` | Yes; it is what the user asked for           | the task, the agent's own instructions          |
//! | `Informational` | No; the agent may use it as background      | the Harness, the workflow state, a background Rule |
//! | `Untrusted`     | No; read with suspicion and scanned          | skills, another agent's handoff, (later) MCP results and memories |
//!
//! Nothing here is a permission. Authority says how a piece of text is framed and scanned; what the
//! agent may *do* is decided in `security/`, which never reads a prompt. Content never gains
//! authority by being included: an `Informational` or `Untrusted` text that says it is an
//! instruction is a finding, not an upgrade.

use serde::{Deserialize, Serialize};

use crate::domain::optimization::SectionKind;
use crate::domain::rules::{Rule, RuleStrength};

/// When nothing says, text is `Untrusted`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ContextAuthority {
    #[default]
    Untrusted,
    Informational,
    Instructional,
    Authoritative,
}

impl ContextAuthority {
    /// The authority of a prompt section. A Rule's own authority depends on its strength and
    /// origin ([`Self::of_rule`]); the section is the most it can have.
    pub fn of_section(section: SectionKind) -> Self {
        match section {
            SectionKind::Personality
            | SectionKind::AtlasRules
            | SectionKind::LiveNarration
            | SectionKind::PlanRule
            | SectionKind::BriefProtocols
            | SectionKind::Framing
            | SectionKind::Rules => Self::Authoritative,
            SectionKind::AgentInstructions | SectionKind::Task => Self::Instructional,
            SectionKind::ProjectContext
            | SectionKind::Harness
            | SectionKind::TaskContext
            | SectionKind::BriefWorkflowContext => Self::Informational,
            SectionKind::Skills | SectionKind::BriefHandoff => Self::Untrusted,
        }
    }

    /// A Rule binds only when it is Mandatory or a Preference *and* its origin may bind;
    /// otherwise it is background.
    pub fn of_rule(rule: &Rule) -> Self {
        match rule.effective_strength() {
            RuleStrength::Mandatory | RuleStrength::Preference => Self::Authoritative,
            RuleStrength::Informational => Self::Informational,
        }
    }

    /// Whether text of this authority may tell the agent what to do. The Context Review and the
    /// guardrails reason from the sections today; this is the question later sources (memories,
    /// MCP results) will be asked.
    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn may_instruct(self) -> bool {
        matches!(self, Self::Authoritative | Self::Instructional)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::guardrail::SourceTrust;
    use crate::domain::rules::tests::rule;
    use crate::domain::rules::{RuleOrigin, RuleScope};

    const ALL: [SectionKind; 15] = [
        SectionKind::Personality,
        SectionKind::AtlasRules,
        SectionKind::LiveNarration,
        SectionKind::PlanRule,
        SectionKind::Rules,
        SectionKind::Harness,
        SectionKind::TaskContext,
        SectionKind::Skills,
        SectionKind::ProjectContext,
        SectionKind::AgentInstructions,
        SectionKind::BriefWorkflowContext,
        SectionKind::BriefHandoff,
        SectionKind::BriefProtocols,
        SectionKind::Task,
        SectionKind::Framing,
    ];

    #[test]
    fn what_each_part_of_the_prompt_may_do() {
        use ContextAuthority::{Authoritative, Informational, Instructional, Untrusted};
        assert_eq!(
            ContextAuthority::of_section(SectionKind::AtlasRules),
            Authoritative
        );
        assert_eq!(
            ContextAuthority::of_section(SectionKind::Rules),
            Authoritative
        );
        assert_eq!(
            ContextAuthority::of_section(SectionKind::Task),
            Instructional
        );
        assert_eq!(
            ContextAuthority::of_section(SectionKind::AgentInstructions),
            Instructional
        );
        assert_eq!(
            ContextAuthority::of_section(SectionKind::Harness),
            Informational
        );
        assert_eq!(
            ContextAuthority::of_section(SectionKind::BriefWorkflowContext),
            Informational
        );
        assert_eq!(ContextAuthority::of_section(SectionKind::Skills), Untrusted);
        assert_eq!(
            ContextAuthority::of_section(SectionKind::BriefHandoff),
            Untrusted
        );
    }

    #[test]
    fn only_authoritative_and_instructional_text_may_instruct() {
        assert!(ContextAuthority::Authoritative.may_instruct());
        assert!(ContextAuthority::Instructional.may_instruct());
        assert!(!ContextAuthority::Informational.may_instruct());
        assert!(!ContextAuthority::Untrusted.may_instruct());
    }

    #[test]
    fn the_authority_of_a_section_never_contradicts_how_far_it_is_trusted() {
        for section in ALL {
            let authority = ContextAuthority::of_section(section);
            match SourceTrust::of(section) {
                // Atlas's own text is the only authority that is not a Rule.
                SourceTrust::Atlas => {
                    assert_eq!(authority, ContextAuthority::Authoritative, "{section:?}");
                }
                // Text from files and other agents never instructs.
                SourceTrust::Untrusted => assert!(!authority.may_instruct(), "{section:?}"),
                SourceTrust::Configured => {
                    assert!(authority != ContextAuthority::Untrusted, "{section:?}");
                }
            }
        }
    }

    #[test]
    fn a_rule_binds_only_when_its_strength_and_its_origin_allow_it() {
        let mut r = rule("r", RuleScope::Project, RuleStrength::Mandatory, "x");
        assert_eq!(
            ContextAuthority::of_rule(&r),
            ContextAuthority::Authoritative
        );
        r.strength = RuleStrength::Informational;
        assert_eq!(
            ContextAuthority::of_rule(&r),
            ContextAuthority::Informational
        );
        r.strength = RuleStrength::Mandatory;
        r.provenance.origin = RuleOrigin::External;
        assert_eq!(
            ContextAuthority::of_rule(&r),
            ContextAuthority::Informational
        );
    }
}
