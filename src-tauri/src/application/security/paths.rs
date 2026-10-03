//! Path containment: "is this path inside the project folder?".
//!
//! Two layers, so the rules are testable on every OS:
//!
//! - [`NormalizedPath`] is a *lexical* model of a path in either [`PathFlavor`] (POSIX or
//!   Windows). It understands separators, drive letters, UNC and verbatim prefixes, `.` and `..`,
//!   and Windows quirks (case-insensitivity, trailing dots/spaces). Containment is compared
//!   component by component, never as a string prefix, so `/project/foo` is not inside
//!   `/project/foobar`.
//! - [`resolve_physical`] asks the real file system: it follows symlinks (also in parts of the
//!   path that do not exist yet, and also before a `..`) and returns the real location, which
//!   is what must be compared with the project folder.
//!
//! Limits (also in ADR 0006): the check happens at one instant, so a symlink swapped in
//! afterwards is not seen (no OS sandbox yet); hard links cannot be detected; Windows short
//! (8.3) names of paths that do not exist yet cannot be expanded.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathFlavor {
    Posix,
    Windows,
}

impl PathFlavor {
    pub const fn host() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Posix
        }
    }

    fn is_separator(self, c: char) -> bool {
        c == '/' || (self == Self::Windows && c == '\\')
    }
}

/// A path reduced to its meaning: where it starts and which names follow, with `.` and `..`
/// resolved. Two paths naming the same place compare equal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedPath {
    flavor: PathFlavor,
    /// Drive (`c:`), UNC share (`//server/share`) — lowercased on Windows. `None` for POSIX
    /// and for relative paths.
    prefix: Option<String>,
    absolute: bool,
    components: Vec<String>,
    /// The path cannot be pinned down without knowing the process state (a Windows path
    /// relative to a drive's current directory, a device path…). Never contained in anything.
    ambiguous: bool,
}

impl NormalizedPath {
    pub fn parse(raw: &str, flavor: PathFlavor) -> Self {
        let mut rest = raw;
        let mut prefix = None;
        let mut ambiguous = false;
        let mut absolute = rest.starts_with(|c| flavor.is_separator(c));

        if flavor == PathFlavor::Windows {
            // `\\?\C:\x` and `\\?\UNC\server\share\x` are the long forms of ordinary paths.
            if let Some(stripped) = rest
                .strip_prefix(r"\\?\")
                .or_else(|| rest.strip_prefix("//?/"))
            {
                rest = stripped;
                if let Some(unc) = strip_prefix_ignore_case(rest, "UNC\\")
                    .or_else(|| strip_prefix_ignore_case(rest, "UNC/"))
                {
                    let normalized = unc.replace('\\', "/");
                    let mut parts = normalized.splitn(3, '/');
                    let (server, share) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                    prefix = Some(format!(
                        "//{}/{}",
                        server.to_lowercase(),
                        share.to_lowercase()
                    ));
                    absolute = true;
                    rest = parts.next().unwrap_or("");
                    return Self::finish(flavor, prefix, absolute, ambiguous, rest);
                }
                absolute = false;
            } else if rest.starts_with(r"\\.\") || rest.starts_with("//./") {
                // Device namespace (`\\.\PhysicalDrive0`, `\\.\pipe\…`): not a file path.
                return Self {
                    flavor,
                    prefix: None,
                    absolute: true,
                    components: Vec::new(),
                    ambiguous: true,
                };
            } else if rest.starts_with(r"\\") || rest.starts_with("//") {
                let normalized = rest.replace('\\', "/");
                let mut parts = normalized.trim_start_matches('/').splitn(3, '/');
                let (server, share) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                prefix = Some(format!(
                    "//{}/{}",
                    server.to_lowercase(),
                    share.to_lowercase()
                ));
                return Self::finish(flavor, prefix, true, ambiguous, parts.next().unwrap_or(""));
            }
            let bytes = rest.as_bytes();
            if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                prefix = Some(rest[..2].to_lowercase());
                rest = &rest[2..];
                absolute = rest.starts_with(|c| flavor.is_separator(c));
                // `C:foo` is relative to the current directory *of drive C*.
                ambiguous = !absolute;
            } else if absolute {
                // `\foo`: the current drive, which is not known here.
                ambiguous = true;
            }
        }
        Self::finish(flavor, prefix, absolute, ambiguous, rest)
    }

