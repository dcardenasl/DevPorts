//! Optimistic, bounded text-file edits shared by human configuration operations.
//!
//! Application writers serialize only their filesystem commit sections. External
//! editors need not cooperate: detectable changes cause a conflict, but a final
//! check followed by rename is not an OS compare-and-swap against another editor.

use std::fs::{File, Metadata, OpenOptions, Permissions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

/// Maximum editable document size; oversized files remain available for manual editing.
pub const MAX_DOCUMENT_BYTES: usize = 262_144;

// This holds no domain state. It serializes filesystem compare/write sections,
// including legacy callers, without locking the actor or waiting on network I/O.
static WRITES: Mutex<()> = Mutex::new(());

/// Why an optimistic text edit cannot proceed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// The target changed since its captured version.
    #[error("{} cambió desde la revisión; vuelve a leerlo antes de guardar", .0.display())]
    Conflict(PathBuf),
    /// An existing file explicitly forbids writing.
    #[error("{} es de solo lectura", .0.display())]
    ReadOnly(PathBuf),
    /// The shape or size is not supported for safe automatic editing.
    #[error("{0}")]
    Unsupported(String),
    /// An operating system operation failed before committing the edit.
    #[error("{0}")]
    Io(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    uid: u32,
    #[cfg(unix)]
    gid: u32,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanos: i64,
}

impl Stamp {
    fn of(metadata: &Metadata) -> Self {
        Self {
            modified: metadata.modified().ok(),
            len: metadata.len(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(unix)]
            mode: metadata.mode(),
            #[cfg(unix)]
            uid: metadata.uid(),
            #[cfg(unix)]
            gid: metadata.gid(),
            #[cfg(unix)]
            changed_seconds: metadata.ctime(),
            #[cfg(unix)]
            changed_nanos: metadata.ctime_nsec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectoryIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    created: Option<SystemTime>,
}

impl DirectoryIdentity {
    fn of(metadata: &Metadata) -> Self {
        Self {
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            created: metadata.created().ok(),
        }
    }
}

/// An exact, read-only file version, including absence and original permissions.
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
    stamp: Option<Stamp>,
    permissions: Option<Permissions>,
    anchor: PathBuf,
    directory: DirectoryIdentity,
}

impl FileSnapshot {
    /// Captures a regular UTF-8 document or its absence without creating anything.
    ///
    /// # Errors
    /// Returns [`EditError`] for unsupported paths, concurrent reads or I/O failures.
    pub fn capture(path: &Path) -> Result<Self, EditError> {
        let path = resolved_destination(path)?;
        let (anchor, directory) = parent_identity(&path)?;
        let mut snapshot = Self {
            anchor,
            directory,
            path,
            bytes: None,
            stamp: None,
            permissions: None,
        };
        let before = match std::fs::symlink_metadata(&snapshot.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(snapshot),
            Err(error) => return Err(io_error(&snapshot.path, &error)),
        };
        if !before.is_file() || before.file_type().is_symlink() {
            return Err(EditError::Unsupported(
                "solo se editan archivos regulares, sin enlaces simbólicos".to_owned(),
            ));
        }
        #[cfg(unix)]
        if before.nlink() != 1 {
            return Err(EditError::Unsupported(
                "no se reemplazan manifiestos con enlaces físicos".to_owned(),
            ));
        }
        if before.permissions().readonly() {
            return Err(EditError::ReadOnly(snapshot.path));
        }
        // Opening without create/truncate performs an OS permission check,
        // including ACLs, without writing data or changing timestamps.
        OpenOptions::new()
            .write(true)
            .open(&snapshot.path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::PermissionDenied {
                    EditError::ReadOnly(snapshot.path.clone())
                } else {
                    io_error(&snapshot.path, &error)
                }
            })?;
        if before.len() > u64::try_from(MAX_DOCUMENT_BYTES).unwrap_or(u64::MAX) {
            return Err(too_large());
        }
        let file = File::open(&snapshot.path).map_err(|error| io_error(&snapshot.path, &error))?;
        let opened = file
            .metadata()
            .map_err(|error| io_error(&snapshot.path, &error))?;
        if Stamp::of(&before) != Stamp::of(&opened) {
            return Err(EditError::Conflict(snapshot.path));
        }
        let mut bytes = Vec::new();
        file.take(
            u64::try_from(MAX_DOCUMENT_BYTES)
                .unwrap_or(0)
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(&snapshot.path, &error))?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(too_large());
        }
        std::str::from_utf8(&bytes)
            .map_err(|_| EditError::Unsupported("el archivo no es texto UTF-8".to_owned()))?;
        let after = std::fs::symlink_metadata(&snapshot.path)
            .map_err(|error| io_error(&snapshot.path, &error))?;
        if Stamp::of(&before) != Stamp::of(&after) || after.file_type().is_symlink() {
            return Err(EditError::Conflict(snapshot.path));
        }
        snapshot.stamp = Some(Stamp::of(&before));
        snapshot.permissions = Some(before.permissions());
        snapshot.bytes = Some(bytes);
        Ok(snapshot)
    }

    /// Actual canonical destination, including the filename that may not exist yet.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The original text, or `None` for a document not created yet.
    ///
    /// # Errors
    /// Returns an unsupported-text error if this snapshot is not UTF-8.
    pub fn text(&self) -> Result<Option<&str>, EditError> {
        self.bytes
            .as_deref()
            .map(std::str::from_utf8)
            .transpose()
            .map_err(|_| EditError::Unsupported("el archivo no es texto UTF-8".to_owned()))
    }

    /// Checks the version without creating or changing any file.
    ///
    /// # Errors
    /// Returns conflict on changed content, identity, permissions or destination.
    pub fn check(&self) -> Result<(), EditError> {
        let metadata = std::fs::symlink_metadata(&self.anchor)
            .map_err(|_| EditError::Conflict(self.path.clone()))?;
        if !metadata.is_dir() || DirectoryIdentity::of(&metadata) != self.directory {
            return Err(EditError::Conflict(self.path.clone()));
        }
        let current = Self::capture(&self.path)?;
        if current.path != self.path || current.stamp != self.stamp || current.bytes != self.bytes {
            return Err(EditError::Conflict(self.path.clone()));
        }
        Ok(())
    }

    /// Commits an already validated candidate against the exact captured version.
    /// Existing edits use rename; new files use no-clobber hard-link installation.
    ///
    /// # Errors
    /// Returns [`EditError`] without replacing the destination when a precondition fails.
    pub fn commit(&self, text: &str) -> Result<(), EditError> {
        if text.len() > MAX_DOCUMENT_BYTES {
            return Err(too_large());
        }
        let _guard = WRITES.lock().unwrap_or_else(PoisonError::into_inner);
        self.check()?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| EditError::Unsupported("el destino no tiene carpeta".to_owned()))?;
        std::fs::create_dir_all(parent).map_err(|error| io_error(parent, &error))?;
        self.check()?;
        let (temporary, mut file) = Temporary::create(parent)?;
        // On macOS std::fs::copy uses COPYFILE_ALL for a regular destination:
        // retain ACLs and extended attributes before replacing the contents.
        #[cfg(target_os = "macos")]
        if self.bytes.is_some() {
            std::fs::copy(&self.path, &temporary.0)
                .map_err(|error| io_error(&self.path, &error))?;
            self.check()?;
            file.set_len(0)
                .map_err(|error| io_error(&self.path, &error))?;
        }
        file.write_all(text.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| io_error(&self.path, &error))?;
        #[cfg(unix)]
        if let Some(stamp) = &self.stamp {
            let metadata = file
                .metadata()
                .map_err(|error| io_error(&self.path, &error))?;
            if metadata.uid() != stamp.uid || metadata.gid() != stamp.gid {
                return Err(EditError::Unsupported(
                    "no se puede conservar el propietario o grupo del archivo".to_owned(),
                ));
            }
        }
        if let Some(permissions) = &self.permissions {
            file.set_permissions(permissions.clone())
                .and_then(|()| file.sync_all())
                .map_err(|error| io_error(&self.path, &error))?;
        }
        self.check()?;
        if self.bytes.is_some() {
            std::fs::rename(&temporary.0, &self.path)
                .map_err(|error| io_error(&self.path, &error))?;
        } else {
            std::fs::hard_link(&temporary.0, &self.path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    EditError::Conflict(self.path.clone())
                } else {
                    io_error(&self.path, &error)
                }
            })?;
        }
        Ok(())
    }
}

