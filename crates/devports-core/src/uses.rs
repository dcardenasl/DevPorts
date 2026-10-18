//! Declared consumers of a shared service: editing `uses:` in project manifests.
//!
//! A project "uses" a shared service because its manifest says so; nothing here
//! observes real connections. Only the application calls this, on a person's
//! confirmation (ADR-009, ADR-030); no socket operation reaches it. Every
//! manifest is validated and every candidate built before the first byte is
//! written. Writes then happen file by file with an independent result each: a
//! failure is reported, never rolled back over somebody else's edit.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::document::{EditError, FileSnapshot};
use crate::manifest::{self, MANIFEST_FILE, is_valid_name};
use crate::reasons::{DocumentKind, FormKey, FormProblem, Ineligibility, Limitation};

/// Most projects one save may touch.
pub const MAX_REQUESTS: usize = 64;

/// Why a consumer cannot be listed or saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum UsesError {
    /// The shared service is not defined in `shared.yml`.
    #[error("«{name}» no está definido en shared.yml; decláralo antes de asociarlo")]
    UnknownShared {
        /// The missing shared service.
        name: String,
    },
    /// The catalog has no such project any more.
    #[error("el proyecto «{project}» ya no existe")]
    UnknownProject {
        /// The missing project.
        project: String,
    },
    /// The manifest is absent, invalid or read-only: nothing was changed.
    #[error("{reason}")]
    Ineligible {
        /// Why, to be written beside the project.
        reason: Ineligibility,
    },
    /// The manifest changed after it was reviewed; review it again.
    #[error("{path} cambió desde la revisión; vuelve a revisarlo")]
    DocumentChanged {
        /// The affected manifest.
        path: String,
    },
    /// The manifest writes `uses` in a form DevPorts does not edit.
    #[error("{limitation}")]
    Unsupported {
        /// Why it cannot be edited safely.
        limitation: Limitation,
    },
    /// Another project of the same save failed validation, so nothing was written.
    #[error("no se escribió nada: otro proyecto de esta selección no pasó la validación")]
    Blocked,
    /// The selection repeats a project, is too long or names an invalid shared service.
    #[error("la selección repite un proyecto o supera el máximo")]
    InvalidRequest,
    /// DevPorts is closing, so the request was not served.
    #[error("DevPorts se está cerrando")]
    Closing,
    /// A file or worker operation failed.
    #[error("{detail}")]
    Io {
        /// OS or runtime explanation, as it said it.
        detail: String,
    },
}

/// One project as a possible consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConsumerChoice {
    /// Project name.
    pub project: String,
    /// Canonical project folder.
    pub root: String,
    /// Canonical manifest path.
    pub manifest: String,
    /// Whether the manifest lists the shared service in `uses`.
    pub declared: bool,
    /// Whether the manifest can currently be edited.
    pub eligible: bool,
    /// Why it cannot be edited, instead of hiding the project.
    pub reason: Option<UsesError>,
    /// Version token of the reviewed content; send it back with the request.
    pub base: Option<String>,
}

/// Every project and whether it declares one shared service. Reading writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConsumerOptions {
    /// The shared service.
    pub shared: String,
    /// Whether `shared.yml` defines it right now.
    pub defined: bool,
    /// Projects ordered by name.
    pub consumers: Vec<ConsumerChoice>,
}

/// The confirmed state of one project: declare the service or stop declaring it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UseRequest {
    /// Project name.
    pub project: String,
    /// `true` to list the shared service in `uses`, `false` to remove it.
    pub include: bool,
    /// The [`ConsumerChoice::base`] the person reviewed.
    pub base: String,
}

/// What a successful request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseOutcome {
    /// The manifest was rewritten.
    Saved,
    /// It already said so; nothing was written and nothing was duplicated.
    Unchanged,
}

/// The result for one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseReport {
    /// Project name.
    pub project: String,
    /// Its outcome, independent of the others.
    pub result: Result<UseOutcome, UsesError>,
}

/// A validated candidate, ready to commit.
struct Plan {
    snapshot: FileSnapshot,
    edited: Option<String>,
}

/// Version token of a manifest's content.
fn token(text: &str) -> String {
    crate::hash::sha256_hex(text.as_bytes())
}

