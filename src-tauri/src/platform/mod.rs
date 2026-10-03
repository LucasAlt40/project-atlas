//! Host-OS adapters. Anything that differs per operating system lives here, behind
//! traits declared in `application/`, so the rest of the core stays OS-agnostic.

use crate::application::app_info::PlatformInfo;

/// Reports the OS the binary was compiled for (`macos`, `windows`, `linux`, ...).
pub struct OsPlatform;

impl PlatformInfo for OsPlatform {
    fn platform_name(&self) -> String {
        std::env::consts::OS.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_non_empty_platform() {
        assert_ne!(OsPlatform.platform_name(), "");
    }
}
