//! Personalities: how an agent behaves. Built-ins travel too (as the user edited them), marked
//! with their id, which means the same on every installation.

use super::super::apply::{Applier, Written};
use super::super::export::Exporter;
use super::super::format::{FormatError, Item, Kind};
use super::super::inspect::{Excerpt, Fact, Inspection, Inspector};
use super::super::spec::{reason_of, ContractSpec, PersonalitySpec};
use super::super::{by_name, decode, Category, Dependency, Described, Existing, Issue, Scope};
use super::{existing_of, invalid};
use crate::application::config::UserConfig;
use crate::application::errors::{AppError, ErrorCode};
use crate::application::personalities::{effective, Fields};
use crate::application::support::new_id;
use crate::domain::personality::{PersonalityProfile, PersonalitySource};
use crate::domain::security::PermissionProfile;

pub struct PersonalityKind;

/// The personality the file describes, held to the rules of the editor.
fn build(
    item: &Item,
    spec: &PersonalitySpec,
    id: String,
    source: PersonalitySource,
    name: &str,
) -> Result<PersonalityProfile, AppError> {
    let fields = Fields::clean(name, &item.description, &spec.instructions, &spec.tags)?;
    let profile = PermissionProfile::parse(&spec.suggested_permission_profile)
        .ok_or_else(|| AppError::new(ErrorCode::PermissionProfileInvalid))?;
    Ok(PersonalityProfile {
        id,
        name: fields.name,
        description: fields.description,
        system_instructions: fields.instructions,
        behavior: spec.behavior.clone(),
        tags: fields.tags,
        source,
        suggested_contract: spec.suggested_contract.to_domain()?,
        suggested_permission_profile: profile.id().to_owned(),
    })
}

impl Category for PersonalityKind {
    fn check(&self, item: &Item, at: &str) -> Result<Vec<Dependency>, FormatError> {
        decode::<PersonalitySpec>(item, at)?;
        Ok(Vec::new())
    }

    fn list(&self, scope: &Scope<'_>) -> Vec<Existing> {
        existing_of(&effective(scope.config), |p| &p.id, |p| &p.name)
    }

    /// A built-in is the same personality on every installation, whatever each has done to it.
    fn existing(&self, scope: &Scope<'_>, item: &Item) -> Option<Existing> {
        let list = self.list(scope);
        if let Ok(PersonalitySpec {
            builtin_id: Some(builtin),
            ..
        }) = decode(item, "")
        {
            if let Some(found) = list.iter().find(|e| e.id == builtin) {
                return Some(found.clone());
            }
        }
        by_name(list, &item.name)
    }

    fn describe(&self, config: &UserConfig, id: &str) -> Option<Described> {
        effective(config)
            .into_iter()
            .find(|p| p.id == id)
            .map(|p| Described {
                builtin_id: (p.source == PersonalitySource::Builtin).then(|| p.id.clone()),
                name: p.name,
                description: p.description,
            })
    }

    fn export(&self, cx: &mut Exporter<'_>, id: &str) -> Result<Item, AppError> {
        let personality = effective(cx.config)
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::new(ErrorCode::PersonalityNotFound))?;
        let spec = PersonalitySpec {
            instructions: personality.system_instructions.clone(),
            behavior: personality.behavior.clone(),
            tags: personality.tags.clone(),
            suggested_contract: ContractSpec::from(&personality.suggested_contract),
            suggested_permission_profile: personality.suggested_permission_profile.clone(),
            builtin_id: (personality.source == PersonalitySource::Builtin)
                .then(|| personality.id.clone()),
        };
        Ok(Item {
            key: String::new(),
            kind: Kind::Personality,
            name: personality.name,
            description: personality.description,
            spec: serde_json::to_value(spec).unwrap_or_default(),
        })
    }

    fn inspect(&self, cx: &Inspector<'_>, item: &Item) -> Inspection {
        let Ok(spec) = decode::<PersonalitySpec>(item, "") else {
            return Inspection::default();
        };
        let mut inspection = Inspection {
            text: Some(Excerpt::of(&spec.instructions)),
            ..Inspection::default()
        };
        if !spec.tags.is_empty() {
            inspection
                .facts
                .push(Fact::new("tags", spec.tags.join(", ")));
        }
        let built = build(
            item,
            &spec,
            String::new(),
            PersonalitySource::Custom,
            &item.name,
        );
        match built {
            Err(error) => inspection
                .issues
                .push(Issue::error("invalid_spec").with("reason", reason_of(&error))),
            Ok(candidate) => {
                inspection.identical = self
                    .existing(&cx.scope(), item)
                    .and_then(|e| effective(cx.config).into_iter().find(|p| p.id == e.id))
                    .is_some_and(|p| {
                        PersonalityProfile {
                            id: String::new(),
                            source: PersonalitySource::Custom,
                            ..p
                        } == PersonalityProfile {
                            id: String::new(),
                            ..candidate
                        }
                    });
            }
        }
        inspection
    }

    fn write(
        &self,
        cx: &mut Applier<'_>,
        config: &mut UserConfig,
        item: &Item,
        replacing: Option<&Existing>,
    ) -> Result<Written, AppError> {
        let spec: PersonalitySpec =
            decode(item, "").map_err(|_| super::super::apply::reject("invalid", &item.name))?;
        let taken: Vec<String> = effective(config)
            .into_iter()
            .filter(|p| replacing.is_none_or(|r| r.id != p.id))
            .map(|p| p.name.to_lowercase())
            .collect();
        let (name, _) = Applier::free_name(
            |candidate| taken.contains(&candidate.trim().to_lowercase()),
            item.name.trim(),
        );
        let personality = match replacing {
            None => build(
                item,
                &spec,
                new_id("custom"),
                PersonalitySource::Custom,
                &name,
            ),
            Some(existing) => {
                let current = effective(config)
                    .into_iter()
                    .find(|p| p.id == existing.id)
                    .ok_or_else(|| AppError::new(ErrorCode::PersonalityNotFound))?;
                build(item, &spec, current.id, current.source, &name)
            }
        }
        .map_err(|error| invalid(item, &error))?;
        let written = Written {
            id: personality.id.clone(),
            name: personality.name.clone(),
            issues: Vec::new(),
            model: None,
        };
        match (replacing, personality.source) {
            (None, _) => config.personalities.push(personality),
            (Some(_), PersonalitySource::Custom) => {
                if let Some(slot) = config
                    .personalities
                    .iter_mut()
                    .find(|p| p.id == personality.id)
                {
                    *slot = personality;
                }
            }
            (Some(_), PersonalitySource::Builtin) => {
                config.builtin_overrides.retain(|p| p.id != personality.id);
                config.builtin_overrides.push(personality);
            }
        }
        let _ = cx;
        Ok(written)
    }
}
