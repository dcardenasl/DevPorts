//! Finding the projects on disk (`MANIFIESTO.md`, section 7).
//!
//! A project is any folder that holds a `.devports.yml` (ADR-001). The search
//! starts from a list of root folders and goes down a bounded number of levels,
//! skipping folders that never hold projects and never following symbolic links.
//! Problems are collected as [`DiscoveryIssue`]s instead of aborting: one broken
//! manifest must not hide the other projects.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::catalog::{Catalog, Project};
use crate::diagnostic::Diagnostic;
use crate::manifest::{self, MANIFEST_FILE, ManifestError};

/// How many folder levels below a root are searched (`max_depth` in
/// `config.yml`). The root itself is level 0.
pub const DEFAULT_MAX_DEPTH: usize = 3;

/// Folder names that are never searched.
const IGNORED_DIRECTORIES: [&str; 4] = ["node_modules", "vendor", ".git", "target"];

/// What a discovery run found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    /// The projects whose manifest loaded.
    pub catalog: Catalog,
    /// Everything that went wrong, in the order it was met.
    pub issues: Vec<DiscoveryIssue>,
}

/// A problem met while discovering projects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryIssue {
    /// A configured root does not exist or is not a folder.
    RootMissing(PathBuf),
    /// A folder could not be listed.
    Unreadable {
        /// The folder.
        path: PathBuf,
        /// The operating system's explanation.
        reason: String,
    },
    /// A manifest exists but could not be loaded; its project is not registered.
    Manifest(ManifestError),
    /// A manifest loaded, but with warnings.
    ManifestWarnings {
        /// The manifest file.
        path: PathBuf,
        /// What is probably not as intended.
        diagnostics: Vec<Diagnostic>,
    },
    /// Two projects declare the same name, so `--project <name>` is ambiguous.
    DuplicateProjectName {
        /// The repeated name.
        name: String,
        /// The folders of the projects that use it.
        roots: Vec<PathBuf>,
    },
}

impl std::fmt::Display for DiscoveryIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootMissing(path) => write!(f, "{}: la carpeta no existe", path.display()),
            Self::Unreadable { path, reason } => {
                write!(f, "{}: no se pudo leer: {reason}", path.display())
            }
            Self::Manifest(error) => write!(f, "{error}"),
            Self::ManifestWarnings { path, diagnostics } => {
                let first = diagnostics
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                write!(f, "{}: {first}", path.display())
            }
            Self::DuplicateProjectName { name, roots } => {
                let places: Vec<String> = roots
                    .iter()
                    .map(|root| root.display().to_string())
                    .collect();
                write!(
                    f,
                    "el nombre de proyecto «{name}» se repite en {}",
                    places.join(", ")
                )
            }
        }
    }
}

/// Expands a leading `~` or `~/` to `home`. Anything else is returned unchanged.
#[must_use]
pub fn expand_tilde(path: &str, home: &Path) -> PathBuf {
    if path == "~" {
        home.to_path_buf()
    } else if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(path)
    }
}

/// The default search root, `~/developer` (ADR-024).
#[must_use]
pub fn default_roots(home: &Path) -> Vec<PathBuf> {
    vec![home.join("developer")]
}

/// Searches `roots` for projects, going down at most `max_depth` levels.
#[must_use]
pub fn discover(roots: &[PathBuf], max_depth: usize) -> Discovery {
    discover_with(roots, max_depth, None)
}

/// Like [`discover`], also checking each manifest's `uses` against the names in
/// `shared.yml`.
#[must_use]
pub fn discover_with(
    roots: &[PathBuf],
    max_depth: usize,
    known_shared: Option<&std::collections::BTreeSet<String>>,
) -> Discovery {
    let mut search = Search {
        known_shared,
        ..Search::default()
    };
    for root in roots {
        match std::fs::canonicalize(root) {
            Ok(canonical) if canonical.is_dir() => search.visit(&canonical, 0, max_depth),
            _ => search
                .issues
                .push(DiscoveryIssue::RootMissing(root.clone())),
        }
    }
    search.finish()
}

#[derive(Default)]
struct Search<'a> {
    known_shared: Option<&'a std::collections::BTreeSet<String>>,
    projects: Vec<Project>,
    issues: Vec<DiscoveryIssue>,
    /// Manifests already loaded, so overlapping roots do not register a project twice.
    loaded: BTreeSet<PathBuf>,
}