/// Reads one project's state without writing.
#[must_use]
pub fn inspect(
    project: &str,
    root: &Path,
    shared: &str,
    known_shared: &BTreeSet<String>,
) -> ConsumerChoice {
    let path = root.join(MANIFEST_FILE);
    let mut choice = ConsumerChoice {
        project: project.to_owned(),
        root: root.display().to_string(),
        manifest: path.display().to_string(),
        declared: false,
        eligible: false,
        reason: None,
        base: None,
    };
    let loaded = FileSnapshot::capture(&path)
        .map_err(edit_error)
        .and_then(|snapshot| {
            let text = snapshot.text().map_err(edit_error)?.map(str::to_owned);
            Ok((snapshot, text))
        });
    match loaded {
        Err(error) => choice.reason = Some(error),
        Ok((_, None)) => {
            choice.reason = Some(UsesError::Ineligible {
                reason: Ineligibility::NoManifest,
            });
        }
        Ok((_, Some(text))) => match manifest::parse_checked(&text, &path, Some(known_shared)) {
            Err(error) => {
                choice.reason = Some(UsesError::Ineligible {
                    reason: Ineligibility::ManifestInvalid {
                        problem: error.into(),
                    },
                });
            }
            Ok(loaded) => {
                choice.declared = loaded.manifest.uses.iter().any(|name| name == shared);
                choice.base = Some(token(&text));
                // A form DevPorts cannot edit is explained now, not at save time.
                match edit_uses(&text, shared, !choice.declared) {
                    Ok(_) => choice.eligible = true,
                    Err(error) => choice.reason = Some(error),
                }
            }
        },
    }
    choice
}

/// Validates every request first; writes only if all of them pass.
///
/// Reports one result per request, in order. A validation failure anywhere
/// blocks every write (the failing projects keep their own reason). Once
/// writing starts, each file succeeds or fails alone and nothing is reverted.
#[must_use]
pub fn apply(
    shared: &str,
    known_shared: &BTreeSet<String>,
    projects: &[(String, PathBuf)],
    requests: &[UseRequest],
) -> Vec<UseReport> {
    apply_with(
        shared,
        known_shared,
        projects,
        requests,
        FileSnapshot::commit,
    )
}

pub(crate) fn apply_with(
    shared: &str,
    known_shared: &BTreeSet<String>,
    projects: &[(String, PathBuf)],
    requests: &[UseRequest],
    mut commit: impl FnMut(&FileSnapshot, &str) -> Result<(), EditError>,
) -> Vec<UseReport> {
    let planned: Vec<(String, Result<Plan, UsesError>)> = requests
        .iter()
        .map(|request| {
            (
                request.project.clone(),
                validate_request(shared, known_shared, projects, requests, request),
            )
        })
        .collect();
    if planned.iter().any(|(_, plan)| plan.is_err()) {
        return planned
            .into_iter()
            .map(|(project, plan)| UseReport {
                project,
                result: match plan {
                    Err(error) => Err(error),
                    Ok(_) => Err(UsesError::Blocked),
                },
            })
            .collect();
    }
    planned
        .into_iter()
        .map(|(project, plan)| {
            let result = plan.and_then(|plan| match plan.edited {
                None => Ok(UseOutcome::Unchanged),
                Some(text) => commit(&plan.snapshot, &text)
                    .map(|()| UseOutcome::Saved)
                    .map_err(edit_error),
            });
            UseReport { project, result }
        })
        .collect()
}

fn validate_request(
    shared: &str,
    known_shared: &BTreeSet<String>,
    projects: &[(String, PathBuf)],
    requests: &[UseRequest],
    request: &UseRequest,
) -> Result<Plan, UsesError> {
    if requests.len() > MAX_REQUESTS
        || requests
            .iter()
            .filter(|other| other.project == request.project)
            .count()
            > 1
        || !is_valid_name(shared)
    {
        return Err(UsesError::InvalidRequest);
    }
    if request.include && !known_shared.contains(shared) {
        return Err(UsesError::UnknownShared {
            name: shared.to_owned(),
        });
    }
    let root = projects
        .iter()
        .find(|(name, _)| *name == request.project)
        .map(|(_, root)| root)
        .ok_or_else(|| UsesError::UnknownProject {
            project: request.project.clone(),
        })?;
    plan(root, known_shared, shared, request)
}

