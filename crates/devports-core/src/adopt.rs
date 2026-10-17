//! "Adopt": propose a manifest from what is running (`MANIFIESTO.md`, section 8).
//!
//! The manifest is born from observation, never written from scratch (ADR-002).
//! This module only *proposes*: it builds the services a folder would declare and
//! renders them as YAML. Writing a file is a person's action from the
//! application, never something an agent can do through the socket.

use std::path::{Path, PathBuf};

use devports_proto::views::InstanceView;

use crate::fingerprint::Fingerprint;

pub mod apply;

/// Images recognised as databases and caches: `kind: data` (ADR-006, ADR-013).
const DATA_IMAGES: [&str; 7] = [
    "mysql",
    "mariadb",
    "postgres",
    "redis",
    "mongo",
    "memcached",
    "elasticsearch",
];
/// Folder names that only say where files are served from.
const GENERIC_FOLDERS: [&str; 9] = [
    "public", "dist", "build", "www", "html", "web", "src", "out", "site",
];
/// Programs that start the real server as a child; their command line is not
/// the one to run, so the proposal flags it for review.
const LAUNCHERS: [&str; 6] = ["npm", "npx", "yarn", "pnpm", "composer", "bun"];

/// One proposed service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedService {
    /// Service name.
    pub name: String,
    /// Working directory relative to the project folder, for a host service.
    pub cwd: Option<String>,
    /// Container name, for a container.
    pub docker: Option<String>,
    /// Port it listens on.
    pub port: Option<u16>,
    /// Command that starts it, with the port replaced by `${PORT}`.
    pub start: Option<String>,
    /// Fingerprint of its command line.
    pub match_text: Option<String>,
    /// `data` for databases and caches.
    pub is_data: bool,
    /// Why the proposed `start` needs a person's eyes.
    pub review: Option<String>,
}

/// A manifest proposal for a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Project name.
    pub project: String,
    /// The folder the manifest would live in.
    pub root: PathBuf,
    /// Proposed services, by name.
    pub services: Vec<ProposedService>,
}

/// Builds the proposal for `dir` from the instances that run inside it and belong
/// to no service yet.
#[must_use]
pub fn propose(dir: &Path, instances: &[InstanceView]) -> Proposal {
    let mut services: Vec<ProposedService> = Vec::new();
    for view in instances.iter().filter(|view| view.service.is_none()) {
        let inside = view
            .cwd
            .as_deref()
            .map(Path::new)
            .is_some_and(|cwd| cwd.starts_with(dir));
        if !inside {
            continue;
        }
        let service = match (&view.container, &view.cwd) {
            (Some(container), _) => container_service(container, view),
            (None, Some(cwd)) => process_service(dir, Path::new(cwd), view),
            (None, None) => continue,
        };
        services.push(service);
    }
    name_uniquely(&mut services);
    services.sort_by(|a, b| a.name.cmp(&b.name));
    Proposal {
        project: slug(
            dir.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("proyecto"),
        ),
        root: dir.to_path_buf(),
        services,
    }
}

/// The service `view` would be inside the project at `root`, or `None` if it
/// cannot be described there (a process whose folder is outside the project).
#[must_use]
pub fn service_for(root: &Path, view: &InstanceView) -> Option<ProposedService> {
    match (&view.container, &view.cwd) {
        (Some(container), _) => Some(container_service(container, view)),
        (None, Some(cwd)) if Path::new(cwd).starts_with(root) => {
            Some(process_service(root, Path::new(cwd), view))
        }
        _ => None,
    }
}

fn process_service(dir: &Path, cwd: &Path, view: &InstanceView) -> ProposedService {
    let relative = cwd
        .strip_prefix(dir)
        .ok()
        .filter(|rel| !rel.as_os_str().is_empty());
    // A document root says nothing about what the service is: use the project's.
    let folder = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !GENERIC_FOLDERS.contains(&name.to_lowercase().as_str()));
    let name = slug(
        folder
            .or_else(|| dir.file_name().and_then(|name| name.to_str()))
            .unwrap_or("servicio"),
    );
    let argv = view.argv.clone();
    let program = argv
        .first()
        .and_then(|program| program.rsplit('/').next())
        .unwrap_or_default()
        .to_owned();
    let review = LAUNCHERS.contains(&program.as_str()).then(|| {
        format!("«{program}» suele lanzar el servidor real como proceso hijo: revisa el comando")
    });
    ProposedService {
        name,
        cwd: Some(relative.map_or_else(|| ".".to_owned(), |rel| format!("./{}", rel.display()))),
        docker: None,
        port: view.port,
        start: view.port.map(|port| start_command(&argv, port)),
        match_text: (!argv.is_empty()).then(|| Fingerprint::from_argv(&argv).render()),
        is_data: false,
        review,
    }
}