struct Temporary(PathBuf);

impl Temporary {
    fn create(parent: &Path) -> Result<(Self, File), EditError> {
        for _ in 0..8 {
            let entropy =
                crate::ids::system_entropy().map_err(|error| EditError::Io(error.to_string()))?;
            let path = parent.join(format!(
                ".devports-edit-{}.tmp",
                crate::ids::token_text(&entropy)
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            match options.open(&path) {
                Ok(file) => return Ok((Self(path), file)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io_error(&path, &error)),
            }
        }
        Err(EditError::Io(
            "no se pudo reservar un temporal único".to_owned(),
        ))
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn io_error(path: &Path, error: &std::io::Error) -> EditError {
    EditError::Io(format!("{}: {error}", path.display()))
}

fn too_large() -> EditError {
    EditError::Unsupported(format!(
        "la edición automática admite como máximo {MAX_DOCUMENT_BYTES} bytes"
    ))
}

fn parent_identity(path: &Path) -> Result<(PathBuf, DirectoryIdentity), EditError> {
    let parent = path
        .parent()
        .ok_or_else(|| EditError::Unsupported("falta la carpeta destino".to_owned()))?;
    for ancestor in parent.ancestors() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => {
                return Ok((ancestor.to_path_buf(), DirectoryIdentity::of(&metadata)));
            }
            Ok(_) => {
                return Err(EditError::Unsupported(
                    "la carpeta destino no es un directorio regular".to_owned(),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(ancestor, &error)),
        }
    }
    Err(EditError::Unsupported(
        "no se encontró la carpeta destino".to_owned(),
    ))
}

fn resolved_destination(path: &Path) -> Result<PathBuf, EditError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(EditError::Unsupported(
            "el destino debe ser absoluto y no contener «..»".to_owned(),
        ));
    }
    let filename = path
        .file_name()
        .ok_or_else(|| EditError::Unsupported("falta el archivo destino".to_owned()))?;
    let parent = path
        .parent()
        .ok_or_else(|| EditError::Unsupported("falta la carpeta destino".to_owned()))?;
    for ancestor in parent.ancestors() {
        match std::fs::canonicalize(ancestor) {
            Ok(existing) => {
                let remainder = parent
                    .strip_prefix(ancestor)
                    .map_err(|error| EditError::Unsupported(error.to_string()))?;
                return Ok(existing.join(remainder).join(filename));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(ancestor, &error)),
        }
    }
    Err(EditError::Unsupported(
        "no se puede resolver la carpeta destino".to_owned(),
    ))
}

#[cfg(test)]
mod tests;
