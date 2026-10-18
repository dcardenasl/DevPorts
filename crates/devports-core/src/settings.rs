//! The scan folders of `config.yml`, edited from the panel (ADR-035).
//!
//! Only `roots` is editable here. The file is edited as text so every other key and
//! comment survives, the candidate must parse with the same reader the core uses and
//! differ from the current configuration in `roots` alone, and a change made by
//! somebody else after the person reviewed it is detected rather than overwritten.
//! Removing a folder only stops looking in it: no file is deleted and no process stopped.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{Config, config_dir};
use crate::discovery::{discover, expand_tilde};
use crate::document::{EditError, FileSnapshot};

/// Most folders the panel will write.
pub const MAX_ROOTS: usize = 32;
/// Longest folder text accepted.
const MAX_ROOT_BYTES: usize = 1024;

/// Why the scan folders cannot be read or saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum SettingsError {
    /// The configuration cannot be edited as it is; nothing was changed.
    #[error("{message}")]
    Ineligible {
        /// Why, with the file's own problem when it has one.
        message: String,
    },
    /// One or more of the folders typed is not acceptable.
    #[error("hay carpetas no válidas")]
    InvalidRoots {
        /// Each problem with the position of its folder in the submitted list.
        problems: Vec<RootProblem>,
    },
    /// The list as a whole is not acceptable (empty or too long).
    #[error("{message}")]
    InvalidList {
        /// What is wrong with it.
        message: String,
    },
    /// `config.yml` changed after it was reviewed; review it again.
    #[error("{path} cambió desde la revisión; vuelve a revisarlo")]
    DocumentChanged {
        /// The configuration file.
        path: String,
    },
    /// The file writes `roots` in a form DevPorts does not edit.
    #[error("{message}")]
    Unsupported {
        /// Why it cannot be edited safely.
        message: String,
    },
    /// A file or worker operation failed.
    #[error("{message}")]
    Io {
        /// OS or runtime explanation.
        message: String,
    },
}

/// A folder the person typed that cannot be used, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RootProblem {
    /// Position in the submitted list.
    pub index: u32,
    /// Reason to display beside the folder.
    pub message: String,
}

/// One folder projects are searched in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ScanRoot {
    /// As written in `config.yml` (may start with `~`).
    pub configured: String,
    /// The folder it resolves to.
    pub path: String,
    /// Whether it exists as a folder right now.
    pub exists: bool,
    /// Projects found inside it with the current depth.
    pub projects: u32,
}

/// The scan folders in force and whether they can be edited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ScanFolders {
    /// `config.yml`.
    pub config_path: String,
    /// The folders, in order.
    pub roots: Vec<ScanRoot>,
    /// `config.yml` does not set `roots`, so these are the defaults.
    pub from_defaults: bool,
    /// How many levels below a folder are searched.
    pub max_depth: u32,
    /// Whether the panel can edit them now.
    pub editable: bool,
    /// Why not, when it cannot.
    pub reason: Option<String>,
    /// Version token of the reviewed file; send it back with the new list.
    pub base: String,
}

/// What saving a list would change in discovery, before anything is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ScanPreview {
    /// The validated folders with what each contains.
    pub roots: Vec<ScanRoot>,
    /// Projects that would start to be listed.
    pub added: Vec<String>,
    /// Projects that would stop being listed (their files and processes are untouched).
    pub removed: Vec<String>,
    /// The list equals the current one, so saving writes nothing.
    pub unchanged: bool,
}

/// What a save did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsOutcome {
    /// `config.yml` was rewritten.
    Saved,
    /// The list already was this one.
    Unchanged,
}

fn config_file(home: &Path) -> PathBuf {
    config_dir(home).join("config.yml")
}

fn token(text: &str) -> String {
    crate::hash::sha256_hex(text.as_bytes())
}

#[derive(Deserialize, Default)]
struct RawRoots {
    roots: Option<Vec<String>>,
}

struct Current {
    snapshot: FileSnapshot,
    text: String,
    config: Config,
    configured: Option<Vec<String>>,
}

fn load(home: &Path) -> Result<Current, SettingsError> {
    let path = config_file(home);
    let snapshot = FileSnapshot::capture(&path).map_err(edit_error)?;
    let existing = snapshot.text().map_err(edit_error)?.map(str::to_owned);
    let config = match &existing {
        None => Config::defaults(home),
        Some(text) => {
            Config::parse(text, &path, home)
                .map_err(|error| SettingsError::Ineligible {
                    message: format!(
                        "config.yml tiene errores; corrígelos a mano antes de editar: {error}"
                    ),
                })?
                .0
        }
    };
    let text = existing.unwrap_or_default();
    let configured = serde_saphyr::from_str::<RawRoots>(&text)
        .ok()
        .and_then(|raw| raw.roots);
    Ok(Current {
        snapshot,
        text,
        config,
        configured,
    })
}

