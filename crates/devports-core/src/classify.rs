//! A guess about what a loose listener is, with the evidence behind it (ADR-033).
//!
//! The class is advice for the panel's layout, never a decision: it does not
//! ignore, hide or stop anything, and the stop rules do not read it. Only
//! explicit signals classify; a process with none stays unclassified instead of
//! being forced into a group. A person's correction is stored under the same
//! exact criterion as an ignore rule and is separate from it.

use devports_proto::views::InstanceView;
use serde::{Deserialize, Serialize};

/// Where a loose listener is placed in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ProcessClass {
    /// Looks like a development server of a project.
    Project,
    /// Looks like a tool or daemon that something else runs.
    Auxiliary,
    /// No evidence either way.
    Unclassified,
}

impl ProcessClass {
    /// The name stored in the database.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Auxiliary => "auxiliary",
            Self::Unclassified => "unclassified",
        }
    }

    /// Reads a stored name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "project" => Some(Self::Project),
            "auxiliary" => Some(Self::Auxiliary),
            "unclassified" => Some(Self::Unclassified),
            _ => None,
        }
    }
}

/// The class of one loose listener and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Classification {
    /// The instance's reference.
    pub instance_ref: String,
    /// The class in force: the person's correction if there is one.
    pub class: ProcessClass,
    /// What automatic evidence alone says. Equal to `class` when not corrected.
    pub detected: ProcessClass,
    /// The program's file name, so a tool can be recognised without its arguments.
    pub program: Option<String>,
    /// Who the tool comes from (`Homebrew`, `VS Code`...), when a signal says so.
    pub origin: Option<String>,
    /// Which signals matched: `homebrewDaemon`, `editorExtension`, `knownProject`, `gitWorktree`.
    pub signals: Vec<String>,
    /// The person chose `class`, even if it equals the automatic one; it can be reset.
    pub corrected: bool,
}

/// Homebrew's install prefixes.
const HOMEBREW_PREFIXES: [&str; 4] = [
    "/opt/homebrew/",
    "/usr/local/Cellar/",
    "/usr/local/opt/",
    "/home/linuxbrew/.linuxbrew/",
];

/// Daemons a package manager starts for other programs. A Homebrew `node` or
/// `python` is somebody's server, so only these names count.
const DAEMONS: [&str; 14] = [
    "php-fpm",
    "nginx",
    "httpd",
    "mysqld",
    "mysqld_safe",
    "mariadbd",
    "postgres",
    "redis-server",
    "memcached",
    "mongod",
    "dnsmasq",
    "caddy",
    "mailpit",
    "mailhog",
];

/// Where editors keep the programs their extensions run, and who owns them.
const EDITOR_MARKERS: [(&str, &str); 7] = [
    ("/.vscode/extensions/", "VS Code"),
    ("/.vscode-server/", "VS Code"),
    ("/.vscode-insiders/", "VS Code Insiders"),
    ("/.cursor/extensions/", "Cursor"),
    ("/.windsurf/", "Windsurf"),
    ("/JetBrains/", "JetBrains"),
    ("/.config/Code/", "VS Code"),
];

/// What the evidence says about a loose host process, before any correction.
#[must_use]
pub fn detect(view: &InstanceView) -> (ProcessClass, Option<String>, Vec<String>) {
    let program = view.argv.first();
    let mut signals = Vec::new();
    let mut origin = None;
    if let Some(program) = program {
        let name = program.rsplit('/').next().unwrap_or(program);
        if HOMEBREW_PREFIXES
            .iter()
            .any(|prefix| program.starts_with(prefix))
            && DAEMONS.contains(&name)
        {
            signals.push("homebrewDaemon".to_owned());
            origin = Some("Homebrew".to_owned());
        }
    }
    if let Some((_, owner)) = EDITOR_MARKERS
        .iter()
        .find(|(marker, _)| view.argv.iter().any(|part| part.contains(marker)))
    {
        signals.push("editorExtension".to_owned());
        origin = Some((*owner).to_owned());
    }
    let auxiliary = !signals.is_empty();
    if view.project.is_some() {
        signals.push("knownProject".to_owned());
    }
    if view.worktree.is_some() {
        signals.push("gitWorktree".to_owned());
    }
    let class = if auxiliary {
        ProcessClass::Auxiliary
    } else if view.project.is_some() || view.worktree.is_some() {
        ProcessClass::Project
    } else {
        ProcessClass::Unclassified
    };
    (class, origin, signals)
}

/// The classification of `view`, applying the person's `correction` if any.
#[must_use]
pub fn classify(view: &InstanceView, correction: Option<ProcessClass>) -> Classification {
    let (detected, origin, signals) = detect(view);
    Classification {
        instance_ref: view.instance_ref.clone(),
        class: correction.unwrap_or(detected),
        detected,
        program: view
            .argv
            .first()
            .map(|program| program.rsplit('/').next().unwrap_or(program).to_owned()),
        origin,
        signals,
        corrected: correction.is_some(),
    }
}

#[cfg(test)]
mod tests;