    fn finish(
        flavor: PathFlavor,
        prefix: Option<String>,
        absolute: bool,
        ambiguous: bool,
        rest: &str,
    ) -> Self {
        let mut components: Vec<String> = Vec::new();
        let mut leading_parents = 0usize;
        for part in rest.split(|c| flavor.is_separator(c)) {
            let part = if flavor == PathFlavor::Windows {
                // Windows ignores trailing dots and spaces: `proj. ` is `proj`.
                let trimmed = part.trim_end_matches(['.', ' ']);
                if part == ".." || part == "." {
                    part
                } else {
                    trimmed
                }
            } else {
                part
            };
            match part {
                "" | "." => {}
                ".." => {
                    if components.pop().is_none() && !absolute {
                        leading_parents += 1;
                    }
                    // `..` at the root of an absolute path stays at the root.
                }
                name => components.push(if flavor == PathFlavor::Windows {
                    name.to_lowercase()
                } else {
                    name.to_owned()
                }),
            }
        }
        // Relative paths that climb above their start keep that fact as leading `..`.
        let mut all = vec!["..".to_owned(); leading_parents];
        all.extend(components);
        Self {
            flavor,
            prefix,
            absolute,
            components: all,
            ambiguous,
        }
    }

    /// Is `self` the same place as `root` or somewhere below it?
    pub fn starts_with(&self, root: &Self) -> bool {
        if self.ambiguous || root.ambiguous || !self.absolute || !root.absolute {
            return false;
        }
        if self.flavor != root.flavor || self.prefix != root.prefix {
            return false;
        }
        self.components.len() >= root.components.len()
            && self.components[..root.components.len()] == root.components[..]
    }
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &text[prefix.len()..])
}

/// Purely lexical containment: `candidate` is absolute, or relative to `base`. Does not touch
/// the file system, so it cannot see symlinks: use [`is_inside_physical`] for real decisions.
#[cfg(test)]
pub fn lexically_inside(root: &str, candidate: &str, base: &str, flavor: PathFlavor) -> bool {
    let root = NormalizedPath::parse(root, flavor);
    let parsed = NormalizedPath::parse(candidate, flavor);
    if parsed.absolute {
        return parsed.starts_with(&root);
    }
    NormalizedPath::parse(&format!("{base}/{candidate}"), flavor).starts_with(&root)
}

#[derive(Debug)]
pub enum PathError {
    /// A symbolic link that points nowhere (or loops): writing through it could create a
    /// file anywhere, so it is refused rather than guessed at.
    BrokenLink,
    Io,
}

/// Where `path` really is, following symlinks everywhere (including before a `..` and in the
/// parts that do not exist yet) and normalizing case and short names on file systems that need
/// it. `path` may be relative to `base`.
///
/// # Errors
///
/// Fails on a dangling or looping symlink and on I/O errors other than "not found".
pub fn resolve_physical(path: &Path, base: &Path) -> Result<PathBuf, PathError> {
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    let mut current = PathBuf::new();
    for component in full.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => {}
            // `current` is already a real location, so stepping up is physically correct.
            Component::ParentDir => {
                current.pop();
            }
            Component::Normal(name) => {
                current.push(name);
                match fs::symlink_metadata(&current) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        current = fs::canonicalize(&current).map_err(|_| PathError::BrokenLink)?;
                    }
                    Ok(_) => {}
                    // Nothing exists here (or below), so no symlink can hide in the rest.
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                        ) => {}
                    Err(_) => return Err(PathError::Io),
                }
            }
        }
    }
    canonicalize_existing_part(&current).map_err(|_| PathError::Io)
}

/// Canonicalizes the longest existing ancestor and re-attaches the rest, so a path that does
/// not exist yet (a file about to be created) still resolves.
fn canonicalize_existing_part(path: &Path) -> io::Result<PathBuf> {
    let mut tail = Vec::new();
    let mut probe = path.to_path_buf();
    loop {
        match fs::canonicalize(&probe) {
            Ok(mut real) => {
                real.extend(tail.iter().rev());
                return Ok(real);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let Some(name) = probe.file_name().map(ToOwned::to_owned) else {
                    return Err(e);
                };
                tail.push(name);
                if !probe.pop() {
                    return Err(e);
                }
            }
            Err(e) => return Err(e),
        }
    }
}

