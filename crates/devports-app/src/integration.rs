//! What the application does for the person's machine: put the `devports`
//! command on their PATH and start at login. Both are explicit menu actions,
//! never done silently (they change things outside the app).

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Label of the launch agent; also its file name.
pub const AGENT_LABEL: &str = "com.davidcardenas.devports";

/// Why an integration step failed.
#[derive(Debug, thiserror::Error)]
pub enum IntegrationError {
    /// There is no home folder.
    #[error("HOME is not set")]
    NoHome,
    /// The command is not inside the application bundle (a bare `cargo run`).
    #[error("el comando devports no está en el paquete: abre DevPorts.app, no el binario suelto")]
    NotBundled,
    /// A file operation failed.
    #[error("{path}: {reason}")]
    Io {
        /// The path involved.
        path: String,
        /// What the system said.
        reason: String,
    },
    /// `launchctl` refused.
    #[error("launchctl: {0}")]
    Launchctl(String),
    /// Something else is where the command should be linked.
    #[error("{0} ya existe y no es un enlace de DevPorts; no se toca")]
    Occupied(String),
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> IntegrationError + '_ {
    move |error| IntegrationError::Io {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}

fn home() -> Result<PathBuf, IntegrationError> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(IntegrationError::NoHome)
}

/// Where the command is linked.
#[must_use]
pub fn command_link(home: &Path) -> PathBuf {
    home.join(".local").join("bin").join("devports")
}

/// The command inside the bundle this process runs from.
fn bundled_command() -> Result<PathBuf, IntegrationError> {
    let exe = std::env::current_exe().map_err(|error| IntegrationError::Io {
        path: "current_exe".to_owned(),
        reason: error.to_string(),
    })?;
    let command = exe
        .parent()
        .and_then(Path::parent)
        .map(|contents| contents.join("Resources").join("bin").join("devports"))
        .filter(|path| path.is_file())
        .ok_or(IntegrationError::NotBundled)?;
    Ok(command)
}

/// Links `devports` into `~/.local/bin`. Returns the link's path.
///
/// # Errors
///
/// See [`IntegrationError`]. An existing link of ours is replaced (the app may
/// have moved); anything else at that path is left alone.
pub fn install_command() -> Result<PathBuf, IntegrationError> {
    link_command(&home()?, &bundled_command()?)
}

fn link_command(home: &Path, command: &Path) -> Result<PathBuf, IntegrationError> {
    let link = command_link(home);
    if let Some(folder) = link.parent() {
        std::fs::create_dir_all(folder).map_err(io(folder))?;
    }
    match std::fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            std::fs::remove_file(&link).map_err(io(&link))?;
        }
        Ok(_) => return Err(IntegrationError::Occupied(link.display().to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io(&link)(error)),
    }
    std::os::unix::fs::symlink(command, &link).map_err(io(&link))?;
    Ok(link)
}

/// Whether `folder` is one of the directories the shell searches.
#[must_use]
pub fn on_path(folder: &Path) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|entry| entry == folder))
}

fn agent_path(home: &Path) -> PathBuf {
    home.join("Library")
        .join("LaunchAgents")
        .join(format!("{AGENT_LABEL}.plist"))
}

/// The launch agent: start at login and come back after a crash, but not after
/// a normal quit (`SuccessfulExit: false`).
#[must_use]
pub fn agent_plist(executable: &Path) -> String {
    let exe = executable
        .display()
        .to_string()
        .replace('&', "&amp;")
        .replace('<', "&lt;");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{AGENT_LABEL}</string>
  <key>ProgramArguments</key><array><string>{exe}</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
  <key>ThrottleInterval</key><integer>5</integer>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
"#
    )
}

/// Whether the launch agent file exists.
#[must_use]
pub fn autostart_enabled() -> bool {
    home().is_ok_and(|home| agent_path(&home).exists())
}

/// Turns start-at-login on or off.
///
/// # Errors
///
/// See [`IntegrationError`].
pub fn set_autostart(enabled: bool) -> Result<(), IntegrationError> {
    let home = home()?;
    let path = agent_path(&home);
    let uid = std::fs::metadata(&home).map_err(io(&home))?.uid();
    let domain = format!("gui/{uid}");
    if enabled {
        let exe = std::env::current_exe().map_err(|error| IntegrationError::Io {
            path: "current_exe".to_owned(),
            reason: error.to_string(),
        })?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(io(folder))?;
        }
        std::fs::write(&path, agent_plist(&exe)).map_err(io(&path))?;
        // Loading it now would start a second copy; that copy sees the first
        // one's socket and exits at once without being restarted.
        launchctl(&["bootstrap", &domain, &path.display().to_string()])
    } else {
        // Not loaded is fine: the goal is that it is not.
        let _ = launchctl(&["bootout", &format!("{domain}/{AGENT_LABEL}")]);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io(&path)(error)),
        }
    }
}

#[allow(
    clippy::disallowed_methods,
    reason = "launchctl is the only interface to launchd; this module is the app's one place that calls it"
)]
fn launchctl(arguments: &[&str]) -> Result<(), IntegrationError> {
    let output = Command::new("launchctl")
        .args(arguments)
        .output()
        .map_err(|error| IntegrationError::Launchctl(error.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(IntegrationError::Launchctl(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

/// Shows a message in a system dialog (a menu-bar app has no other window).
#[allow(
    clippy::disallowed_methods,
    reason = "osascript is the way to show a native dialog without a webview"
)]
pub fn tell(message: &str) {
    let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "display dialog \"{escaped}\" with title \"DevPorts\" buttons {{\"OK\"}} default button \"OK\""
    );
    let _ = Command::new("osascript").args(["-e", &script]).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_restarts_after_a_crash_but_not_after_a_normal_quit() {
        let plist = agent_plist(Path::new(
            "/Applications/DevPorts.app/Contents/MacOS/DevPorts",
        ));
        assert!(plist.contains("<key>SuccessfulExit</key><false/>"));
        assert!(plist.contains("<key>RunAtLoad</key><true/>"));
        assert!(plist.contains("<key>ThrottleInterval</key><integer>5</integer>"));
        assert!(plist.contains("/Applications/DevPorts.app/Contents/MacOS/DevPorts"));
        assert!(plist.contains(AGENT_LABEL));
    }

    #[test]
    fn paths_are_escaped_in_the_plist() {
        let plist = agent_plist(Path::new("/Apps/A&B <x>/DevPorts"));
        assert!(plist.contains("/Apps/A&amp;B &lt;x>/DevPorts"));
    }

    #[test]
    fn the_command_link_is_replaced_when_it_is_ours_and_respected_when_it_is_not() {
        let dir = tempfile::TempDir::new().unwrap();
        let home = dir.path();
        let first = home.join("one");
        let second = home.join("two");
        std::fs::write(&first, "").unwrap();
        std::fs::write(&second, "").unwrap();

        let link = link_command(home, &first).unwrap();
        assert_eq!(std::fs::read_link(&link).unwrap(), first);
        link_command(home, &second).unwrap();
        assert_eq!(std::fs::read_link(&link).unwrap(), second);

        std::fs::remove_file(&link).unwrap();
        std::fs::write(&link, "someone else's file").unwrap();
        assert!(matches!(
            link_command(home, &first),
            Err(IntegrationError::Occupied(_))
        ));
        assert_eq!(
            std::fs::read_to_string(&link).unwrap(),
            "someone else's file"
        );
    }
}
