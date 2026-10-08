use std::sync::Arc;

use super::parse::parse_project_rules;
use crate::application::config::ConfigRepository;
use crate::domain::rules::{Rule, RuleScope};

/// Gathers the rules that might apply to an execution, from the one place each scope lives:
/// the configuration (global, workspace, workflow, agent), the project's rules file, and the task.
/// It resolves nothing (see `resolve`) and writes nothing.
pub struct RuleService {
    config: Arc<ConfigRepository>,
}

impl RuleService {
    pub fn new(config: Arc<ConfigRepository>) -> Self {
        Self { config }
    }

    /// `project_rules` is the text of `.atlas/context/rules.md`, when the project has one;
    /// `task_rules` come with the task. A project or task rule stored in the configuration is
    /// ignored: those scopes have their own source.
    pub fn collect(&self, project_rules: Option<&str>, task_rules: &[Rule]) -> Vec<Rule> {
        let mut rules: Vec<Rule> = self.config.read(|config| {
            config
                .rules
                .iter()
                .filter(|rule| !matches!(rule.scope, RuleScope::Project | RuleScope::Task))
                .cloned()
                .collect()
        });
        if let Some(text) = project_rules {
            rules.extend(parse_project_rules(text));
        }
        rules.extend(task_rules.iter().cloned());
        rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::config::memory::MemoryStore;
    use crate::domain::rules::tests::rule;
    use crate::domain::rules::RuleStrength;

    fn service(stored: Vec<Rule>) -> RuleService {
        let config = Arc::new(ConfigRepository::load(Box::<MemoryStore>::default()));
        config
            .modify(|c| {
                c.rules = stored;
                Ok(())
            })
            .unwrap();
        RuleService::new(config)
    }

    #[test]
    fn each_scope_comes_from_its_own_source() {
        let service = service(vec![
            rule("g", RuleScope::Global, RuleStrength::Preference, "global"),
            rule("a", RuleScope::Agent, RuleStrength::Preference, "agent"),
        ]);
        let task = rule("t", RuleScope::Task, RuleStrength::Preference, "task");

        let rules = service.collect(Some("- [mandatory] Write tests\n"), &[task]);

        let scopes: Vec<_> = rules.iter().map(|r| r.scope).collect();
        assert_eq!(
            scopes,
            [
                RuleScope::Global,
                RuleScope::Agent,
                RuleScope::Project,
                RuleScope::Task
            ]
        );
    }

    #[test]
    fn a_project_or_task_rule_in_the_configuration_is_ignored_so_there_is_one_source_per_scope() {
        let service = service(vec![
            rule(
                "p",
                RuleScope::Project,
                RuleStrength::Mandatory,
                "from config",
            ),
            rule("t", RuleScope::Task, RuleStrength::Mandatory, "from config"),
        ]);

        assert_eq!(service.collect(None, &[]), []);
    }

    #[test]
    fn rules_saved_before_rules_existed_load_as_none() {
        let config: crate::application::config::UserConfig =
            serde_json::from_str(r#"{"personalities":[],"agents":[]}"#).unwrap();

        assert_eq!(config.rules, []);
    }
}
