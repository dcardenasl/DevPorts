//! Projects and services known to the core, and the rule that assigns an
//! observed process to one of them (`MANIFIESTO.md`, section 3).
//!
//! Identity is `(project, service)`, decided by the process's working directory
//! and command line, never by its port (ADR-003).

use std::path::{Component, Path, PathBuf};

use crate::docker::Container;
use crate::fingerprint::Fingerprint;
use crate::manifest::{Manifest, ServiceKind};
use crate::process::ListeningProcess;

/// A service of a project, with its paths resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    /// Service name, unique within its project.
    pub name: String,
    /// Whether the service holds state.
    pub kind: ServiceKind,
    /// Absolute, resolved working directory. `None` for container services.
    pub cwd: Option<PathBuf>,
    /// Container name. `None` for host services.
    pub docker: Option<String>,
    /// Preferred port of the main instance.
    pub port: Option<u16>,
    /// Fingerprint declared by `match`, if any.
    pub fingerprint: Option<Fingerprint>,
}

/// A project: the folder that holds a `.devports.yml` (ADR-001).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// Project name from the manifest.
    pub name: String,
    /// Absolute, resolved project folder.
    pub root: PathBuf,
    /// Services declared by the manifest, ordered by name.
    pub services: Vec<Service>,
}

impl Project {
    /// Builds a project from its manifest. `root` is the folder containing the
    /// manifest; relative service paths are resolved against it.
    #[must_use]
    pub fn from_manifest(root: &Path, manifest: &Manifest) -> Self {
        let root = resolve(root);
        let services = manifest
            .services
            .iter()
            .map(|(name, spec)| Service {
                name: name.clone(),
                kind: spec.kind,
                cwd: spec.cwd.as_deref().map(|cwd| resolve(&root.join(cwd))),
                docker: spec.docker.clone(),
                port: spec.port,
                fingerprint: spec.match_text.as_deref().map(Fingerprint::from_match_text),
            })
            .collect();
        Self {
            name: manifest.project.clone(),
            root,
            services,
        }
    }
}

/// Where an observed process belongs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification<'a> {
    /// The process is an instance of this service.
    Service {
        /// The owning project.
        project: &'a Project,
        /// The matched service.
        service: &'a Service,
    },
    /// The process runs inside a project but matches none of its services.
    /// It is a candidate for "adopt".
    UnregisteredInProject {
        /// The project it runs inside.
        project: &'a Project,
    },
    /// Several services match equally well; none is chosen at random.
    Ambiguous {
        /// The project it runs inside.
        project: &'a Project,
        /// The services that tie.
        candidates: Vec<&'a Service>,
    },
    /// The process is outside every project, or its directory is unknown.
    NoProject,
}

/// Where a container belongs (ADR-012).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerClassification<'a> {
    /// A manifest declares this container as a service (`docker: <name>`).
    Service {
        /// The owning project.
        project: &'a Project,
        /// The service that names the container.
        service: &'a Service,
    },
    /// Several projects declare a service with this container name.
    Ambiguous {
        /// The projects that claim it, ordered by folder.
        projects: Vec<&'a Project>,
    },
    /// A compose container whose compose folder lies inside a known project.
    InProject {
        /// The project that contains the compose folder.
        project: &'a Project,
    },
    /// A compose container outside every known project, grouped by its compose
    /// project name.
    Compose {
        /// `com.docker.compose.project`.
        name: &'a str,
        /// `com.docker.compose.project.working_dir`, when present.
        working_dir: Option<&'a Path>,
    },
    /// A loose container that no manifest declares: "no project" until someone
    /// assigns it with one click.
    NoProject,
}

/// Every project the core knows about.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    projects: Vec<Project>,
}

impl Catalog {
    /// Builds a catalog from projects, in any order.
    #[must_use]
    pub fn new(mut projects: Vec<Project>) -> Self {
        projects.sort_by(|a, b| a.root.cmp(&b.root));
        Self { projects }
    }

