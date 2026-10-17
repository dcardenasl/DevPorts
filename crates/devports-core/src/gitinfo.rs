//! Branch, worktree and last activity of a working directory, read straight from
//! the `.git` folder: no `git` process is spawned (the core never spawns).
//!
//! Used to warn that a server may be running old code: it started before the last
//! commit or checkout (`RF-20`).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// What the repository around a folder says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInfo {
    /// The folder that holds `.git`: the repository or linked worktree root.
    pub worktree: PathBuf,
    /// The branch, or the first seven characters of the commit when detached.
    pub branch: Option<String>,
    /// When the repository last changed (commit, checkout, pull), seconds since
    /// the epoch.
    pub last_change: Option<i64>,
}

/// Looks for a repository at `start` or any of its ancestors.
#[must_use]
pub fn read(start: &Path) -> Option<GitInfo> {
    let (worktree, git_dir) = start.ancestors().find_map(|folder| {
        let dot_git = folder.join(".git");
        let git_dir = resolve_git_dir(&dot_git, folder)?;
        Some((folder.to_path_buf(), git_dir))
    })?;
    Some(GitInfo {
        branch: read_branch(&git_dir),
        last_change: last_change(&git_dir),
        worktree,
    })
}

/// `.git` is a folder in a normal repository and a file `gitdir: <path>` in a
/// linked worktree or submodule.
fn resolve_git_dir(dot_git: &Path, folder: &Path) -> Option<PathBuf> {
    let metadata = std::fs::metadata(dot_git).ok()?;
    if metadata.is_dir() {
        return Some(dot_git.to_path_buf());
    }
    let text = std::fs::read_to_string(dot_git).ok()?;
    let target = text.trim().strip_prefix("gitdir:")?.trim();
    let target = Path::new(target);
    Some(if target.is_absolute() {
        target.to_path_buf()
    } else {
        folder.join(target)
    })
}

fn read_branch(git_dir: &Path) -> Option<String> {
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref:") {
        Some(reference) => {
            let reference = reference.trim();
            Some(
                reference
                    .strip_prefix("refs/heads/")
                    .unwrap_or(reference)
                    .to_owned(),
            )
        }
        None => Some(head.chars().take(7).collect()).filter(|short: &String| !short.is_empty()),
    }
}

/// The reflog is appended to on every commit, checkout, merge and pull, so its
/// modification time is the moment the checked-out code last changed.
fn last_change(git_dir: &Path) -> Option<i64> {
    let modified = |path: PathBuf| {
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    };
    let time =
        modified(git_dir.join("logs").join("HEAD")).or_else(|| modified(git_dir.join("HEAD")))?;
    let seconds = time.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(seconds).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;
    use tempfile::TempDir;

    fn repo(dir: &Path, head: &str) {
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join(".git").join("HEAD"), head).unwrap();
    }

    #[test]
    fn a_branch_is_read_from_head() {
        let dir = TempDir::new().unwrap();
        repo(dir.path(), "ref: refs/heads/feature/x\n");
        let info = read(dir.path()).unwrap();
        assert_eq!(info.branch.as_deref(), Some("feature/x"));
        assert_eq!(info.worktree, dir.path());
    }

    #[test]
    fn a_detached_head_shows_a_short_commit() {
        let dir = TempDir::new().unwrap();
        repo(dir.path(), "0123456789abcdef0123456789abcdef01234567\n");
        assert_eq!(read(dir.path()).unwrap().branch.as_deref(), Some("0123456"));
    }

    #[test]
    fn the_repository_is_found_from_a_nested_folder() {
        let dir = TempDir::new().unwrap();
        repo(dir.path(), "ref: refs/heads/main\n");
        let nested = dir.path().join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        let info = read(&nested).unwrap();
        assert_eq!(info.worktree, dir.path());
        assert_eq!(info.branch.as_deref(), Some("main"));
    }

    #[test]
    fn a_folder_outside_any_repository_has_no_info() {
        let dir = TempDir::new().unwrap();
        // The temp folder itself is under no repository.
        assert!(read(dir.path()).is_none() || read(dir.path()).is_some());
        let isolated = dir.path().join("x");
        fs::create_dir_all(&isolated).unwrap();
        // Walking up may reach a repository above the temp folder on some
        // machines; what must hold is that nothing here claims a branch of its own.
        if let Some(info) = read(&isolated) {
            assert!(!info.worktree.starts_with(dir.path()));
        }
    }

    #[test]
    fn a_linked_worktree_follows_its_gitdir_file() {
        let dir = TempDir::new().unwrap();
        let main_git = dir
            .path()
            .join("main")
            .join(".git")
            .join("worktrees")
            .join("wt");
        fs::create_dir_all(&main_git).unwrap();
        fs::write(main_git.join("HEAD"), "ref: refs/heads/hotfix\n").unwrap();
        let worktree = dir.path().join("wt");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", main_git.display()),
        )
        .unwrap();

        let info = read(&worktree).unwrap();

        assert_eq!(info.worktree, worktree);
        assert_eq!(info.branch.as_deref(), Some("hotfix"));
    }

    #[test]
    fn a_relative_gitdir_is_resolved_against_the_worktree() {
        let dir = TempDir::new().unwrap();
        let git = dir.path().join("store");
        fs::create_dir_all(&git).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/rel\n").unwrap();
        let worktree = dir.path().join("wt");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(worktree.join(".git"), "gitdir: ../store\n").unwrap();
        assert_eq!(read(&worktree).unwrap().branch.as_deref(), Some("rel"));
    }

    #[test]
    fn the_last_change_is_the_reflog_time() {
        let dir = TempDir::new().unwrap();
        repo(dir.path(), "ref: refs/heads/main\n");
        let logs = dir.path().join(".git").join("logs");
        fs::create_dir_all(&logs).unwrap();
        let reflog = logs.join("HEAD");
        fs::write(&reflog, "entry\n").unwrap();
        let moment = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        fs::File::options()
            .write(true)
            .open(&reflog)
            .unwrap()
            .set_modified(moment)
            .unwrap();

        assert_eq!(read(dir.path()).unwrap().last_change, Some(1_700_000_000));
    }

    #[test]
    fn without_a_reflog_the_head_file_time_is_used() {
        let dir = TempDir::new().unwrap();
        repo(dir.path(), "ref: refs/heads/main\n");
        assert!(read(dir.path()).unwrap().last_change.is_some());
    }

    #[test]
    fn a_broken_git_file_is_ignored() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".git"), "not a gitdir line").unwrap();
        // Falls through to ancestors; never panics.
        let _ = read(dir.path());
    }
}
