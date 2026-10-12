//! Mobile access (ADR 0034): the desktop's way to switch the phone server on and off, see its
//! address and QR, and revoke devices. Nothing here takes an address from the webview that the
//! computer does not itself have, and nothing returns a secret except the pairing code the person
//! is meant to scan.

use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime, State};

use super::events::TauriWorkflowObserver;
use crate::application::errors::AppError;
use crate::application::remote::{Asset, AssetSource, RemoteFailure, RemoteStatus};
use crate::infrastructure::{start_remote_server, ServerDeps};
use crate::state::AppState;

/// The mobile page's files: the ones Tauri embedded with the app (the second Vite entry,
/// `mobile.html`). Same build as the desktop, so the two can never disagree.
struct TauriAssets<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> AssetSource for TauriAssets<R> {
    fn get(&self, path: &str) -> Option<Asset> {
        #[cfg(debug_assertions)]
        if let Some(asset) = dev_folder(path) {
            return Some(asset);
        }
        let resolver = self.app.asset_resolver();
        let found = resolver.get(path.to_owned())?;
        // Tauri answers a path it does not know with the desktop's `index.html`, so it can be a
        // single-page app. That must never reach a phone: it is the desktop UI.
        if path != "index.html" {
            let desktop = resolver.get("index.html".to_owned());
            if desktop.is_some_and(|d| d.bytes == found.bytes) {
                return None;
            }
        }
        Some(Asset {
            bytes: found.bytes,
            content_type: found.mime_type,
        })
    }
}

/// For development: `ATLAS_MOBILE_DIST=<path to dist>` serves the built page from a folder, since
/// `tauri dev` embeds nothing. Debug builds only.
#[cfg(debug_assertions)]
fn dev_folder(path: &str) -> Option<Asset> {
    let root = std::path::PathBuf::from(std::env::var_os("ATLAS_MOBILE_DIST")?);
    let relative = std::path::Path::new(path);
    if relative
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return None;
    }
    let bytes = std::fs::read(root.join(relative)).ok()?;
    let content_type = match relative.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    };
    Some(Asset {
        bytes,
        content_type: content_type.to_owned(),
    })
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_remote_status(state: State<'_, AppState>) -> RemoteStatus {
    state.remote.status()
}

/// Switches the server on (at `address`, which must be one the computer has; the recommended one
/// when none is given) or off. Switching off ends every session and connection and leaves the
/// workflow runs as they were. A server that could not start is reported in the status.
///
/// # Errors
///
/// Never fails: what went wrong is `status.error`.
#[tauri::command]
pub async fn set_remote_enabled<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    enabled: bool,
    address: Option<String>,
) -> Result<RemoteStatus, AppError> {
    let access = state.remote.clone();
    if !enabled {
        access.stop();
        return Ok(access.status());
    }
    match access.begin_start(address.as_deref()) {
        Ok(ip) => {
            let deps = ServerDeps {
                access: access.clone(),
                // A remote answer is announced to the desktop like a local one.
                observer: Arc::new(TauriWorkflowObserver { app: app.clone() }),
                assets: Arc::new(TauriAssets { app }),
            };
            match start_remote_server(ip, deps) {
                Ok(started) => access.mark_active(ip, started.port, started.stop),
                Err(failure) => access.mark_failed(failure),
            }
        }
        Err(RemoteFailure::AlreadyRunning) => {}
        Err(failure) => access.mark_failed(failure),
    }
    Ok(access.status())
}

/// A new pairing code. The one before stops working.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn new_remote_pairing_code(state: State<'_, AppState>) -> RemoteStatus {
    // Not running is not an error here: the status says so.
    let _ = state.remote.new_pairing_code();
    state.remote.status()
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn revoke_remote_device(state: State<'_, AppState>, device_id: String) -> RemoteStatus {
    state.remote.revoke_device(&device_id);
    state.remote.status()
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn revoke_all_remote_devices(state: State<'_, AppState>) -> RemoteStatus {
    state.remote.revoke_all_devices();
    state.remote.status()
}

/// Whatever way Atlas ends, the phone server ends with it.
pub fn stop_on_exit<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<AppState>() {
        state.remote.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, MockRuntime};

    /// The phone's page comes out of the app's own embedded files. Those exist only in a build that
    /// embeds the frontend (`tauri build`, or `--features tauri/custom-protocol` with `dist/`
    /// built); a plain test build has none, and then there is nothing to check.
    #[test]
    fn the_phone_page_comes_from_the_embedded_frontend_and_never_the_desktop_page() {
        let app = mock_builder()
            .build(crate::context::<MockRuntime>())
            .expect("the app builds");
        let assets = TauriAssets {
            app: app.handle().clone(),
        };
        if app.asset_resolver().get("mobile.html".to_owned()).is_none() {
            eprintln!("no embedded frontend in this build: nothing to check");
            return;
        }

        let page = assets.get("mobile.html").expect("the phone page");
        let html = String::from_utf8(page.bytes).unwrap();
        assert!(
            page.content_type.starts_with("text/html"),
            "{}",
            page.content_type
        );
        assert!(
            html.contains("/assets/mobile-"),
            "it is the phone page: {html}"
        );
        assert!(
            !html.contains("src=\"/assets/main-"),
            "not the desktop's entry"
        );

        // What the page loads is served; what is not there is not answered with the desktop page.
        let script = html
            .split("src=\"")
            .filter_map(|part| part.split('"').next())
            .find(|src| src.starts_with("/assets/mobile-") && src.contains(".js"))
            .expect("the page's script");
        let script = assets
            .get(script.trim_start_matches('/'))
            .expect("its script");
        assert!(
            script.content_type.contains("javascript"),
            "{}",
            script.content_type
        );
        assert!(assets.get("assets/does-not-exist.js").is_none());
    }
}
