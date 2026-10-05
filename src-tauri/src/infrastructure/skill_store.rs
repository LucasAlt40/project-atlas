use std::fs;
use std::path::{Component, Path};

use crate::application::optimization::skills::SkillStore;
use crate::application::optimization::skills::{RawSkill, ResourceEntry, ResourceKind};

const SKILL_FILE: &str = "SKILL.md";
const MAX_SKILL_BYTES: u64 = 256 * 1024;
const MAX_RESOURCE_BYTES: u64 = 64 * 1024;
const MAX_RESOURCES: usize = 200;
const MAX_DEPTH: usize = 3;

/// A single normal name: no separators, no `..`, nothing that could leave the folder.
fn plain_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none()
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn is_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

/// Skills on disk: `<base>/<name>/SKILL.md`. Nothing is read through a symlink, and a path that
/// would leave a skill's folder is refused.
pub struct FsSkillStore;

impl FsSkillStore {
    fn skill_file(base: &Path, name: &str) -> Option<std::path::PathBuf> {
        (plain_name(name) && is_dir(&base.join(name))).then(|| base.join(name).join(SKILL_FILE))
    }
}

impl SkillStore for FsSkillStore {
    fn list(&self, base: &Path) -> Vec<String> {
        let Ok(entries) = fs::read_dir(base) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter(|e| {
                is_dir(&e.path())
                    && fs::symlink_metadata(e.path().join(SKILL_FILE)).is_ok_and(|m| m.is_file())
            })
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        names.sort();
        names
    }

