// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// WebKitGTK renders a blank window on many Linux setups (NVIDIA, Wayland, VMs) when its
/// DMABUF renderer or accelerated compositing is active. Disable them unless the user opted in.
#[cfg(target_os = "linux")]
fn apply_linux_webkit_workarounds() {
    for key in [
        "WEBKIT_DISABLE_DMABUF_RENDERER",
        "WEBKIT_DISABLE_COMPOSITING_MODE",
    ] {
        if std::env::var_os(key).is_none() {
            std::env::set_var(key, "1");
        }
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    apply_linux_webkit_workarounds();
    atlas_lib::run();
}