fn container_service(name: &str, view: &InstanceView) -> ProposedService {
    let is_data = view.image.as_deref().is_some_and(|image| {
        let image = image.to_lowercase();
        DATA_IMAGES.iter().any(|known| image.contains(known))
    });
    ProposedService {
        name: slug(name),
        cwd: None,
        docker: Some(name.to_owned()),
        port: view.port,
        start: Some(format!("docker start {name}")),
        match_text: None,
        is_data,
        review: None,
    }
}

/// `argv` as one command, with the listening port replaced by `${PORT}`.
fn start_command(argv: &[String], port: u16) -> String {
    let port_text = port.to_string();
    argv.iter()
        .enumerate()
        .map(|(index, token)| {
            let token = if index == 0 {
                token
                    .rsplit('/')
                    .find(|part| !part.is_empty())
                    .unwrap_or(token)
                    .to_owned()
            } else {
                token.clone()
            };
            if token == port_text {
                "${PORT}".to_owned()
            } else if let Some(head) = token.strip_suffix(&format!("={port_text}")) {
                format!("{head}=${{PORT}}")
            } else if let Some(head) = token.strip_suffix(&format!(":{port_text}")) {
                format!("{head}:${{PORT}}")
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lower case, letters and digits, other runs becoming one hyphen.
#[must_use]
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "servicio".to_owned()
    } else {
        trimmed.chars().take(63).collect()
    }
}

/// Two services with the same name get their port appended (`api-8080`).
fn name_uniquely(services: &mut [ProposedService]) {
    let names: Vec<String> = services
        .iter()
        .map(|service| service.name.clone())
        .collect();
    for service in services.iter_mut() {
        if names.iter().filter(|name| **name == service.name).count() > 1
            && let Some(port) = service.port
        {
            service.name = format!("{}-{port}", service.name);
        }
    }
}

impl ProposedService {
    /// The service as the lines of a `services:` entry, indented by two and four
    /// spaces.
    #[must_use]
    pub fn yaml_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("  {}:", self.name)];
        if let Some(cwd) = &self.cwd {
            lines.push(format!("    cwd: {}", scalar(cwd)));
        }
        if let Some(container) = &self.docker {
            lines.push(format!("    docker: {}", scalar(container)));
        }
        if self.is_data {
            lines.push("    kind: data".to_owned());
        }
        if let Some(port) = self.port {
            lines.push(format!("    port: {port}"));
        }
        if let Some(start) = &self.start {
            lines.push(match &self.review {
                Some(note) => format!("    start: {}   # revisar: {note}", scalar(start)),
                None => format!("    start: {}", scalar(start)),
            });
        }
        if let Some(text) = &self.match_text {
            lines.push(format!("    match: {}", scalar(text)));
        }
        lines
    }
}

impl Proposal {
    /// The proposal as a `.devports.yml`.
    #[must_use]
    pub fn to_yaml(&self) -> String {
        let mut lines = vec![
            "version: 1".to_owned(),
            format!("project: {}", scalar(&self.project)),
        ];
        if self.services.is_empty() {
            lines.push("services: {}".to_owned());
            return join_lines(&lines);
        }
        lines.push("services:".to_owned());
        for service in &self.services {
            lines.extend(service.yaml_lines());
        }
        join_lines(&lines)
    }
}

fn join_lines(lines: &[String]) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// A YAML scalar, quoted only when it has to be.
fn scalar(text: &str) -> String {
    let plain = !text.is_empty()
        && !text.starts_with(|c: char| {
            matches!(
                c,
                '-' | '?'
                    | ':'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | ','
                    | '#'
                    | '&'
                    | '*'
                    | '!'
                    | '|'
                    | '>'
                    | '\''
                    | '"'
                    | '%'
                    | '@'
                    | '`'
                    | ' '
            )
        })
        && !text.contains(": ")
        && !text.contains(" #")
        && !text.ends_with(':')
        && !text.ends_with(' ');
    if plain {
        text.to_owned()
    } else {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

#[cfg(test)]
mod tests;