    fn fingerprint(&self, base: &Path, name: &str) -> Option<String> {
        let meta = fs::symlink_metadata(Self::skill_file(base, name)?).ok()?;
        if !meta.is_file() {
            return None;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        Some(format!("{}-{modified}", meta.len()))
    }

    fn read(&self, base: &Path, name: &str) -> Option<RawSkill> {
        let path = Self::skill_file(base, name)?;
        let meta = fs::symlink_metadata(&path).ok()?;
        if !meta.is_file() || meta.len() > MAX_SKILL_BYTES {
            return None;
        }
        let text = fs::read_to_string(&path).ok()?;
        Some(RawSkill {
            text,
            fingerprint: self.fingerprint(base, name)?,
        })
    }

    fn resources(&self, base: &Path, name: &str) -> Vec<ResourceEntry> {
        if !plain_name(name) {
            return Vec::new();
        }
        let root = base.join(name);
        let mut found = Vec::new();
        let mut pending = vec![(root.clone(), 0usize)];
        while let Some((dir, depth)) = pending.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                let Ok(meta) = fs::symlink_metadata(&path) else {
                    continue;
                };
                if meta.is_dir() && depth < MAX_DEPTH {
                    pending.push((path, depth + 1));
                } else if meta.is_file() {
                    let Ok(relative) = path.strip_prefix(&root) else {
                        continue;
                    };
                    let relative = relative
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/");
                    if relative == SKILL_FILE {
                        continue;
                    }
                    let kind = match relative.split('/').next() {
                        Some("references") => ResourceKind::Reference,
                        Some("scripts") => ResourceKind::Script,
                        _ => ResourceKind::Other,
                    };
                    found.push(ResourceEntry {
                        path: relative,
                        kind,
                        bytes: meta.len(),
                    });
                }
            }
            if found.len() >= MAX_RESOURCES {
                break;
            }
        }
        found.sort_by(|a, b| a.path.cmp(&b.path));
        found.truncate(MAX_RESOURCES);
        found
    }

    fn read_resource(&self, base: &Path, name: &str, path: &str) -> Option<String> {
        if !plain_name(name) || !safe_relative(path) {
            return None;
        }
        // No folder on the way may be a link either.
        let mut current = base.join(name);
        if let Some(parent) = Path::new(path).parent() {
            for part in parent.components() {
                current.push(part);
                if !is_dir(&current) {
                    return None;
                }
            }
        }
        let file = current.join(Path::new(path).file_name()?);
        let meta = fs::symlink_metadata(&file).ok()?;
        if !meta.is_file() || meta.len() > MAX_RESOURCE_BYTES {
            return None;
        }
        fs::read_to_string(file).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::security::testutil::TempDir;

    fn write(root: &Path, path: &str, text: &str) {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    #[test]
    fn lists_only_folders_with_a_skill_file_and_describes_their_resources() {
        let dir = TempDir::new("skills");
        write(
            dir.path(),
            "billing/SKILL.md",
            "---\nname: billing\n---\nbody",
        );
        write(dir.path(), "billing/references/tax.md", "tax rules");
        write(dir.path(), "billing/scripts/run.sh", "echo hi");
        write(dir.path(), "billing/assets/logo.txt", "x");
        write(dir.path(), "notes/readme.md", "not a skill");

        let store = FsSkillStore;
        assert_eq!(store.list(dir.path()), ["billing"]);
        let resources = store.resources(dir.path(), "billing");
        let kinds: Vec<_> = resources
            .iter()
            .map(|r| (r.path.as_str(), r.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                ("assets/logo.txt", ResourceKind::Other),
                ("references/tax.md", ResourceKind::Reference),
                ("scripts/run.sh", ResourceKind::Script),
            ]
        );
        assert!(store
            .read(dir.path(), "billing")
            .unwrap()
            .text
            .contains("body"));
        assert_eq!(
            store
                .read_resource(dir.path(), "billing", "references/tax.md")
                .as_deref(),
            Some("tax rules")
        );
    }

    #[test]
    fn the_fingerprint_follows_the_file() {
        let dir = TempDir::new("skills-fp");
        write(dir.path(), "a/SKILL.md", "one");
        let store = FsSkillStore;
        let before = store.fingerprint(dir.path(), "a").unwrap();
        write(dir.path(), "a/SKILL.md", "one and more");
        assert_ne!(store.fingerprint(dir.path(), "a").unwrap(), before);
        assert_eq!(store.fingerprint(dir.path(), "missing"), None);
    }

    #[test]
    fn nothing_leaves_the_skills_folder() {
        let dir = TempDir::new("skills-escape");
        write(dir.path(), "a/SKILL.md", "x");
        write(dir.path(), "secret.txt", "private");
        let store = FsSkillStore;

        for path in [
            "../secret.txt",
            "/etc/passwd",
            "references/../../secret.txt",
            "",
        ] {
            assert_eq!(store.read_resource(dir.path(), "a", path), None, "{path}");
        }
        assert_eq!(store.read(dir.path(), "../a"), None);
        assert_eq!(store.fingerprint(dir.path(), "a/../a"), None);
        assert_eq!(store.resources(dir.path(), "..").len(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        use std::os::unix::fs::symlink;
        let dir = TempDir::new("skills-link");
        write(dir.path(), "a/SKILL.md", "x");
        write(dir.path(), "outside/secret.md", "private");
        symlink(dir.path().join("outside"), dir.path().join("a/references")).unwrap();
        symlink(dir.path().join("outside"), dir.path().join("linked")).unwrap();
        let store = FsSkillStore;

        assert_eq!(
            store.read_resource(dir.path(), "a", "references/secret.md"),
            None
        );
        assert_eq!(store.list(dir.path()), ["a"]);
        assert_eq!(store.resources(dir.path(), "a").len(), 0);
    }

    #[test]
    fn a_resource_that_is_not_text_or_is_too_big_is_not_read() {
        let dir = TempDir::new("skills-big");
        write(dir.path(), "a/SKILL.md", "x");
        fs::write(dir.path().join("a/references/bin.md"), [0xff, 0xfe, 0x00]).unwrap_or_else(
            |_| {
                fs::create_dir_all(dir.path().join("a/references")).unwrap();
                fs::write(dir.path().join("a/references/bin.md"), [0xff, 0xfe, 0x00]).unwrap();
            },
        );
        write(
            dir.path(),
            "a/references/big.md",
            &"x".repeat(usize::try_from(MAX_RESOURCE_BYTES).unwrap() + 1),
        );
        let store = FsSkillStore;

        assert_eq!(
            store.read_resource(dir.path(), "a", "references/bin.md"),
            None
        );
        assert_eq!(
            store.read_resource(dir.path(), "a", "references/big.md"),
            None
        );
    }

    #[test]
    fn skills_on_disk_go_through_the_whole_layer() {
        use crate::application::optimization::skills::{SelectionInput, SkillService};
        use std::sync::Arc;

        let project = TempDir::new("skills-project");
        let user = TempDir::new("skills-user");
        write(
            project.path(),
            ".atlas/skills/billing-rules/SKILL.md",
            "---\nname: billing-rules\ndescription: Use when changing invoices, taxes or billing totals in the finance module\n---\nRound every invoice total to two decimals.\nSee [the tax table](references/tax-table.md).\n",
        );
        write(
            project.path(),
            ".atlas/skills/billing-rules/references/tax-table.md",
            "Brazil: 17 percent.\n",
        );
        write(
            user.path(),
            "angular-screens/SKILL.md",
            "---\nname: angular-screens\ndescription: Use when building Angular screens and PrimeNG components for the web app\n---\nUse standalone components.\n",
        );
        let skills = SkillService::new(Arc::new(FsSkillStore), Some(user.path().to_path_buf()));
        let project_path = project.path().to_string_lossy().into_owned();

        let plan = skills.prepare(
            &project_path,
            &SelectionInput {
                task: "Fix the invoice total in billing using the tax table",
                personality_id: "developer",
                technologies: &[],
            },
        );

        assert_eq!(plan.metrics.discovered, 2);
        assert_eq!(plan.metrics.activated, ["billing-rules"]);
        let text = plan.text().unwrap();
        assert!(text.contains("Brazil: 17 percent."));
        assert!(!text.contains("standalone components"));
        // The same call again reads nothing from disk: both skills come from the cache.
        let again = skills.prepare(
            &project_path,
            &SelectionInput {
                task: "Fix the invoice total in billing using the tax table",
                personality_id: "developer",
                technologies: &[],
            },
        );
        assert_eq!(
            (again.metrics.cache_hits, again.metrics.cache_misses),
            (2, 0)
        );
        // Changing one file refreshes only that skill.
        write(
            project.path(),
            ".atlas/skills/billing-rules/SKILL.md",
            "---\nname: billing-rules\ndescription: Use when changing invoices, taxes or billing totals in the finance module\n---\nRound every invoice total to four decimals now.\n",
        );
        let changed = skills.prepare(
            &project_path,
            &SelectionInput {
                task: "Fix the invoice total in billing",
                personality_id: "developer",
                technologies: &[],
            },
        );
        assert_eq!(
            (changed.metrics.cache_hits, changed.metrics.cache_misses),
            (1, 1)
        );
        assert!(changed.text().unwrap().contains("four decimals"));
    }
}