fn root_view(configured: &str, path: &Path, depth: usize) -> ScanRoot {
    let exists = path.is_dir();
    let projects = if exists {
        u32::try_from(
            discover(&[path.to_path_buf()], depth)
                .catalog
                .projects()
                .len(),
        )
        .unwrap_or(u32::MAX)
    } else {
        0
    };
    ScanRoot {
        configured: configured.to_owned(),
        path: path.display().to_string(),
        exists,
        projects,
    }
}

/// Reads the scan folders without writing. A configuration that cannot be edited is
/// still reported, with the reason, so the panel can show what is in force.
///
/// # Errors
/// Returns [`SettingsError::Io`] when the file cannot be read at all.
pub fn read(home: &Path) -> Result<ScanFolders, SettingsError> {
    let path = config_file(home);
    match load(home) {
        Ok(current) => {
            let depth = current.config.max_depth;
            let configured: Vec<String> = current.configured.clone().unwrap_or_else(|| {
                current
                    .config
                    .roots
                    .iter()
                    .map(|root| root.display().to_string())
                    .collect()
            });
            let roots = configured
                .iter()
                .zip(&current.config.roots)
                .map(|(configured, root)| root_view(configured, root, depth))
                .collect();
            let base = token(&current.text);
            let editable = edit_roots(&current.text, &configured).is_ok();
            Ok(ScanFolders {
                config_path: path.display().to_string(),
                roots,
                from_defaults: current.configured.is_none(),
                max_depth: u32::try_from(depth).unwrap_or(u32::MAX),
                editable,
                reason: (!editable).then(|| {
                    "config.yml escribe «roots» en una forma que DevPorts no edita; edítalo a mano"
                        .to_owned()
                }),
                base,
            })
        }
        Err(SettingsError::Ineligible { message }) => {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            Ok(ScanFolders {
                config_path: path.display().to_string(),
                roots: Vec::new(),
                from_defaults: false,
                max_depth: u32::try_from(crate::discovery::DEFAULT_MAX_DEPTH).unwrap_or(0),
                editable: false,
                reason: Some(message),
                base: token(&text),
            })
        }
        Err(error) => Err(error),
    }
}

/// Validates a candidate list against what is in force, with no writes. New folders
/// must exist; folders already configured are kept even if they have vanished, so a
/// disconnected drive does not make the rest of the list uneditable.
fn validate(
    home: &Path,
    current: &[PathBuf],
    roots: &[String],
) -> Result<Vec<PathBuf>, SettingsError> {
    if roots.is_empty() {
        return Err(SettingsError::InvalidList {
            message: "deja al menos una carpeta: sin ellas no se descubriría ningún proyecto"
                .to_owned(),
        });
    }
    if roots.len() > MAX_ROOTS {
        return Err(SettingsError::InvalidList {
            message: format!("como máximo {MAX_ROOTS} carpetas"),
        });
    }
    let current: Vec<PathBuf> = current
        .iter()
        .map(|root| root.components().collect())
        .collect();
    let mut problems = Vec::new();
    let mut expanded: Vec<PathBuf> = Vec::new();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    for (index, root) in roots.iter().enumerate() {
        let position = u32::try_from(index).unwrap_or(u32::MAX);
        let mut problem = |message: &str| {
            problems.push(RootProblem {
                index: position,
                message: message.to_owned(),
            });
        };
        let root = root.trim();
        if root.is_empty() {
            problem("escribe una carpeta");
            continue;
        }
        if root.len() > MAX_ROOT_BYTES
            || root
                .chars()
                .any(|character| character.is_control() || character == '"' || character == '\\')
        {
            problem(
                "la ruta contiene caracteres que no se admiten (comillas, barras invertidas o saltos de línea)",
            );
            continue;
        }
        let path = expand_tilde(root, home);
        if !path.is_absolute() {
            problem("usa una ruta absoluta o que empiece por ~/");
            continue;
        }
        let path: PathBuf = path.components().collect();
        if path == home || path.parent().is_none() || path.components().count() <= 2 {
            problem("esa carpeta es demasiado amplia; elige la que contiene tus proyectos");
            continue;
        }
        let known = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !seen.insert(known) {
            problem("esa carpeta ya está en la lista");
            continue;
        }
        if !current.contains(&path) && !path.is_dir() {
            problem("la carpeta no existe");
            continue;
        }
        expanded.push(path);
    }
    if problems.is_empty() {
        Ok(expanded)
    } else {
        Err(SettingsError::InvalidRoots { problems })
    }
}