impl Search<'_> {
    fn visit(&mut self, dir: &Path, depth: usize, max_depth: usize) {
        self.load_manifest_in(dir);
        if depth >= max_depth {
            return;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) => {
                self.issues.push(DiscoveryIssue::Unreadable {
                    path: dir.to_path_buf(),
                    reason: error.to_string(),
                });
                return;
            }
        };
        let mut children: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            // `file_type` does not follow links, so a symlink is never a directory here.
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .filter(|entry| !is_skipped(&entry.file_name().to_string_lossy()))
            .map(|entry| entry.path())
            .collect();
        children.sort();
        for child in children {
            self.visit(&child, depth.saturating_add(1), max_depth);
        }
    }

    fn load_manifest_in(&mut self, dir: &Path) {
        let path = dir.join(MANIFEST_FILE);
        if !path.is_file() || !self.loaded.insert(path.clone()) {
            return;
        }
        match manifest::load(&path, self.known_shared) {
            Ok(loaded) => {
                if !loaded.warnings.is_empty() {
                    self.issues.push(DiscoveryIssue::ManifestWarnings {
                        path: path.clone(),
                        diagnostics: loaded.warnings,
                    });
                }
                self.projects
                    .push(Project::from_manifest(dir, &loaded.manifest));
            }
            Err(error) => self.issues.push(DiscoveryIssue::Manifest(error)),
        }
    }

    fn finish(mut self) -> Discovery {
        let mut roots_by_name: BTreeMap<&str, Vec<PathBuf>> = BTreeMap::new();
        for project in &self.projects {
            roots_by_name
                .entry(project.name.as_str())
                .or_default()
                .push(project.root.clone());
        }
        let duplicates: Vec<DiscoveryIssue> = roots_by_name
            .into_iter()
            .filter(|(_, roots)| roots.len() > 1)
            .map(|(name, roots)| DiscoveryIssue::DuplicateProjectName {
                name: name.to_owned(),
                roots,
            })
            .collect();
        self.issues.extend(duplicates);
        Discovery {
            catalog: Catalog::new(self.projects),
            issues: self.issues,
        }
    }
}