/// Is `candidate` (resolved against `base`) physically inside `root`?
///
/// # Errors
///
/// Fails if either path cannot be resolved (see [`resolve_physical`]); callers must treat that
/// as "not inside".
pub fn is_inside_physical(root: &Path, candidate: &Path, base: &Path) -> Result<bool, PathError> {
    let root = resolve_physical(root, base)?;
    let candidate = resolve_physical(candidate, base)?;
    let flavor = PathFlavor::host();
    Ok(NormalizedPath::parse(&candidate.to_string_lossy(), flavor)
        .starts_with(&NormalizedPath::parse(&root.to_string_lossy(), flavor)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: PathFlavor = PathFlavor::Posix;
    const W: PathFlavor = PathFlavor::Windows;

    fn inside(flavor: PathFlavor, root: &str, candidate: &str) -> bool {
        NormalizedPath::parse(candidate, flavor).starts_with(&NormalizedPath::parse(root, flavor))
    }

    #[test]
    fn the_project_and_everything_below_it_is_inside() {
        let root = "/Users/lucas/dev/lontano";
        for ok in [
            "/Users/lucas/dev/lontano",
            "/Users/lucas/dev/lontano/",
            "/Users/lucas/dev/lontano/src",
            "/Users/lucas/dev/lontano/tests/a/b.rs",
            "/Users/lucas/dev/lontano/./src//x",
        ] {
            assert!(inside(P, root, ok), "{ok}");
        }
    }

    #[test]
    fn everything_else_is_outside() {
        let root = "/Users/lucas/dev/lontano";
        for bad in [
            "/Users/lucas/Documents",
            "/Users/lucas/.ssh",
            "/Users/lucas/.aws/credentials",
            "/Users/lucas/Library",
            "/Users/lucas/Desktop",
            "/Users/lucas/dev/other-project",
            "/Users/lucas/dev",
            "/",
        ] {
            assert!(!inside(P, root, bad), "{bad}");
        }
    }

    #[test]
    fn a_longer_sibling_name_is_not_inside_a_shorter_root() {
        assert!(!inside(P, "/project/foo", "/project/foobar"));
        assert!(!inside(P, "/project/foobar", "/project/foo"));
        assert!(!inside(P, "/project/foo", "/project/foo-other/x"));
        assert!(!inside(W, r"C:\project\foo", r"C:\project\foobar\x"));
    }

    #[test]
    fn dot_dot_is_resolved_before_comparing() {
        let root = "/work/proj";
        assert!(inside(P, root, "/work/proj/src/../lib"));
        assert!(!inside(P, root, "/work/proj/../other"));
        assert!(!inside(P, root, "/work/proj/src/../../secrets"));
        // Cannot climb out through the filesystem root either.
        assert!(!inside(P, root, "/../../etc"));
        assert!(inside(P, "/", "/../../etc"));
    }

    #[test]
    fn relative_paths_are_resolved_against_the_base() {
        assert!(lexically_inside(
            "/work/proj",
            "src/main.rs",
            "/work/proj",
            P
        ));
        assert!(lexically_inside("/work/proj", "./a/../b", "/work/proj", P));
        assert!(!lexically_inside("/work/proj", "../x", "/work/proj", P));
        assert!(!lexically_inside(
            "/work/proj",
            "../../etc/passwd",
            "/work/proj",
            P
        ));
        assert!(lexically_inside(
            "/work/proj",
            "/work/proj/x",
            "/elsewhere",
            P
        ));
        assert!(!lexically_inside(
            "/work/proj",
            "/etc/passwd",
            "/work/proj",
            P
        ));
    }

    #[test]
    fn posix_paths_are_case_sensitive_and_backslash_is_a_name_character() {
        assert!(!inside(P, "/work/Proj", "/work/proj/x"));
        assert!(inside(P, "/work/proj", r"/work/proj/a\b"));
        assert!(!inside(P, "/work/proj", r"/work/proj\..\..\etc"));
        assert!(inside(P, "/work/proj", r"/work/proj/..\x"));
    }

    #[test]
    fn windows_paths_accept_both_separators_and_ignore_case() {
        let root = r"C:\Users\Lucas\dev\Lontano";
        assert!(inside(W, root, r"C:\Users\Lucas\dev\Lontano\src"));
        assert!(inside(W, root, "c:/users/lucas/DEV/lontano/src/main.rs"));
        assert!(inside(W, root, r"C:\Users\Lucas\dev\Lontano\a\..\b"));
        assert!(!inside(W, root, r"C:\Users\Lucas\dev\Lontano\..\other"));
        assert!(!inside(W, root, r"C:\Users\Lucas\Documents"));
        assert!(!inside(W, root, r"D:\Users\Lucas\dev\Lontano"));
    }

    #[test]
    fn windows_long_prefixes_and_trailing_dots_do_not_hide_the_location() {
        let root = r"C:\proj";
        assert!(inside(W, root, r"\\?\C:\proj\src"));
        assert!(inside(W, r"\\?\C:\proj", r"C:\proj\src"));
        assert!(!inside(W, root, r"\\?\C:\other"));
        // `proj. ` and `proj` are the same folder to Windows.
        assert!(inside(W, root, r"C:\proj. \src"));
        assert!(!inside(W, root, r"C:\proj.other"));
    }

    #[test]
    fn windows_paths_that_depend_on_process_state_are_never_inside() {
        let root = r"C:\proj";
        for ambiguous in [
            r"C:src",
            r"\proj\src",
            r"\\.\C:\proj",
            r"\\.\PhysicalDrive0",
        ] {
            assert!(!inside(W, root, ambiguous), "{ambiguous}");
        }
    }

    #[test]
    fn windows_unc_shares_are_compared_by_server_and_share() {
        let root = r"\\server\share\proj";
        assert!(inside(W, root, r"\\SERVER\Share\proj\x"));
        assert!(inside(W, root, r"\\?\UNC\server\share\proj\x"));
        assert!(!inside(W, root, r"\\server\other\proj\x"));
        assert!(!inside(W, root, r"\\server\share\proj2"));
    }

    #[test]
    fn a_relative_path_is_never_inside_an_absolute_root() {
        assert!(!inside(P, "/work/proj", "src/main.rs"));
        assert!(!inside(P, "/work/proj", "../proj"));
    }

    #[cfg(unix)]
    mod physical {
        use std::os::unix::fs::symlink;

        use super::*;
        use crate::application::security::testutil::TempDir;

        struct Tree {
            _dir: TempDir,
            project: PathBuf,
            outside: PathBuf,
        }

        /// `<tmp>/project/{src/,link-out -> ../outside, link-in -> src, dangling -> nowhere}`
        /// and `<tmp>/outside/secret.txt`.
        fn tree() -> Tree {
            let dir = TempDir::new("paths");
            let project = dir.path().join("project");
            let outside = dir.path().join("outside");
            fs::create_dir_all(project.join("src")).unwrap();
            fs::create_dir_all(&outside).unwrap();
            fs::write(outside.join("secret.txt"), "s").unwrap();
            symlink("../outside", project.join("link-out")).unwrap();
            symlink("src", project.join("link-in")).unwrap();
            symlink("nowhere", project.join("dangling")).unwrap();
            Tree {
                _dir: dir,
                project,
                outside,
            }
        }

        fn check(tree: &Tree, candidate: &str) -> bool {
            is_inside_physical(&tree.project, Path::new(candidate), &tree.project).unwrap_or(false)
        }

        #[test]
        fn plain_paths_inside_the_project_are_inside() {
            let t = tree();
            assert!(check(&t, "src"));
            assert!(check(&t, "src/main.rs"));
            assert!(check(&t, "./src/../src"));
            assert!(check(&t, "."));
        }

        #[test]
        fn paths_that_do_not_exist_yet_are_judged_by_where_they_would_be() {
            let t = tree();
            assert!(check(&t, "src/new/deeper/file.rs"));
            assert!(!check(&t, "../new-file"));
            assert!(!check(&t, "../outside/new-file"));
        }

        #[test]
        fn a_symlink_out_of_the_project_does_not_bypass_the_boundary() {
            let t = tree();
            assert!(!check(&t, "link-out"));
            assert!(!check(&t, "link-out/secret.txt"));
            // Even a file that does not exist yet behind the link.
            assert!(!check(&t, "link-out/new.txt"));
            assert!(!check(&t, &t.outside.join("secret.txt").to_string_lossy()));
        }

        #[test]
        fn a_symlink_that_stays_inside_the_project_is_fine() {
            let t = tree();
            assert!(check(&t, "link-in"));
            assert!(check(&t, "link-in/file.rs"));
        }

        #[test]
        fn dot_dot_after_a_symlink_follows_the_real_location() {
            let t = tree();
            // `link-out` is `<tmp>/outside`, so `..` from it is `<tmp>`: outside the project,
            // although lexically `project/link-out/..` is the project itself.
            assert!(!check(&t, "link-out/.."));
            assert!(lexically_inside(
                &t.project.to_string_lossy(),
                "link-out/..",
                &t.project.to_string_lossy(),
                PathFlavor::Posix
            ));
        }

        #[test]
        fn a_dangling_symlink_is_refused_not_guessed_at() {
            let t = tree();
            assert!(matches!(
                resolve_physical(Path::new("dangling/new.txt"), &t.project),
                Err(PathError::BrokenLink)
            ));
            assert!(!check(&t, "dangling"));
        }

        #[test]
        fn an_absolute_path_outside_is_outside() {
            let t = tree();
            assert!(!check(&t, "/etc/passwd"));
            assert!(!check(&t, "/"));
        }
    }
}