/// What saving `roots` would do to discovery. Writes nothing.
///
/// # Errors
/// Returns the validation or eligibility problem that would stop a save.
pub fn preview(home: &Path, roots: &[String]) -> Result<ScanPreview, SettingsError> {
    let current = load(home)?;
    let candidate = validate(home, &current.config.roots, roots)?;
    let depth = current.config.max_depth;
    let names = |paths: &[PathBuf]| -> BTreeSet<String> {
        discover(paths, depth)
            .catalog
            .projects()
            .iter()
            .map(|project| project.name.clone())
            .collect()
    };
    let before = names(&current.config.roots);
    let after = names(&candidate);
    Ok(ScanPreview {
        roots: roots
            .iter()
            .zip(&candidate)
            .map(|(configured, path)| root_view(configured.trim(), path, depth))
            .collect(),
        added: after.difference(&before).cloned().collect(),
        removed: before.difference(&after).cloned().collect(),
        unchanged: candidate == current.config.roots,
    })
}

/// Saves `roots` if `base` is still the version of `config.yml` that was reviewed.
///
/// # Errors
/// Returns a typed validation, conflict or I/O error; nothing is written in any of them.
pub fn apply(home: &Path, roots: &[String], base: &str) -> Result<SettingsOutcome, SettingsError> {
    let current = load(home)?;
    let candidate = validate(home, &current.config.roots, roots)?;
    // Already the requested list: nothing to write, whatever the version.
    if candidate == current.config.roots && current.configured.is_some() {
        return Ok(SettingsOutcome::Unchanged);
    }
    if token(&current.text) != base {
        return Err(SettingsError::DocumentChanged {
            path: config_file(home).display().to_string(),
        });
    }
    let written: Vec<String> = roots.iter().map(|root| root.trim().to_owned()).collect();
    let edited = edit_roots(&current.text, &written)?;
    let path = config_file(home);
    let after = Config::parse(&edited, &path, home)
        .map_err(|error| SettingsError::Unsupported {
            message: format!("la edición no produjo una configuración válida: {error}"),
        })?
        .0;
    let mut expected = current.config.clone();
    expected.roots = candidate;
    if after != expected {
        return Err(SettingsError::Unsupported {
            message: "la edición habría cambiado algo más que «roots»; edítalo a mano".to_owned(),
        });
    }
    current.snapshot.commit(&edited).map_err(edit_error)?;
    Ok(SettingsOutcome::Saved)
}

fn edit_error(error: EditError) -> SettingsError {
    match error {
        EditError::Conflict(path) => SettingsError::DocumentChanged {
            path: path.display().to_string(),
        },
        EditError::ReadOnly(path) => SettingsError::Ineligible {
            message: format!("{} es de solo lectura", path.display()),
        },
        EditError::Unsupported(message) => SettingsError::Unsupported { message },
        EditError::Io(message) => SettingsError::Io { message },
    }
}

fn unsupported(message: &str) -> SettingsError {
    SettingsError::Unsupported {
        message: message.to_owned(),
    }
}