/// Folders that never hold projects: dependency and build output, version
/// control, and hidden folders.
fn is_skipped(name: &str) -> bool {
    name.starts_with('.') || IGNORED_DIRECTORIES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn manifest_text(name: &str) -> String {
        format!("version: 1\nproject: {name}\nservices:\n  web:\n    cwd: .\n")
    }

    fn write_manifest(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(MANIFEST_FILE), manifest_text(name)).unwrap();
    }

    fn names(discovery: &Discovery) -> Vec<&str> {
        discovery
            .catalog
            .projects()
            .iter()
            .map(|project| project.name.as_str())
            .collect()
    }

    fn found(root: &TempDir, max_depth: usize) -> Discovery {
        discover(&[root.path().to_path_buf()], max_depth)
    }

    /// I-14: depth limit, ignored folders and nesting.
    // covers: I-14
    #[test]
    fn finds_manifests_down_to_the_depth_limit_and_no_further() {
        let root = TempDir::new().unwrap();
        write_manifest(root.path(), "level0");
        write_manifest(&root.path().join("a"), "level1");
        write_manifest(&root.path().join("a/b"), "level2");
        write_manifest(&root.path().join("a/b/c"), "level3");
        write_manifest(&root.path().join("a/b/c/d"), "level4");

        let discovery = found(&root, 3);

        assert_eq!(names(&discovery), ["level0", "level1", "level2", "level3"]);
        assert!(discovery.issues.is_empty());
    }

    // covers: I-14
    #[test]
    fn skips_dependency_build_and_hidden_folders() {
        let root = TempDir::new().unwrap();
        write_manifest(&root.path().join("node_modules/pkg"), "in-node-modules");
        write_manifest(&root.path().join("vendor/pkg"), "in-vendor");
        write_manifest(&root.path().join("target/debug"), "in-target");
        write_manifest(&root.path().join(".git/x"), "in-git");
        write_manifest(&root.path().join(".hidden/x"), "in-hidden");
        write_manifest(&root.path().join("real"), "real");

        assert_eq!(names(&found(&root, 3)), ["real"]);
    }

    // covers: I-14
    #[test]
    fn nested_manifests_are_independent_projects() {
        let root = TempDir::new().unwrap();
        write_manifest(&root.path().join("shop"), "outer");
        write_manifest(&root.path().join("shop/api"), "inner");

        assert_eq!(names(&found(&root, 3)), ["outer", "inner"]);
    }

    #[test]
    fn a_broken_manifest_is_reported_and_does_not_hide_the_others() {
        let root = TempDir::new().unwrap();
        write_manifest(&root.path().join("good"), "good");
        fs::create_dir_all(root.path().join("bad")).unwrap();
        fs::write(
            root.path().join("bad").join(MANIFEST_FILE),
            "version: 7\nproject: bad\n",
        )
        .unwrap();
        fs::create_dir_all(root.path().join("worse")).unwrap();
        fs::write(
            root.path().join("worse").join(MANIFEST_FILE),
            "project: [unclosed\n",
        )
        .unwrap();

        let discovery = found(&root, 3);

        assert_eq!(names(&discovery), ["good"]);
        assert_eq!(discovery.issues.len(), 2);
        assert!(
            discovery
                .issues
                .iter()
                .all(|issue| matches!(issue, DiscoveryIssue::Manifest(_)))
        );
    }

    #[test]
    fn a_missing_root_is_an_issue_and_the_other_roots_still_work() {
        let root = TempDir::new().unwrap();
        write_manifest(&root.path().join("shop"), "shop");
        let missing = root.path().join("does-not-exist");

        let discovery = discover(&[missing.clone(), root.path().to_path_buf()], 3);

        assert_eq!(names(&discovery), ["shop"]);
        assert_eq!(discovery.issues, [DiscoveryIssue::RootMissing(missing)]);
    }

    #[test]
    fn overlapping_roots_register_a_project_once() {
        let root = TempDir::new().unwrap();
        write_manifest(&root.path().join("php/shop"), "shop");

        let discovery = discover(&[root.path().to_path_buf(), root.path().join("php")], 3);

        assert_eq!(names(&discovery), ["shop"]);
        assert!(discovery.issues.is_empty());
    }

    #[test]
    fn two_projects_with_the_same_name_are_reported() {
        let root = TempDir::new().unwrap();
        write_manifest(&root.path().join("one"), "twin");
        write_manifest(&root.path().join("two"), "twin");

        let discovery = found(&root, 3);

        assert_eq!(names(&discovery), ["twin", "twin"]);
        assert!(matches!(
            discovery.issues.as_slice(),
            [DiscoveryIssue::DuplicateProjectName { name, roots }] if name == "twin" && roots.len() == 2
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_not_followed() {
        let root = TempDir::new().unwrap();
        write_manifest(&root.path().join("real"), "real");
        // A link back to the root would loop forever if it were followed.
        std::os::unix::fs::symlink(root.path(), root.path().join("loop")).unwrap();
        // A link to a project elsewhere must not register it a second time.
        std::os::unix::fs::symlink(root.path().join("real"), root.path().join("alias")).unwrap();

        assert_eq!(names(&found(&root, 3)), ["real"]);
    }

    #[test]
    fn the_depth_limit_is_a_parameter() {
        let root = TempDir::new().unwrap();
        write_manifest(&root.path().join("a"), "one-down");
        write_manifest(&root.path().join("a/b"), "two-down");

        assert_eq!(names(&found(&root, 1)), ["one-down"]);
        assert_eq!(names(&found(&root, 2)), ["one-down", "two-down"]);
        assert!(names(&found(&root, 0)).is_empty());
    }

    #[test]
    fn project_roots_are_canonical() {
        let root = TempDir::new().unwrap();
        write_manifest(&root.path().join("shop"), "shop");

        let discovery = found(&root, 3);
        let expected = fs::canonicalize(root.path().join("shop")).unwrap();

        assert_eq!(discovery.catalog.projects()[0].root, expected);
    }

    #[test]
    fn tilde_expansion_only_touches_a_leading_tilde() {
        let home = Path::new("/Users/me");
        assert_eq!(expand_tilde("~", home), Path::new("/Users/me"));
        assert_eq!(
            expand_tilde("~/developer", home),
            Path::new("/Users/me/developer")
        );
        assert_eq!(expand_tilde("/opt/~/x", home), Path::new("/opt/~/x"));
        assert_eq!(expand_tilde("~other/x", home), Path::new("~other/x"));
        assert_eq!(
            expand_tilde("relative/dir", home),
            Path::new("relative/dir")
        );
        assert_eq!(default_roots(home), [PathBuf::from("/Users/me/developer")]);
    }
}
