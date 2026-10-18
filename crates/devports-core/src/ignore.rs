//! The rule behind "ignore this service" (ADR-032).
//!
//! A rule is a stored key that names exactly one thing: a command in a folder,
//! or a container on one Docker context. The key is the whole criterion: the
//! panel shows it before it is saved and again when it is listed, and nothing
//! broader than it ever matches. Rules written by earlier builds keep matching
//! exactly as they did, are listed as such and can be restored like any other.

use std::collections::BTreeSet;

use devports_proto::views::InstanceView;
use serde::{Deserialize, Serialize};

/// The context name Docker uses when nothing else is configured.
pub const DEFAULT_CONTEXT: &str = "default";
const PROCESS_V2: &str = "v2:process:";
const CONTAINER_V2: &str = "v2:container:";
const PROCESS_V1: &str = "process:";
const CONTAINER_V1: &str = "container:";
const SEPARATOR: char = '\u{1f}';

/// What a rule keeps out of the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum IgnoreKind {
    /// A command line started in a folder.
    Process,
    /// A container on a Docker context.
    Container,
    /// A stored key this build cannot read; it is still listed and restorable.
    Unknown,
}

/// A rule as the person sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct IgnoreRule {
    /// Stable handle for restoring; derived from the stored key.
    pub id: String,
    /// What kind of thing it matches.
    pub kind: IgnoreKind,
    /// Working directory a process must have started in.
    pub folder: Option<String>,
    /// Exact command line, program first. For rules from earlier builds the
    /// arguments are split on spaces for display only.
    pub command: Vec<String>,
    /// Container name.
    pub container: Option<String>,
    /// Docker context the container lives on, when the rule records one.
    pub context: Option<String>,
    /// Written by an earlier build: matches as it always did and is never widened.
    pub legacy: bool,
    /// When the rule was saved, ISO 8601; absent for a preview.
    pub created_at: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ProcessCriterion {
    cwd: String,
    argv: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct ContainerCriterion {
    context: String,
    name: String,
}

/// The key a new rule for `view` is stored under: its folder and exact argument
/// vector, or its container name on the current Docker `context`. Arguments are
/// kept as a list, so `["a b"]` and `["a", "b"]` are different commands.
#[must_use]
pub fn rule_key(view: &InstanceView, context: &str) -> Option<String> {
    if let Some(name) = &view.container {
        let criterion = ContainerCriterion {
            context: context.to_owned(),
            name: name.clone(),
        };
        return serde_json::to_string(&criterion)
            .ok()
            .map(|json| format!("{CONTAINER_V2}{json}"));
    }
    let cwd = view.cwd.clone()?;
    view.argv.first()?;
    let criterion = ProcessCriterion {
        cwd,
        argv: view.argv.clone(),
    };
    serde_json::to_string(&criterion)
        .ok()
        .map(|json| format!("{PROCESS_V2}{json}"))
}

/// The key earlier builds stored, kept only to recognise their rules.
fn legacy_key(view: &InstanceView) -> Option<String> {
    if let Some(container) = &view.container {
        return Some(format!("{CONTAINER_V1}{container}"));
    }
    let cwd = view.cwd.as_deref()?;
    let program = view.argv.first()?;
    Some(format!(
        "{PROCESS_V1}{cwd}{SEPARATOR}{program}{SEPARATOR}{}",
        view.argv.join(" ")
    ))
}

/// Whether any stored rule matches `view`. Both the current and the legacy key
/// are exact lookups: a rule never matches more than it names.
#[must_use]
pub fn is_ignored(rules: &BTreeSet<String>, view: &InstanceView, context: &str) -> bool {
    rule_key(view, context).is_some_and(|key| rules.contains(&key))
        || legacy_key(view).is_some_and(|key| rules.contains(&key))
}

/// The stable handle of a stored key.
#[must_use]
pub fn rule_id(key: &str) -> String {
    crate::hash::sha256_hex(key.as_bytes())
        .chars()
        .take(32)
        .collect()
}

/// Describes a stored key, including ones this build cannot read. `created_at`
/// is seconds since the epoch, `0` when there is no saved rule yet.
#[must_use]
pub fn describe(key: &str, created_at: i64) -> IgnoreRule {
    let mut rule = IgnoreRule {
        id: rule_id(key),
        kind: IgnoreKind::Unknown,
        folder: None,
        command: Vec::new(),
        container: None,
        context: None,
        legacy: false,
        created_at: (created_at > 0)
            .then(|| jiff::Timestamp::from_second(created_at).ok())
            .flatten()
            .map(|moment| moment.to_string()),
    };
    if let Some(json) = key.strip_prefix(PROCESS_V2) {
        if let Ok(criterion) = serde_json::from_str::<ProcessCriterion>(json) {
            rule.kind = IgnoreKind::Process;
            rule.folder = Some(criterion.cwd);
            rule.command = criterion.argv;
        }
    } else if let Some(json) = key.strip_prefix(CONTAINER_V2) {
        if let Ok(criterion) = serde_json::from_str::<ContainerCriterion>(json) {
            rule.kind = IgnoreKind::Container;
            rule.container = Some(criterion.name);
            rule.context = Some(criterion.context);
        }
    } else if let Some(rest) = key.strip_prefix(CONTAINER_V1) {
        rule.kind = IgnoreKind::Container;
        rule.container = Some(rest.to_owned());
        rule.legacy = true;
    } else if let Some(rest) = key.strip_prefix(PROCESS_V1) {
        let mut parts = rest.splitn(3, SEPARATOR);
        if let (Some(cwd), Some(_program), Some(command)) =
            (parts.next(), parts.next(), parts.next())
        {
            rule.kind = IgnoreKind::Process;
            rule.folder = Some(cwd.to_owned());
            rule.command = command.split_whitespace().map(str::to_owned).collect();
            rule.legacy = true;
        }
    }
    rule
}

#[cfg(test)]
mod tests;