fn quote(root: &str) -> String {
    format!("\"{root}\"")
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

/// One list item: plain, `"double"` or `'single'` quoted, with no escapes or nesting.
fn item(text: &str) -> Result<String, SettingsError> {
    let text = text.trim();
    let inner = match text.chars().next() {
        Some(quote @ ('"' | '\'')) => text
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
            .ok_or_else(|| unsupported("«roots» tiene un elemento que DevPorts no edita"))?,
        _ => text,
    };
    if inner.is_empty()
        || inner.chars().any(|c| {
            matches!(
                c,
                '"' | '\'' | '\\' | '[' | ']' | '{' | '}' | '&' | '*' | '!' | '|' | '>' | ',' | '#'
            )
        })
    {
        return Err(unsupported(
            "«roots» tiene valores que DevPorts no edita (anclas, escapes o estructuras)",
        ));
    }
    Ok(inner.to_owned())
}

fn flow_list(roots: &[String]) -> String {
    roots
        .iter()
        .map(|root| quote(root))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Replaces the top-level `roots` with `roots`, changing nothing else.
///
/// Supported forms: no key, a one-line flow list and a block list of plain or quoted
/// folders. Comments, other keys and line endings are kept as written.
fn edit_roots(text: &str, roots: &[String]) -> Result<String, SettingsError> {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
    match lines.iter().position(|line| line.starts_with("roots:")) {
        None => insert_key(&mut lines, roots, newline),
        Some(index) => {
            let line = lines
                .get(index)
                .ok_or_else(|| unsupported("no se encontró «roots»"))?;
            let (content, eol) = split_eol(line);
            let rest = content.trim_start_matches("roots:");
            let (value, comment) = split_comment(rest);
            let trimmed = value.trim();
            if trimmed.starts_with('[') {
                let replacement = flow_line(rest, value, comment, eol, roots)?;
                *lines
                    .get_mut(index)
                    .ok_or_else(|| unsupported("no se encontró «roots»"))? = replacement;
            } else if trimmed.is_empty() {
                replace_block(&mut lines, index, roots, newline)?;
            } else {
                return Err(unsupported(
                    "«roots» no es una lista de carpetas; edítalo a mano",
                ));
            }
        }
    }
    Ok(lines.concat())
}

fn insert_key(lines: &mut Vec<String>, roots: &[String], newline: &str) {
    let at = lines
        .iter()
        .position(|line| line.starts_with("version:"))
        .map_or(0, |version| version.saturating_add(1));
    if let Some(line) = at
        .checked_sub(1)
        .and_then(|previous| lines.get_mut(previous))
        && !line.ends_with('\n')
    {
        line.push_str(newline);
    }
    let header = if lines.is_empty() {
        format!("version: 1{newline}")
    } else {
        String::new()
    };
    lines.insert(
        at.min(lines.len()),
        format!("{header}roots: [{}]{newline}", flow_list(roots)),
    );
}

/// `roots: [..]` rewritten in place, keeping the spacing and the trailing comment.
fn flow_line(
    rest: &str,
    value: &str,
    comment: &str,
    eol: &str,
    roots: &[String],
) -> Result<String, SettingsError> {
    let inner = value
        .trim()
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .ok_or_else(|| unsupported("«roots» abarca varias líneas; edítalo a mano"))?;
    for part in inner.split(',').filter(|part| !part.trim().is_empty()) {
        item(part)?;
    }
    let lead = rest.get(..rest.len().saturating_sub(rest.trim_start().len()));
    let gap = value.get(value.trim_end().len()..).unwrap_or_default();
    Ok(format!(
        "roots:{}[{}]{gap}{comment}{eol}",
        lead.filter(|lead| !lead.is_empty()).unwrap_or(" "),
        flow_list(roots),
    ))
}

/// Block form: the item lines are replaced where the first one was; comments around
/// them stay.
fn replace_block(
    lines: &mut Vec<String>,
    index: usize,
    roots: &[String],
    newline: &str,
) -> Result<(), SettingsError> {
    let mut item_lines: Vec<usize> = Vec::new();
    let mut indent = None;
    let mut at = index.saturating_add(1);
    while let Some(next) = lines.get(at) {
        let (content, _) = split_eol(next);
        let body = content.trim();
        if body.is_empty() || body.starts_with('#') {
            at = at.saturating_add(1);
            continue;
        }
        let width = content.len().saturating_sub(content.trim_start().len());
        if width == 0 {
            break;
        }
        let (entry, _) = split_comment(body);
        let value = entry
            .strip_prefix('-')
            .filter(|value| value.starts_with([' ', '\t']))
            .ok_or_else(|| unsupported("«roots» no es una lista de carpetas simples"))?;
        if indent.is_some_and(|first| first != width) {
            return Err(unsupported("«roots» mezcla sangrías; edítalo a mano"));
        }
        indent = Some(width);
        item(value)?;
        item_lines.push(at);
        at = at.saturating_add(1);
    }
    let pad = " ".repeat(indent.unwrap_or(2));
    let insert_at = item_lines
        .first()
        .copied()
        .unwrap_or_else(|| index.saturating_add(1));
    for position in item_lines.into_iter().rev() {
        lines.remove(position);
    }
    if let Some(previous) = insert_at
        .checked_sub(1)
        .and_then(|previous| lines.get_mut(previous))
        && !previous.ends_with('\n')
    {
        previous.push_str(newline);
    }
    for (offset, root) in roots.iter().enumerate() {
        lines.insert(
            insert_at.saturating_add(offset),
            format!("{pad}- {}{newline}", quote(root)),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