fn plan(
    root: &Path,
    known_shared: &BTreeSet<String>,
    shared: &str,
    request: &UseRequest,
) -> Result<Plan, UsesError> {
    let path = root.join(MANIFEST_FILE);
    let snapshot = FileSnapshot::capture(&path).map_err(edit_error)?;
    let text = snapshot
        .text()
        .map_err(edit_error)?
        .ok_or(UsesError::Ineligible {
            reason: Ineligibility::NoManifest,
        })?;
    let before = manifest::parse_checked(text, &path, Some(known_shared))
        .map_err(|error| UsesError::Ineligible {
            reason: Ineligibility::ManifestInvalid {
                problem: error.into(),
            },
        })?
        .manifest;
    let declared = before.uses.iter().any(|name| name == shared);
    // Already in the requested state: nothing to write, whatever the version.
    if declared == request.include {
        return Ok(Plan {
            snapshot,
            edited: None,
        });
    }
    if request.base != token(text) {
        return Err(UsesError::DocumentChanged {
            path: snapshot.path().display().to_string(),
        });
    }
    let edited = edit_uses(text, shared, request.include)?;
    let after = manifest::parse_checked(&edited, &path, Some(known_shared))
        .map_err(|error| UsesError::Unsupported {
            limitation: Limitation::EditInvalid {
                document: DocumentKind::Manifest,
                problem: error.into(),
            },
        })?
        .manifest;
    let mut expected = before.clone();
    expected.uses.retain(|name| name != shared);
    if request.include {
        expected.uses.push(shared.to_owned());
    }
    if after != expected {
        return Err(UsesError::Unsupported {
            limitation: Limitation::EditTouchedMore { key: FormKey::Uses },
        });
    }
    Ok(Plan {
        snapshot,
        edited: Some(edited),
    })
}

fn edit_error(error: EditError) -> UsesError {
    match error {
        EditError::Conflict(path) => UsesError::DocumentChanged {
            path: path.display().to_string(),
        },
        EditError::ReadOnly(path) => UsesError::Ineligible {
            reason: Ineligibility::read_only(&path),
        },
        EditError::Unsupported(limitation) => UsesError::Unsupported { limitation },
        EditError::Io(detail) => UsesError::Io { detail },
    }
}

/// The `uses` key is written in a way that cannot be edited.
fn unsupported(problem: FormProblem) -> UsesError {
    unsupported_key(FormKey::Uses, problem)
}

fn unsupported_key(key: FormKey, problem: FormProblem) -> UsesError {
    UsesError::Unsupported {
        limitation: Limitation::Form { key, problem },
    }
}

/// Adds or removes `name` in the top-level `uses`, changing nothing else.
///
/// Supported forms: no key, a one-line flow list (`uses: [a, b]`) and a block
/// list of plain names, with comments and line endings kept as written.
fn edit_uses(text: &str, name: &str, include: bool) -> Result<String, UsesError> {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
    let key = lines.iter().position(|line| line.starts_with("uses:"));
    match key {
        None if !include => Ok(text.to_owned()),
        None => {
            insert_key(&mut lines, name, newline)?;
            Ok(lines.concat())
        }
        Some(index) => {
            edit_existing(&mut lines, index, name, include, newline)?;
            Ok(lines.concat())
        }
    }
}

fn split_eol(line: &str) -> (&str, &str) {
    let content = line.trim_end_matches(['\n', '\r']);
    line.split_at_checked(content.len()).unwrap_or((line, ""))
}

/// Splits `rest` at a YAML comment (`#` at the start or after whitespace).
fn split_comment(rest: &str) -> (&str, &str) {
    let cut = rest.char_indices().find_map(|(at, character)| {
        let after_space = rest
            .get(..at)
            .is_some_and(|before| before.is_empty() || before.ends_with(char::is_whitespace));
        (character == '#' && after_space).then_some(at)
    });
    cut.and_then(|at| rest.split_at_checked(at))
        .unwrap_or((rest, ""))
}

fn insert_key(lines: &mut Vec<String>, name: &str, newline: &str) -> Result<(), UsesError> {
    let project = lines
        .iter()
        .position(|line| line.starts_with("project:"))
        .ok_or_else(|| unsupported_key(FormKey::Project, FormProblem::NotFound))?;
    let continues = lines
        .get(project.saturating_add(1))
        .is_some_and(|next| next.starts_with([' ', '\t']) && !next.trim().is_empty());
    let single = lines.get(project).is_some_and(|line| {
        let (content, _) = split_eol(line);
        let (value, _) = split_comment(content.trim_start_matches("project:"));
        let value = value.trim();
        !value.is_empty() && !value.starts_with(['|', '>'])
    });
    if continues || !single {
        return Err(unsupported_key(FormKey::Project, FormProblem::MultiLine));
    }
    if let Some(line) = lines.get_mut(project)
        && !line.ends_with('\n')
    {
        line.push_str(newline);
    }
    lines.insert(
        project.saturating_add(1),
        format!("uses: [{name}]{newline}"),
    );
    Ok(())
}

