# ADR 0003 — Deny-by-default command permissions

**Status:** accepted

By default, Tauri lets the webview call any `#[tauri::command]`. Atlas instead declares its commands in
`build.rs` (`AppManifest`), which makes each one require an `allow-<name>` permission granted in
`capabilities/`. This keeps the invariant that adding a privileged command is an explicit, reviewable
change, which matters once agents can execute code on the user's machine.
