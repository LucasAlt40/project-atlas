//! [`IdeLauncher`] over the editors' own command-line launchers (and, on macOS, `open -a`).
//!
//! The editors are a fixed list: the program is never taken from outside it. The folder is
//! passed as one argument of a process started without a shell, and must be an absolute path to
//! an existing folder (so it can never read as an option).

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::application::ide::{Ide, IdeError, IdeLauncher};

struct Known {
    id: &'static str,
    name: &'static str,
    /// The macOS application name (`open -a`).
    mac_app: &'static str,
    /// The command-line launcher the editor installs.
    cli: &'static str,
    /// The launcher's file name on Windows.
    windows_cli: &'static str,
}

const KNOWN: &[Known] = &[
    Known {
        id: "vscode",
        name: "Visual Studio Code",
        mac_app: "Visual Studio Code",
        cli: "code",
        windows_cli: "code.cmd",
    },
    Known {
        id: "cursor",
        name: "Cursor",
        mac_app: "Cursor",
        cli: "cursor",
        windows_cli: "cursor.cmd",
    },
    Known {
        id: "intellij",
        name: "IntelliJ IDEA",
        mac_app: "IntelliJ IDEA",
        cli: "idea",
        windows_cli: "idea64.exe",
    },
];

/// How a program is started: a program and its arguments, nothing else.
pub type Spawn = dyn Fn(&OsString, &[OsString]) -> io::Result<()> + Send + Sync;

pub struct SystemIdeLauncher {
    /// Folders searched for launchers (the `PATH`).
    path_dirs: Vec<PathBuf>,
    /// Folders macOS applications live in.
    app_dirs: Vec<PathBuf>,
    macos: bool,
    windows: bool,
    spawn: Box<Spawn>,
}

impl SystemIdeLauncher {
    pub fn system() -> Self {
        let mut app_dirs = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            app_dirs.push(PathBuf::from(home).join("Applications"));
        }
        Self {
            path_dirs: std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).collect())
                .unwrap_or_default(),
            app_dirs,
            macos: cfg!(target_os = "macos"),
            windows: cfg!(windows),
            spawn: Box::new(|program, args| {
                Command::new(program)
                    .args(args)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map(|_| ())
            }),
        }
    }

    #[cfg(test)]
    pub fn with(
        path_dirs: Vec<PathBuf>,
        app_dirs: Vec<PathBuf>,
        macos: bool,
        windows: bool,
        spawn: Box<Spawn>,
    ) -> Self {
        Self {
            path_dirs,
            app_dirs,
            macos,
            windows,
            spawn,
        }
    }

    fn launcher(&self, ide: &Known) -> Option<PathBuf> {
        let name = if self.windows {
            ide.windows_cli
        } else {
            ide.cli
        };
        self.path_dirs
            .iter()
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    }

    fn has_app(&self, ide: &Known) -> bool {
        self.macos
            && self
                .app_dirs
                .iter()
                .any(|dir| dir.join(format!("{}.app", ide.mac_app)).is_dir())
    }

    /// The program and arguments that open `folder`, from the fixed list only.
    fn command(&self, ide: &Known, folder: &Path) -> Option<(OsString, Vec<OsString>)> {
        if self.has_app(ide) {
            return Some((
                OsString::from("open"),
                vec![
                    OsString::from("-a"),
                    OsString::from(ide.mac_app),
                    folder.as_os_str().to_owned(),
                ],
            ));
        }
        self.launcher(ide).map(|program| {
            (
                program.into_os_string(),
                vec![folder.as_os_str().to_owned()],
            )
        })
    }
}

impl IdeLauncher for SystemIdeLauncher {
    fn available(&self) -> Vec<Ide> {
        KNOWN
            .iter()
            .filter(|ide| self.has_app(ide) || self.launcher(ide).is_some())
            .map(|ide| Ide {
                id: ide.id.to_owned(),
                name: ide.name.to_owned(),
            })
            .collect()
    }