    /// The projects, ordered by folder.
    #[must_use]
    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    /// Decides which project a container belongs to (ADR-012).
    ///
    /// 1. A service with `docker: <name>` equal to the container name claims it.
    /// 2. Otherwise a compose container belongs to the deepest known project
    ///    that contains its compose folder.
    /// 3. Otherwise it is grouped under its compose project name.
    /// 4. Otherwise it is loose.
    #[must_use]
    pub fn classify_container<'a>(
        &'a self,
        container: &'a Container,
    ) -> ContainerClassification<'a> {
        let claims: Vec<(&Project, &Service)> = self
            .projects
            .iter()
            .flat_map(|project| {
                project
                    .services
                    .iter()
                    .filter(|service| service.docker.as_deref() == Some(container.name.as_str()))
                    .map(move |service| (project, service))
            })
            .collect();
        match claims.as_slice() {
            [] => {}
            [(project, service)] => return ContainerClassification::Service { project, service },
            _ => {
                return ContainerClassification::Ambiguous {
                    projects: claims.iter().map(|(project, _)| *project).collect(),
                };
            }
        }

        let Some(compose) = &container.compose else {
            return ContainerClassification::NoProject;
        };
        let inside = compose.working_dir.as_deref().and_then(|dir| {
            let dir = resolve(dir);
            self.projects
                .iter()
                .filter(|project| dir.starts_with(&project.root))
                .max_by_key(|project| project.root.components().count())
        });
        match inside {
            Some(project) => ContainerClassification::InProject { project },
            None => ContainerClassification::Compose {
                name: &compose.project,
                working_dir: compose.working_dir.as_deref(),
            },
        }
    }

    /// Decides which project and service `process` belongs to.
    ///
    /// 1. The project is the deepest one whose folder contains the process's
    ///    working directory ("the deepest manifest wins").
    /// 2. A service matches when the working directory is its `cwd` or inside
    ///    it, and, if it declares `match`, the process's normalised command line
    ///    contains that fingerprint as consecutive tokens.
    /// 3. Several matches: the longest fingerprint wins; on a tie, the service
    ///    whose `port` the process listens on; on a further tie the answer is
    ///    [`Classification::Ambiguous`].
    #[must_use]
    pub fn classify(&self, process: &ListeningProcess) -> Classification<'_> {
        let Some(cwd) = process.info.cwd.as_deref() else {
            return Classification::NoProject;
        };
        let Some(project) = self
            .projects
            .iter()
            .filter(|project| cwd.starts_with(&project.root))
            .max_by_key(|project| project.root.components().count())
        else {
            return Classification::NoProject;
        };

        let command = Fingerprint::from_argv(&process.info.argv);
        let mut matches: Vec<&Service> = project
            .services
            .iter()
            .filter(|service| {
                service
                    .cwd
                    .as_deref()
                    .is_some_and(|dir| cwd.starts_with(dir))
                    && service
                        .fingerprint
                        .as_ref()
                        .is_none_or(|needle| command.contains(needle))
            })
            .collect();

        if matches.len() > 1 {
            keep_best(&mut matches, |service| {
                service.fingerprint.as_ref().map_or(0, Fingerprint::len)
            });
        }
        if matches.len() > 1 {
            let on_port: Vec<&Service> = matches
                .iter()
                .copied()
                .filter(|service| {
                    service
                        .port
                        .is_some_and(|port| process.ports.contains(&port))
                })
                .collect();
            if !on_port.is_empty() {
                matches = on_port;
            }
        }

        match matches.as_slice() {
            [] => Classification::UnregisteredInProject { project },
            [service] => Classification::Service { project, service },
            _ => Classification::Ambiguous {
                project,
                candidates: matches,
            },
        }
    }
}

/// Keeps only the entries with the highest `score`.
fn keep_best(entries: &mut Vec<&Service>, score: impl Fn(&Service) -> usize) {
    let best = entries.iter().map(|entry| score(entry)).max().unwrap_or(0);
    entries.retain(|entry| score(entry) == best);
}