fn edit_existing(
    lines: &mut Vec<String>,
    index: usize,
    name: &str,
    include: bool,
    newline: &str,
) -> Result<(), UsesError> {
    let Some(line) = lines.get(index) else {
        return Err(unsupported(FormProblem::NotFound));
    };
    let (content, eol) = split_eol(line);
    let rest = content.trim_start_matches("uses:");
    let (value, comment) = split_comment(rest);
    let trimmed = value.trim();
    if trimmed.starts_with('[') {
        let flow = trimmed
            .strip_prefix('[')
            .and_then(|inner| inner.strip_suffix(']'))
            .ok_or_else(|| unsupported(FormProblem::MultiLine))?;
        let mut names = plain_names(flow.split(','), true)?;
        update_names(&mut names, name, include);
        let lead = rest.get(..rest.len().saturating_sub(rest.trim_start().len()));
        let gap = value.get(value.trim_end().len()..).unwrap_or_default();
        let replacement = format!(
            "uses:{}[{}]{gap}{comment}{eol}",
            lead.filter(|lead| !lead.is_empty()).unwrap_or(" "),
            names.join(", "),
        );
        *lines
            .get_mut(index)
            .ok_or_else(|| unsupported(FormProblem::NotFound))? = replacement;
        return Ok(());
    }
    if !trimmed.is_empty() {
        return Err(unsupported(FormProblem::NotAList));
    }
    let comment = comment.to_owned();
    edit_block(lines, index, &comment, name, include, newline)
}

/// Block form: `uses:` followed by indented `- name` items.
fn edit_block(
    lines: &mut Vec<String>,
    index: usize,
    comment: &str,
    name: &str,
    include: bool,
    newline: &str,
) -> Result<(), UsesError> {
    let mut items: Vec<(usize, String)> = Vec::new();
    let mut indent = None;
    let mut at = index.saturating_add(1);
    while let Some(line) = lines.get(at) {
        let (content, _) = split_eol(line);
        let trimmed = content.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            at = at.saturating_add(1);
            continue;
        }
        let width = content.len().saturating_sub(content.trim_start().len());
        if width == 0 {
            break;
        }
        let (entry, _) = split_comment(trimmed);
        let value = entry
            .strip_prefix('-')
            .filter(|value| value.starts_with([' ', '\t']))
            .ok_or_else(|| unsupported(FormProblem::NotSimpleList))?;
        if indent.is_some_and(|first| first != width) {
            return Err(unsupported(FormProblem::MixedIndent));
        }
        indent = Some(width);
        let parsed = plain_names(std::iter::once(value), false)?;
        items.extend(parsed.into_iter().map(|parsed| (at, parsed)));
        at = at.saturating_add(1);
    }
    let mut names: Vec<String> = items.iter().map(|(_, item)| item.clone()).collect();
    let present = names.iter().any(|item| item == name);
    update_names(&mut names, name, include);
    if include {
        if present {
            return Ok(());
        }
        let last = items.last().map_or(index, |(at, _)| *at);
        if let Some(line) = lines.get_mut(last)
            && !line.ends_with('\n')
        {
            line.push_str(newline);
        }
        let pad = " ".repeat(indent.unwrap_or(2));
        lines.insert(last.saturating_add(1), format!("{pad}- {name}{newline}"));
        return Ok(());
    }
    // Remove every matching item line (back to front keeps indices valid).
    let mut doomed: Vec<usize> = items
        .iter()
        .filter(|(_, item)| item == name)
        .map(|(at, _)| *at)
        .collect();
    doomed.sort_unstable();
    for at in doomed.into_iter().rev() {
        lines.remove(at);
    }
    if names.is_empty() {
        let eol = lines
            .get(index)
            .map_or("", |line| split_eol(line).1)
            .to_owned();
        let gap = if comment.is_empty() { "" } else { " " };
        if let Some(line) = lines.get_mut(index) {
            *line = format!("uses: []{gap}{comment}{eol}");
        }
    }
    Ok(())
}

/// Parses plain names (no quotes, anchors or nesting) from list items.
fn plain_names<'a>(
    items: impl Iterator<Item = &'a str>,
    allow_empty: bool,
) -> Result<Vec<String>, UsesError> {
    let mut names = Vec::new();
    for item in items {
        let item = item.trim();
        if item.is_empty() {
            if allow_empty {
                continue;
            }
            return Err(unsupported(FormProblem::EmptyItem));
        }
        if !is_valid_name(item) {
            return Err(unsupported(FormProblem::InvalidValue));
        }
        names.push(item.to_owned());
    }
    Ok(names)
}

fn update_names(names: &mut Vec<String>, name: &str, include: bool) {
    names.retain(|existing| existing != name);
    if include {
        names.push(name.to_owned());
    }
}

#[cfg(test)]
mod tests;
