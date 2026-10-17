//! Where the socket lives. Both the core and the CLI compute it, so the rule
//! lives here, in the one crate they share.

use std::path::{Path, PathBuf};

/// Longest socket path macOS accepts (`sockaddr_un.sun_path` is 104 bytes, one
/// of them the terminator), with a few bytes of margin.
pub const MAX_SOCKET_PATH_BYTES: usize = 100;

/// The folder name under Application Support and under the temporary directory.
const APP_DIR: &str = "devports";
/// The socket file name.
const SOCKET_FILE: &str = "devports.sock";

/// Chooses the socket path.
///
/// 1. `override_path` (`DEVPORTS_SOCKET`), whatever it is;
/// 2. `~/Library/Application Support/devports/devports.sock`, if it fits in
///    [`MAX_SOCKET_PATH_BYTES`];
/// 3. otherwise `<tmpdir>/devports/devports.sock`. macOS gives each user a
///    private `TMPDIR`, so the fallback is still the user's own.
///
/// `None` only when there is no home and no temporary directory to build from.
#[must_use]
pub fn socket_path(
    override_path: Option<&Path>,
    home: Option<&Path>,
    tmpdir: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(path) = override_path {
        return Some(path.to_path_buf());
    }
    let preferred = home.map(|home| {
        home.join("Library")
            .join("Application Support")
            .join(APP_DIR)
            .join(SOCKET_FILE)
    });
    match preferred {
        Some(path) if path.as_os_str().len() <= MAX_SOCKET_PATH_BYTES => Some(path),
        other => tmpdir
            .map(|tmp| tmp.join(APP_DIR).join(SOCKET_FILE))
            .or(other),
    }
}

/// [`socket_path`] using the process environment (`DEVPORTS_SOCKET`, `HOME`,
/// `TMPDIR`).
#[must_use]
pub fn socket_path_from_env() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    socket_path(
        var("DEVPORTS_SOCKET").map(PathBuf::from).as_deref(),
        var("HOME").map(PathBuf::from).as_deref(),
        var("TMPDIR").map(PathBuf::from).as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_under_application_support() {
        let path =
            socket_path(None, Some(Path::new("/Users/me")), Some(Path::new("/tmp"))).unwrap();
        assert_eq!(
            path,
            Path::new("/Users/me/Library/Application Support/devports/devports.sock")
        );
    }

    #[test]
    fn an_override_wins_over_everything() {
        let path = socket_path(
            Some(Path::new("/custom/x.sock")),
            Some(Path::new("/Users/me")),
            None,
        )
        .unwrap();
        assert_eq!(path, Path::new("/custom/x.sock"));
    }

    #[test]
    fn a_home_that_makes_the_path_too_long_falls_back_to_the_temporary_directory() {
        let long_home = format!("/Users/{}", "a".repeat(60));
        let path = socket_path(
            None,
            Some(Path::new(&long_home)),
            Some(Path::new("/var/folders/ab/cd/T")),
        )
        .unwrap();
        assert_eq!(
            path,
            Path::new("/var/folders/ab/cd/T/devports/devports.sock")
        );
        assert!(path.as_os_str().len() <= MAX_SOCKET_PATH_BYTES);
    }

    #[test]
    fn a_long_home_without_a_temporary_directory_still_gets_a_path() {
        let long_home = format!("/Users/{}", "a".repeat(80));
        assert!(socket_path(None, Some(Path::new(&long_home)), None).is_some());
    }

    #[test]
    fn without_a_home_the_temporary_directory_is_used() {
        let path = socket_path(None, None, Some(Path::new("/tmp"))).unwrap();
        assert_eq!(path, Path::new("/tmp/devports/devports.sock"));
        assert!(socket_path(None, None, None).is_none());
    }

    #[test]
    fn a_typical_macos_path_fits() {
        let home = Path::new("/Users/davidcardenas");
        let path = socket_path(None, Some(home), None).unwrap();
        assert!(
            path.as_os_str().len() <= MAX_SOCKET_PATH_BYTES,
            "{}",
            path.display()
        );
    }
}
