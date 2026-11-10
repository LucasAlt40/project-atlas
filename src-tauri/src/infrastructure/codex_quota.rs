//! The Codex CLI's account limits, read from its own session logs. `codex exec --json` does not
//! report them, but every session it records (`$CODEX_HOME/sessions/YYYY/MM/DD/rollout-*.jsonl`,
//! `~/.codex` by default) carries `token_count` events with the account's `rate_limits`. Only that
//! object is read, from the newest sessions; nothing is written and no model is called.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::domain::usage::{QuotaInfo, QuotaOrigin, QuotaWindow, UsageSource};

/// Newest session files looked at before giving up (a session may have no limits event).
const MAX_FILES: usize = 20;

/// Where the Codex CLI keeps its data: `CODEX_HOME`, else `~/.codex`.
pub fn codex_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CODEX_HOME").map(PathBuf::from) {
        return home.is_absolute().then_some(home);
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .map(|home| home.join(".codex"))
}

/// The limits in the newest Codex session that recorded them, or `None`.
pub fn latest_codex_quota(codex_home: &Path) -> Option<QuotaInfo> {
    newest_sessions(&codex_home.join("sessions"))
        .into_iter()
        .take(MAX_FILES)
        .find_map(|file| quota_in(&file))
}

/// Session files, newest first: the folders are dates and the names start with the time.
fn newest_sessions(root: &Path) -> Vec<PathBuf> {
    fn sorted(dir: &Path) -> Vec<PathBuf> {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .map(|d| d.filter_map(Result::ok).map(|e| e.path()).collect())
            .unwrap_or_default();
        entries.sort();
        entries.reverse();
        entries
    }
    let mut files = Vec::new();
    for year in sorted(root).into_iter().filter(|p| p.is_dir()) {
        for month in sorted(&year).into_iter().filter(|p| p.is_dir()) {
            for day in sorted(&month).into_iter().filter(|p| p.is_dir()) {
                files.extend(sorted(&day).into_iter().filter(|p| {
                    p.extension().is_some_and(|e| e == "jsonl")
                        && p.file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with("rollout-"))
                }));
                if files.len() >= MAX_FILES {
                    return files;
                }
            }
        }
    }
    files
}

/// The last `rate_limits` of one session file.
fn quota_in(file: &Path) -> Option<QuotaInfo> {
    let reader = BufReader::new(fs::File::open(file).ok()?);
    let mut last: Option<Value> = None;
    for line in reader.lines().map_while(Result::ok) {
        if !line.contains("\"rate_limits\"") {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            let limits = &value["payload"]["rate_limits"];
            if limits.is_object() {
                last = Some(limits.clone());
            }
        }
    }
    let limits = last?;
    let windows: Vec<QuotaWindow> = ["primary", "secondary"]
        .iter()
        .filter_map(|key| {
            let window = &limits[*key];
            Some(QuotaWindow {
                id: window_id(window["window_minutes"].as_u64()?),
                used_fraction: window["used_percent"].as_f64()? / 100.0,
                resets_at: window["resets_at"].as_u64(),
            })
        })
        .collect();
    if windows.is_empty() {
        return None;
    }
    let observed_at = fs::metadata(file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    Some(QuotaInfo {
        windows,
        source: UsageSource::ProviderReported,
        observed_at,
        origin: QuotaOrigin::CliSession,
        plan: limits["plan_type"].as_str().map(str::to_owned),
    })
}

/// The same names Claude uses for its windows, so the screen labels both alike.
fn window_id(minutes: u64) -> String {
    match minutes {
        300 => "five_hour".to_owned(),
        10_080 => "seven_day".to_owned(),
        43_200 => "thirty_day".to_owned(),
        m if m % 1_440 == 0 => format!("{}_day", m / 1_440),
        m if m % 60 == 0 => format!("{}_hour", m / 60),
        m => format!("{m}_minute"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::security::testutil::TempDir;

    fn session(home: &Path, day: &str, name: &str, lines: &[&str]) {
        let dir = home.join("sessions").join(day);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), lines.join("\n")).unwrap();
    }

    const LIMITS: &str = r#"{"timestamp":"2026-10-31T00:30:20Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"primary":{"used_percent":50.0,"window_minutes":43200,"resets_at":1794871665},"secondary":{"used_percent":12.5,"window_minutes":300,"resets_at":1794000000},"plan_type":"free"}}}"#;

    #[test]
    fn reads_the_limits_of_the_newest_session_that_has_them() {
        let home = TempDir::new("codex-home");
        session(
            home.path(),
            "2026/10/30",
            "rollout-2026-10-30T21-00-00-a.jsonl",
            &[LIMITS],
        );
        // A newer session without a limits event does not hide the older reading.
        session(
            home.path(),
            "2026/11/02",
            "rollout-2026-11-02T09-00-00-b.jsonl",
            &[r#"{"type":"session_meta","payload":{}}"#],
        );

        let quota = latest_codex_quota(home.path()).unwrap();

        assert_eq!(quota.origin, QuotaOrigin::CliSession);
        assert_eq!(quota.plan.as_deref(), Some("free"));
        assert_eq!(quota.windows.len(), 2);
        assert_eq!(quota.windows[0].id, "thirty_day");
        assert!((quota.windows[0].used_fraction - 0.5).abs() < 1e-9);
        assert_eq!(quota.windows[0].resets_at, Some(1_794_871_665));
        assert_eq!(quota.windows[1].id, "five_hour");
    }

    #[test]
    fn the_last_reading_in_a_session_wins() {
        let home = TempDir::new("codex-home");
        let later = LIMITS.replace("50.0", "75.0");
        session(
            home.path(),
            "2026/10/30",
            "rollout-x.jsonl",
            &[LIMITS, &later],
        );

        let quota = latest_codex_quota(home.path()).unwrap();

        assert!((quota.windows[0].used_fraction - 0.75).abs() < 1e-9);
    }

    #[test]
    fn nothing_is_made_up_without_sessions_or_limits() {
        let home = TempDir::new("codex-home");
        assert!(latest_codex_quota(home.path()).is_none());
        session(
            home.path(),
            "2026/10/30",
            "rollout-x.jsonl",
            &["not json", "{}"],
        );
        assert!(latest_codex_quota(home.path()).is_none());
    }
}
