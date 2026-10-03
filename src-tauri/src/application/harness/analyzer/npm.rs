use serde_json::Value;

use crate::application::harness::snapshot::ScanSnapshot;

/// Manifests are looked for at the root and up to this many folders down.
pub const DEPTH: usize = 2;

/// The parsed `package.json` files, shallowest first.
pub fn manifests(s: &ScanSnapshot) -> Vec<(&str, Value)> {
    let mut found: Vec<(&str, Value)> = s
        .files_named("package.json", DEPTH)
        .into_iter()
        .filter_map(|path| {
            let json = serde_json::from_str::<Value>(s.files.get(path)?).ok()?;
            Some((path, json))
        })
        .collect();
    found.sort_by_key(|(path, _)| path.matches('/').count());
    found
}

/// Where `name` is listed in a manifest: the field path and the version spec.
pub fn dependency(json: &Value, name: &str) -> Option<(String, String)> {
    ["dependencies", "devDependencies"]
        .iter()
        .find_map(|section| {
            json[section]
                .get(name)
                .and_then(Value::as_str)
                .map(|spec| (format!("{section}[\"{name}\"]"), spec.to_owned()))
        })
}

/// `^18.2.0` -> `18`; anything that is not a plain version -> `true`.
pub fn major(spec: &str) -> String {
    let trimmed = spec.trim_start_matches(|c: char| !c.is_ascii_digit());
    let major: String = trimmed.chars().take_while(char::is_ascii_digit).collect();
    if major.is_empty() {
        "true".to_owned()
    } else {
        major
    }
}
