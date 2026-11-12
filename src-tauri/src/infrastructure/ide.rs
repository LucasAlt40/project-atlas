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

/// Visual Studio (Windows only). It installs no launcher on the `PATH`: its `devenv.exe` is found
/// in its own install folders, `Microsoft Visual Studio\<version>\<edition>\Common7\IDE`, under
/// the Program Files folders. It opens a folder given as its one argument ("Open Folder").
const VISUAL_STUDIO: (&str, &str) = ("visualstudio", "Visual Studio");

/// How a program is started: a program and its arguments, nothing else.
pub type Spawn = dyn Fn(&OsString, &[OsString]) -> io::Result<()> + Send + Sync;

pub struct SystemIdeLauncher {
    /// Folders searched for launchers (the `PATH`).
    path_dirs: Vec<PathBuf>,
    /// Folders macOS applications live in.
    app_dirs: Vec<PathBuf>,
    macos: bool,
    windows: bool,
    /// The Program Files folders Visual Studio is installed under (Windows).
    program_files: Vec<PathBuf>,
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
            program_files: ["ProgramFiles", "ProgramFiles(x86)"]
                .iter()
                .filter_map(std::env::var_os)
                .map(PathBuf::from)
                .collect(),
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
            program_files: Vec::new(),
            spawn,
        }
    }

    /// Where to look for Visual Studio, for tests.
    #[cfg(test)]
    #[must_use]
    pub fn with_program_files(mut self, dirs: Vec<PathBuf>) -> Self {
        self.program_files = dirs;
        self
    }

    /// The newest Visual Studio's `devenv.exe`, on Windows only.
    fn devenv(&self) -> Option<PathBuf> {
        if !self.windows {
            return None;
        }
        let sorted = |dir: &Path| -> Vec<PathBuf> {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
                .map(|read| read.filter_map(Result::ok).map(|e| e.path()).collect())
                .unwrap_or_default();
            // Newest version first ("2022" after "2019", "18" read as a number).
            entries.sort_by_key(|p| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (name.parse::<u32>().unwrap_or(0), name)
            });
            entries.reverse();
            entries
        };
        self.program_files
            .iter()
            .flat_map(|root| sorted(&root.join("Microsoft Visual Studio")))
            .flat_map(|version| sorted(&version))
            .map(|edition| edition.join("Common7").join("IDE").join("devenv.exe"))
            .find(|devenv| devenv.is_file())
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
        let mut found: Vec<Ide> = KNOWN
            .iter()
            .filter(|ide| self.has_app(ide) || self.launcher(ide).is_some())
            .map(|ide| Ide {
                id: ide.id.to_owned(),
                name: ide.name.to_owned(),
            })
            .collect();
        if self.devenv().is_some() {
            found.push(Ide {
                id: VISUAL_STUDIO.0.to_owned(),
                name: VISUAL_STUDIO.1.to_owned(),
            });
        }
        found
    }

    fn open(&self, ide_id: &str, folder: &Path) -> Result<(), IdeError> {
        if ide_id == VISUAL_STUDIO.0 {
            if !folder.is_absolute() || !folder.is_dir() {
                return Err(IdeError::NotAFolder);
            }
            let devenv = self
                .devenv()
                .ok_or_else(|| IdeError::NotInstalled(VISUAL_STUDIO.1.to_owned()))?;
            return (self.spawn)(&devenv.into_os_string(), &[folder.as_os_str().to_owned()])
                .map_err(|e| IdeError::LaunchFailed(e.to_string()));
        }
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

    fn install_visual_studio(program_files: &Path, version: &str) -> PathBuf {
        let ide = program_files
            .join("Microsoft Visual Studio")
            .join(version)
            .join("Community")
            .join("Common7")
            .join("IDE");
        fs::create_dir_all(&ide).unwrap();
        fs::write(ide.join("devenv.exe"), "").unwrap();
        ide.join("devenv.exe")
    }

    #[test]
    fn on_windows_visual_studio_is_found_in_its_install_folder_and_opens_the_folder() {
        let bin = TempDir::new("ide-bin");
        let apps = TempDir::new("ide-apps");
        let program_files = TempDir::new("Program Files");
        let folder = TempDir::new("project");
        install_visual_studio(program_files.path(), "2019");
        let newest = install_visual_studio(program_files.path(), "2022");
        let (windows, calls) = launcher(bin.path(), apps.path(), false, true);
        let windows = windows.with_program_files(vec![program_files.path().to_path_buf()]);

        let found: Vec<_> = windows.available().into_iter().map(|i| i.id).collect();
        assert_eq!(found, ["visualstudio"]);
        windows.open("visualstudio", folder.path()).unwrap();

        let calls = calls.lock().unwrap();
        assert_eq!(calls[0].0, newest.into_os_string());
        assert_eq!(calls[0].1, [folder.path().as_os_str().to_owned()]);
        drop(calls);
        assert_eq!(
            windows.open("visualstudio", Path::new("relative")),
            Err(IdeError::NotAFolder)
        );
    }

    #[test]
    fn visual_studio_is_never_offered_off_windows_or_when_it_is_not_installed() {
        let bin = TempDir::new("ide-bin");
        let apps = TempDir::new("ide-apps");
        let program_files = TempDir::new("Program Files");
        let folder = TempDir::new("project");
        let (empty, _) = launcher(bin.path(), apps.path(), false, true);
        let empty = empty.with_program_files(vec![program_files.path().to_path_buf()]);
        assert!(empty.available().iter().all(|i| i.id != "visualstudio"));
        assert_eq!(
            empty.open("visualstudio", folder.path()),
            Err(IdeError::NotInstalled("Visual Studio".to_owned()))
        );

        install_visual_studio(program_files.path(), "2022");
        let (mac, _) = launcher(bin.path(), apps.path(), true, false);
        let mac = mac.with_program_files(vec![program_files.path().to_path_buf()]);
        assert!(mac.available().iter().all(|i| i.id != "visualstudio"));
    }
}