    fn open(&self, ide_id: &str, folder: &Path) -> Result<(), IdeError> {
        let ide = KNOWN
            .iter()
            .find(|ide| ide.id == ide_id)
            .ok_or_else(|| IdeError::Unknown(ide_id.to_owned()))?;
        if !folder.is_absolute() || !folder.is_dir() {
            return Err(IdeError::NotAFolder);
        }
        let (program, args) = self
            .command(ide, folder)
            .ok_or_else(|| IdeError::NotInstalled(ide.name.to_owned()))?;
        (self.spawn)(&program, &args).map_err(|e| IdeError::LaunchFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::application::security::testutil::TempDir;

    type Calls = Arc<Mutex<Vec<(OsString, Vec<OsString>)>>>;

    fn launcher(
        path: &Path,
        apps: &Path,
        macos: bool,
        windows: bool,
    ) -> (SystemIdeLauncher, Calls) {
        let calls: Calls = Arc::default();
        let recorded = calls.clone();
        let launcher = SystemIdeLauncher::with(
            vec![path.to_path_buf()],
            vec![apps.to_path_buf()],
            macos,
            windows,
            Box::new(move |program, args| {
                recorded
                    .lock()
                    .unwrap()
                    .push((program.clone(), args.to_vec()));
                Ok(())
            }),
        );
        (launcher, calls)
    }

    fn install_cli(dir: &Path, name: &str) {
        fs::write(dir.join(name), "#!/bin/sh\n").unwrap();
    }

    #[test]
    fn finds_the_editors_that_are_installed_and_only_those() {
        let bin = TempDir::new("ide-bin");
        let apps = TempDir::new("ide-apps");
        install_cli(bin.path(), "cursor");
        let (launcher, _) = launcher(bin.path(), apps.path(), false, false);

        let found: Vec<_> = launcher.available().into_iter().map(|i| i.id).collect();

        assert_eq!(found, ["cursor"]);
    }

    #[test]
    fn opens_the_folder_with_the_editors_own_launcher_as_one_argument_without_a_shell() {
        let bin = TempDir::new("ide-bin");
        let apps = TempDir::new("ide-apps");
        let folder = TempDir::new("a folder; echo pwned && $(id)");
        install_cli(bin.path(), "code");
        let (launcher, calls) = launcher(bin.path(), apps.path(), false, false);

        launcher.open("vscode", folder.path()).unwrap();

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, bin.path().join("code").into_os_string());
        // The hostile-looking folder name is one argument, untouched.
        assert_eq!(calls[0].1, [folder.path().as_os_str().to_owned()]);
    }

    #[test]
    fn on_macos_an_installed_application_is_opened_with_open_dash_a() {
        let bin = TempDir::new("ide-bin");
        let apps = TempDir::new("ide-apps");
        fs::create_dir_all(apps.path().join("Visual Studio Code.app")).unwrap();
        let folder = TempDir::new("project");
        let (launcher, calls) = launcher(bin.path(), apps.path(), true, false);

        assert_eq!(launcher.available().len(), 1);
        launcher.open("vscode", folder.path()).unwrap();

        let calls = calls.lock().unwrap();
        assert_eq!(calls[0].0, OsString::from("open"));
        assert_eq!(calls[0].1[0], OsString::from("-a"));
        assert_eq!(calls[0].1[1], OsString::from("Visual Studio Code"));
    }

    #[test]
    fn on_windows_the_launcher_has_the_extension_windows_gives_it() {
        let bin = TempDir::new("ide-bin");
        let apps = TempDir::new("ide-apps");
        install_cli(bin.path(), "code.cmd");
        let (launcher, _) = launcher(bin.path(), apps.path(), false, true);

        assert_eq!(launcher.available().len(), 1);
    }

    #[test]
    fn refuses_anything_that_is_not_a_known_editor_or_an_absolute_existing_folder() {
        let bin = TempDir::new("ide-bin");
        let apps = TempDir::new("ide-apps");
        install_cli(bin.path(), "code");
        let folder = TempDir::new("project");
        let (launcher, calls) = launcher(bin.path(), apps.path(), false, false);

        for ide in ["rm -rf /", "code; id", "../code", "", "VSCODE", "sh"] {
            assert_eq!(
                launcher.open(ide, folder.path()),
                Err(IdeError::Unknown(ide.to_owned())),
                "{ide:?}"
            );
        }
        for bad in [
            "relative/folder",
            "-n",
            "--new-window",
            "/definitely/not/there",
        ] {
            assert_eq!(
                launcher.open("vscode", Path::new(bad)),
                Err(IdeError::NotAFolder),
                "{bad:?}"
            );
        }
        // A file is not a folder either.
        let file = folder.path().join("f.txt");
        fs::write(&file, "x").unwrap();
        assert_eq!(launcher.open("vscode", &file), Err(IdeError::NotAFolder));
        assert!(calls.lock().unwrap().is_empty(), "nothing was started");
    }

    #[test]
    fn says_so_when_the_editor_is_not_installed_or_cannot_start() {
        let bin = TempDir::new("ide-bin");
        let apps = TempDir::new("ide-apps");
        let folder = TempDir::new("project");
        let (missing, _) = launcher(bin.path(), apps.path(), false, false);
        assert_eq!(
            missing.open("cursor", folder.path()),
            Err(IdeError::NotInstalled("Cursor".to_owned()))
        );

        install_cli(bin.path(), "cursor");
        let broken = SystemIdeLauncher::with(
            vec![bin.path().to_path_buf()],
            vec![],
            false,
            false,
            Box::new(|_, _| Err(io::Error::other("no"))),
        );
        assert_eq!(
            broken.open("cursor", folder.path()),
            Err(IdeError::LaunchFailed("no".to_owned()))
        );
    }
}