/// Absolute path with `.` and `..` resolved. Symbolic links are resolved too
/// when the path exists, so it compares equal to the directories the kernel
/// reports for processes (`/var` and `/private/var`, upper and lower case on a
/// case-insensitive volume). A path that does not exist yet is resolved
/// lexically.
fn resolve(path: &Path) -> PathBuf {
    let lexical = lexically_normalized(path);
    std::fs::canonicalize(&lexical).unwrap_or(lexical)
}

fn lexically_normalized(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(component);
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::{ComposeLabels, ContainerState};
    use crate::manifest::{self, Manifest};
    use crate::process::{ProcessId, ProcessInfo, ProcessStart};
    use proptest::prelude::*;

    fn project(root: &str, yaml: &str) -> Project {
        let manifest: Manifest = manifest::parse(yaml, Path::new("m.yml")).unwrap();
        Project::from_manifest(Path::new(root), &manifest)
    }

    fn process(cwd: &str, argv: &[&str], ports: &[u16]) -> ListeningProcess {
        ListeningProcess {
            info: ProcessInfo {
                id: ProcessId {
                    pid: 100,
                    start: ProcessStart(1),
                },
                ppid: 1,
                pgid: 100,
                uid: 501,
                argv: argv.iter().map(|part| (*part).to_owned()).collect(),
                cwd: Some(PathBuf::from(cwd)),
            },
            ports: ports.to_vec(),
        }
    }

    fn shop() -> Project {
        project(
            "/work/shop",
            "version: 1\nproject: shop\nservices:\n  \
             api:\n    cwd: ./api\n    port: 8080\n    match: php spark serve\n  \
             admin:\n    cwd: ./admin\n    port: 8081\n  \
             cache:\n    docker: redis\n    kind: data\n",
        )
    }

    fn name_of<'a>(classification: &Classification<'a>) -> Option<&'a str> {
        match classification {
            Classification::Service { service, .. } => Some(service.name.as_str()),
            _ => None,
        }
    }

    #[test]
    fn a_process_in_a_service_directory_with_its_command_matches() {
        let catalog = Catalog::new(vec![shop()]);
        let found = catalog.classify(&process(
            "/work/shop/api",
            &["/usr/bin/php", "spark", "serve", "--port", "8080"],
            &[8080],
        ));
        assert_eq!(name_of(&found), Some("api"));
    }

    #[test]
    fn a_directory_inside_the_service_directory_still_matches() {
        let catalog = Catalog::new(vec![shop()]);
        let found = catalog.classify(&process(
            "/work/shop/api/public",
            &["php", "spark", "serve"],
            &[8080],
        ));
        assert_eq!(name_of(&found), Some("api"));
    }

    #[test]
    fn a_service_without_match_accepts_any_command_in_its_directory() {
        let catalog = Catalog::new(vec![shop()]);
        let found = catalog.classify(&process("/work/shop/admin", &["anything"], &[9000]));
        assert_eq!(name_of(&found), Some("admin"));
    }

    #[test]
    fn the_wrong_command_in_the_right_directory_is_unregistered() {
        let catalog = Catalog::new(vec![shop()]);
        let found = catalog.classify(&process(
            "/work/shop/api",
            &["python3", "-m", "http.server", "9000"],
            &[9000],
        ));
        assert!(
            matches!(found, Classification::UnregisteredInProject { project } if project.name == "shop")
        );
    }

    #[test]
    fn a_directory_in_the_project_but_in_no_service_is_unregistered() {
        let catalog = Catalog::new(vec![shop()]);
        let found = catalog.classify(&process("/work/shop/checkout", &["node"], &[3000]));
        assert!(matches!(
            found,
            Classification::UnregisteredInProject { .. }
        ));
    }

    #[test]
    fn a_directory_outside_every_project_has_no_project() {
        let catalog = Catalog::new(vec![shop()]);
        assert_eq!(
            catalog.classify(&process("/elsewhere", &["node"], &[3000])),
            Classification::NoProject
        );
        let mut unknown = process("/work/shop/api", &["node"], &[3000]);
        unknown.info.cwd = None;
        assert_eq!(catalog.classify(&unknown), Classification::NoProject);
    }

    #[test]
    fn a_sibling_folder_with_a_common_prefix_is_not_inside_the_project() {
        let catalog = Catalog::new(vec![shop()]);
        assert_eq!(
            catalog.classify(&process(
                "/work/shop-old/api",
                &["php", "spark", "serve"],
                &[1]
            )),
            Classification::NoProject
        );
    }

    #[test]
    fn the_deepest_project_wins_for_nested_manifests() {
        let outer = project(
            "/work/shop",
            "version: 1\nproject: outer\nservices:\n  all:\n    cwd: .\n",
        );
        let inner = project(
            "/work/shop/api",
            "version: 1\nproject: inner\nservices:\n  api:\n    cwd: .\n",
        );
        let catalog = Catalog::new(vec![outer, inner]);
        let found = catalog.classify(&process("/work/shop/api/src", &["node"], &[3000]));
        match found {
            Classification::Service { project, service } => {
                assert_eq!(
                    (project.name.as_str(), service.name.as_str()),
                    ("inner", "api")
                );
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn the_longest_fingerprint_breaks_a_tie_between_services() {
        let project = project(
            "/work/app",
            "version: 1\nproject: app\nservices:\n  \
             generic:\n    cwd: .\n    match: node\n  \
             vite:\n    cwd: .\n    match: node vite --port=N\n",
        );
        let catalog = Catalog::new(vec![project]);
        let found = catalog.classify(&process(
            "/work/app",
            &["node", "/x/node_modules/.bin/vite", "--port=5173"],
            &[5173],
        ));
        assert_eq!(name_of(&found), Some("vite"));
    }

    #[test]
    fn the_listening_port_breaks_a_remaining_tie() {
        let project = project(
            "/work/app",
            "version: 1\nproject: app\nservices:\n  \
             one:\n    cwd: .\n    port: 4000\n  \
             two:\n    cwd: .\n    port: 4001\n",
        );
        let catalog = Catalog::new(vec![project]);
        let found = catalog.classify(&process("/work/app", &["node"], &[4001]));
        assert_eq!(name_of(&found), Some("two"));
    }

    #[test]
    fn an_unresolved_tie_is_ambiguous_and_never_random() {
        let project = project(
            "/work/app",
            "version: 1\nproject: app\nservices:\n  \
             one:\n    cwd: .\n    port: 4000\n  \
             two:\n    cwd: .\n    port: 4001\n",
        );
        let catalog = Catalog::new(vec![project]);
        let found = catalog.classify(&process("/work/app", &["node"], &[9999]));
        let Classification::Ambiguous { candidates, .. } = found else {
            panic!("expected an ambiguous result, got {found:?}");
        };
        let names: Vec<&str> = candidates
            .iter()
            .map(|service| service.name.as_str())
            .collect();
        assert_eq!(names, ["one", "two"]);
    }

    #[test]
    fn container_services_never_match_a_host_process() {
        let catalog = Catalog::new(vec![shop()]);
        let found = catalog.classify(&process("/work/shop", &["redis-server"], &[6379]));
        assert!(!matches!(found, Classification::Service { .. }));
    }

    #[test]
    fn service_paths_are_resolved_lexically_when_they_do_not_exist() {
        let project = project(
            "/work/shop",
            "version: 1\nproject: shop\nservices:\n  api:\n    cwd: ./a/../api/./\n",
        );
        assert_eq!(
            project.services[0].cwd.as_deref(),
            Some(Path::new("/work/shop/api"))
        );
    }

    fn container(name: &str, compose: Option<(&str, Option<&str>)>) -> Container {
        Container {
            id: format!("id-{name}"),
            name: name.to_owned(),
            image: "mysql:8".to_owned(),
            state: ContainerState::Running,
            ports: Vec::new(),
            compose: compose.map(|(project, dir)| ComposeLabels {
                project: project.to_owned(),
                working_dir: dir.map(PathBuf::from),
            }),
            created: 1,
        }
    }

    /// D-01: compose containers are assigned by their labels.
    #[test]
    fn a_container_named_by_a_manifest_is_that_service() {
        let catalog = Catalog::new(vec![shop()]);
        let cache = container("redis", None);
        assert!(matches!(
            catalog.classify_container(&cache),
            ContainerClassification::Service { project, service }
                if project.name == "shop" && service.name == "cache"
        ));
    }

    #[test]
    fn a_compose_container_in_a_known_project_folder_belongs_to_it() {
        let catalog = Catalog::new(vec![shop()]);
        let web = container("shop-web-1", Some(("shop", Some("/work/shop/web"))));
        assert!(matches!(
            catalog.classify_container(&web),
            ContainerClassification::InProject { project } if project.name == "shop"
        ));
    }

    #[test]
    fn the_deepest_project_wins_for_a_compose_folder_too() {
        let outer = project("/work/shop", "version: 1\nproject: outer\nservices: {}\n");
        let inner = project(
            "/work/shop/api",
            "version: 1\nproject: inner\nservices: {}\n",
        );
        let catalog = Catalog::new(vec![outer, inner]);
        let web = container("x", Some(("p", Some("/work/shop/api/compose"))));
        assert!(matches!(
            catalog.classify_container(&web),
            ContainerClassification::InProject { project } if project.name == "inner"
        ));
    }

    #[test]
    fn a_compose_container_elsewhere_is_grouped_by_its_compose_project() {
        let catalog = Catalog::new(vec![shop()]);
        let mysql = container(
            "mysql",
            Some(("docker-services", Some("/Users/me/docker-services"))),
        );
        assert_eq!(
            catalog.classify_container(&mysql),
            ContainerClassification::Compose {
                name: "docker-services",
                working_dir: Some(Path::new("/Users/me/docker-services")),
            }
        );
    }

    #[test]
    fn a_compose_container_without_a_folder_is_still_grouped() {
        let catalog = Catalog::new(vec![shop()]);
        let lone = container("db", Some(("stack", None)));
        assert!(matches!(
            catalog.classify_container(&lone),
            ContainerClassification::Compose {
                name: "stack",
                working_dir: None
            }
        ));
    }

    #[test]
    fn a_loose_container_has_no_project() {
        let catalog = Catalog::new(vec![shop()]);
        assert_eq!(
            catalog.classify_container(&container("suite-mariadb", None)),
            ContainerClassification::NoProject
        );
    }

    #[test]
    fn two_projects_claiming_one_container_are_ambiguous() {
        let first = project(
            "/work/a",
            "version: 1\nproject: a\nservices:\n  db:\n    docker: shared-db\n",
        );
        let second = project(
            "/work/b",
            "version: 1\nproject: b\nservices:\n  db:\n    docker: shared-db\n",
        );
        let catalog = Catalog::new(vec![first, second]);
        let shared = container("shared-db", None);
        let found = catalog.classify_container(&shared);
        let ContainerClassification::Ambiguous { projects } = found else {
            panic!("expected an ambiguous result, got {found:?}");
        };
        let names: Vec<&str> = projects
            .iter()
            .map(|project| project.name.as_str())
            .collect();
        assert_eq!(names, ["a", "b"]);
    }

    proptest! {
        /// PT-02: the identity of a service does not change when only the port does.
        #[test]
        fn identity_survives_a_port_change(first in 1024_u16..=65_535, second in 1024_u16..=65_535) {
            let catalog = Catalog::new(vec![shop()]);
            let run = |port: u16| {
                let port_text = port.to_string();
                let found = catalog.classify(&process(
                    "/work/shop/api",
                    &["php", "spark", "serve", "--port", port_text.as_str()],
                    &[port],
                ));
                name_of(&found).map(str::to_owned)
            };
            prop_assert_eq!(run(first), run(second));
            prop_assert_eq!(run(first), Some("api".to_owned()));
        }
    }
}
