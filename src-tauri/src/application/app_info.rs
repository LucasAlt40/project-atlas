use std::sync::Arc;

use crate::domain::app_info::AppInfo;

const APP_NAME: &str = "Project Atlas";

/// Port: how the application learns about the host operating system.
/// Implemented in `platform/`; faked in tests.
pub trait PlatformInfo: Send + Sync {
    fn platform_name(&self) -> String;
}

/// Use case: describe the running application.
pub struct AppInfoService {
    version: String,
    platform: Arc<dyn PlatformInfo>,
}

impl AppInfoService {
    pub fn new(version: String, platform: Arc<dyn PlatformInfo>) -> Self {
        Self { version, platform }
    }

    pub fn get_app_info(&self) -> AppInfo {
        AppInfo {
            name: APP_NAME.to_owned(),
            version: self.version.clone(),
            platform: self.platform.platform_name(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakePlatform;

    impl PlatformInfo for FakePlatform {
        fn platform_name(&self) -> String {
            "testos".to_owned()
        }
    }

    #[test]
    fn describes_the_application() {
        let service = AppInfoService::new("1.2.3".to_owned(), Arc::new(FakePlatform));

        assert_eq!(
            service.get_app_info(),
            AppInfo {
                name: "Project Atlas".to_owned(),
                version: "1.2.3".to_owned(),
                platform: "testos".to_owned(),
            }
        );
    }
}
