//! The SQLite store: registered instances and the activity log
//! (`ARQUITECTURA.md`, section 9).
//!
//! One connection, owned by the core's single actor, so there is exactly one
//! writer. The database is in WAL mode, so a crash never leaves it half-written,
//! and it is versioned with `PRAGMA user_version` so a newer build can migrate an
//! older file.
//!
//! Only what must survive a restart is stored: registered instances (their
//! leases and the hash of their stop token) and events. What is merely observed
//! (processes and containers) is rebuilt from the system on every start.

use std::path::Path;

use devports_proto::{LanguagePreference, Preferences, ThemePreference};
use rusqlite::{Connection, OptionalExtension, Row, params};

/// The schema, one entry per version. Never edit an entry that has shipped:
/// append a new one.
const MIGRATIONS: [&str; 4] = [
    "
    CREATE TABLE instances (
      id INTEGER PRIMARY KEY,
      ref TEXT NOT NULL UNIQUE,
      project TEXT, service TEXT,
      role TEXT NOT NULL CHECK (role IN ('principal','temporal','unregistered')),
      registered INTEGER NOT NULL DEFAULT 0,
      state TEXT NOT NULL CHECK (state IN
        ('reserved','running','stopping','stopped','released','expired_alive','down')),
      port INTEGER,
      pid INTEGER, pid_start INTEGER,
      container_id TEXT,
      cwd TEXT, branch TEXT, worktree TEXT,
      launcher_pid INTEGER, launcher_start INTEGER, launcher_chain TEXT NOT NULL DEFAULT '[]',
      reason TEXT,
      token_hash TEXT,
      reserved_at INTEGER, started_at INTEGER, expires_at INTEGER,
      leased_total_s INTEGER NOT NULL DEFAULT 0,
      warned INTEGER NOT NULL DEFAULT 0,
      healthy INTEGER,
      ended_at INTEGER, end_reason TEXT
    );
    CREATE INDEX idx_instances_active ON instances (state, project, service);
    CREATE UNIQUE INDEX idx_instances_token ON instances (token_hash) WHERE token_hash IS NOT NULL;

    CREATE TABLE events (
      id INTEGER PRIMARY KEY,
      ts INTEGER NOT NULL,
      kind TEXT NOT NULL,
      project TEXT, service TEXT, instance_ref TEXT,
      payload TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX idx_events_ts ON events (ts);
    ",
    "
    CREATE TABLE ignored (
      key TEXT PRIMARY KEY,
      created_at INTEGER NOT NULL
    );
    ",
    "
    CREATE TABLE classifications (
      key TEXT PRIMARY KEY,
      class TEXT NOT NULL CHECK (class IN ('project','auxiliary','unclassified')),
      created_at INTEGER NOT NULL
    );
    ",
    "
    CREATE TABLE preferences (
      key TEXT PRIMARY KEY CHECK (key IN ('theme','language')),
      value TEXT NOT NULL CHECK (
        (key = 'theme' AND value IN ('system','light','dark'))
        OR (key = 'language' AND value IN ('system','es','en'))
      ),
      updated_at INTEGER NOT NULL
    );
    ",
];

/// Why a store operation failed.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// SQLite reported an error.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The file was written by a newer build.
    #[error(
        "the database is from a newer DevPorts (schema {found}, this build knows {known}); update devports"
    )]
    TooNew {
        /// Version found in the file.
        found: usize,
        /// Newest version this build understands.
        known: usize,
    },
    /// A value in the file is not one this build knows.
    #[error("unreadable value in column `{column}`: {value}")]
    Unreadable {
        /// The column.
        column: &'static str,
        /// What was found.
        value: String,
    },
}

/// What kind of instance a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The official instance of a service.
    Principal,
    /// A test instance with a lease.
    Temporal,
    /// A duplicate that nobody registered.
    Unregistered,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Self::Principal => "principal",
            Self::Temporal => "temporal",
            Self::Unregistered => "unregistered",
        }
    }

    fn parse(text: &str) -> Result<Self, StoreError> {
        match text {
            "principal" => Ok(Self::Principal),
            "temporal" => Ok(Self::Temporal),
            "unregistered" => Ok(Self::Unregistered),
            other => Err(StoreError::Unreadable {
                column: "role",
                value: other.to_owned(),
            }),
        }
    }
}

/// Where an instance is in its life (`ARQUITECTURA.md`, section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Port reserved, waiting for the process to open it.
    Reserved,
    /// Bound to a live process or container.
    Running,
    /// Being stopped.
    Stopping,
    /// Stopped.
    Stopped,
    /// The reservation was given up.
    Released,
    /// The lease ended and the process would not die.
    ExpiredAlive,
    /// The process is gone.
    Down,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Released => "released",
            Self::ExpiredAlive => "expired_alive",
            Self::Down => "down",
        }
    }

    fn parse(text: &str) -> Result<Self, StoreError> {
        match text {
            "reserved" => Ok(Self::Reserved),
            "running" => Ok(Self::Running),
            "stopping" => Ok(Self::Stopping),
            "stopped" => Ok(Self::Stopped),
            "released" => Ok(Self::Released),
            "expired_alive" => Ok(Self::ExpiredAlive),
            "down" => Ok(Self::Down),
            other => Err(StoreError::Unreadable {
                column: "state",
                value: other.to_owned(),
            }),
        }
    }

    /// Whether the instance still counts: it is neither over nor given up.
    #[must_use]
    pub fn is_live(self) -> bool {
        matches!(
            self,
            Self::Reserved | Self::Running | Self::Stopping | Self::ExpiredAlive | Self::Down
        )
    }
}

/// One row of `instances`. Times are seconds since the Unix epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceRecord {
    /// Row id; `0` before the record is inserted.
    pub id: i64,
    /// Short public reference.
    pub reference: String,
    /// Project name.
    pub project: Option<String>,
    /// Service name.
    pub service: Option<String>,
    /// Principal, Temporal or unregistered.
    pub role: Role,
    /// Whether it went through `devports run`.
    pub registered: bool,
    /// Life-cycle state.
    pub state: State,
    /// Port, reserved or in use.
    pub port: Option<u16>,
    /// Process id of the bound process.
    pub pid: Option<u32>,
    /// Start time of that process, microseconds since the epoch.
    pub pid_start: Option<u64>,
    /// Container id, for a container.
    pub container_id: Option<String>,
    /// Working directory.
    pub cwd: Option<String>,
    /// Branch when it started.
    pub branch: Option<String>,
    /// Worktree folder.
    pub worktree: Option<String>,
    /// Pid of the `devports run` that launched it.
    pub launcher_pid: Option<u32>,
    /// Start time of that launcher, microseconds since the epoch.
    pub launcher_start: Option<u64>,
    /// The launcher's ancestors, as JSON.
    pub launcher_chain: String,
    /// Why a Temporal was started.
    pub reason: Option<String>,
    /// SHA-256 of the stop token (Temporals); the token itself is never stored.
    pub token_hash: Option<String>,
    /// When the port was reserved.
    pub reserved_at: Option<i64>,
    /// When the process was bound.
    pub started_at: Option<i64>,
    /// When the lease ends.
    pub expires_at: Option<i64>,
    /// Total seconds leased so far.
    pub leased_total_s: u64,
    /// Whether the one-minute warning went out.
    pub warned: bool,
    /// Last health result.
    pub healthy: Option<bool>,
    /// When it ended.
    pub ended_at: Option<i64>,
    /// Why it ended.
    pub end_reason: Option<String>,
}

impl InstanceRecord {
    /// A new, empty record for `reference` with the given role and state.
    #[must_use]
    pub fn new(reference: impl Into<String>, role: Role, state: State) -> Self {
        Self {
            id: 0,
            reference: reference.into(),
            project: None,
            service: None,
            role,
            registered: true,
            state,
            port: None,
            pid: None,
            pid_start: None,
            container_id: None,
            cwd: None,
            branch: None,
            worktree: None,
            launcher_pid: None,
            launcher_start: None,
            launcher_chain: "[]".to_owned(),
            reason: None,
            token_hash: None,
            reserved_at: None,
            started_at: None,
            expires_at: None,
            leased_total_s: 0,
            warned: false,
            healthy: None,
            ended_at: None,
            end_reason: None,
        }
    }
}

/// One row of `events`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    /// Row id.
    pub id: i64,
    /// When, seconds since the epoch.
    pub ts: i64,
    /// Event kind, in `snake_case`.
    pub kind: String,
    /// Project concerned.
    pub project: Option<String>,
    /// Service concerned.
    pub service: Option<String>,
    /// Reference of the instance concerned.
    pub instance_ref: Option<String>,
    /// Details, as JSON.
    pub payload: String,
}

const INSTANCE_COLUMNS: &str = "id, ref, project, service, role, registered, state, port, pid, \
    pid_start, container_id, cwd, branch, worktree, launcher_pid, launcher_chain, reason, \
    token_hash, reserved_at, started_at, expires_at, leased_total_s, warned, healthy, ended_at, \
    end_reason, launcher_start";

/// The database.
#[derive(Debug)]
pub struct Store {
    connection: Connection,
}

impl Store {
    /// Opens (creating and migrating if needed) the database at `path`.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            // A failure here surfaces as an "unable to open" error just below.
            let _ = std::fs::create_dir_all(parent);
        }
        Self::prepare(Connection::open(path)?)
    }

    /// An in-memory database, for tests.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::prepare(Connection::open_in_memory()?)
    }

    fn prepare(connection: Connection) -> Result<Self, StoreError> {
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        let found = self.schema_version()?;
        if found > MIGRATIONS.len() {
            return Err(StoreError::TooNew {
                found,
                known: MIGRATIONS.len(),
            });
        }
        for (index, migration) in MIGRATIONS.iter().enumerate().skip(found) {
            let version = index.saturating_add(1);
            let transaction = self.connection.unchecked_transaction()?;
            transaction.execute_batch(migration)?;
            transaction.pragma_update(
                None,
                "user_version",
                i64::try_from(version).unwrap_or(i64::MAX),
            )?;
            transaction.commit()?;
        }
        Ok(())
    }

    /// The schema version of the open database.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn schema_version(&self) -> Result<usize, StoreError> {
        let version: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        Ok(usize::try_from(version).unwrap_or(0))
    }

    /// Inserts `record` and returns it with its row id.
    ///
    /// # Errors
    ///
    /// See [`StoreError`]; notably a duplicate reference or token hash.
    pub fn insert_instance(&self, record: &InstanceRecord) -> Result<InstanceRecord, StoreError> {
        self.connection.execute(
            "INSERT INTO instances (ref, project, service, role, registered, state, port, pid, \
             pid_start, container_id, cwd, branch, worktree, launcher_pid, launcher_chain, reason, \
             token_hash, reserved_at, started_at, expires_at, leased_total_s, warned, healthy, \
             ended_at, end_reason, launcher_start) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,\
             ?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)",
            params![
                record.reference,
                record.project,
                record.service,
                record.role.as_str(),
                record.registered,
                record.state.as_str(),
                record.port,
                record.pid,
                record.pid_start.map(u64_to_i64),
                record.container_id,
                record.cwd,
                record.branch,
                record.worktree,
                record.launcher_pid,
                record.launcher_chain,
                record.reason,
                record.token_hash,
                record.reserved_at,
                record.started_at,
                record.expires_at,
                u64_to_i64(record.leased_total_s),
                record.warned,
                record.healthy,
                record.ended_at,
                record.end_reason,
                record.launcher_start.map(u64_to_i64),
            ],
        )?;
        let mut stored = record.clone();
        stored.id = self.connection.last_insert_rowid();
        Ok(stored)
    }

    /// Writes every column of `record` back to its row.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn update_instance(&self, record: &InstanceRecord) -> Result<(), StoreError> {
        self.connection.execute(
            "UPDATE instances SET ref=?2, project=?3, service=?4, role=?5, registered=?6, \
             state=?7, port=?8, pid=?9, pid_start=?10, container_id=?11, cwd=?12, branch=?13, \
             worktree=?14, launcher_pid=?15, launcher_chain=?16, reason=?17, token_hash=?18, \
             reserved_at=?19, started_at=?20, expires_at=?21, leased_total_s=?22, warned=?23, \
             healthy=?24, ended_at=?25, end_reason=?26, launcher_start=?27 WHERE id=?1",
            params![
                record.id,
                record.reference,
                record.project,
                record.service,
                record.role.as_str(),
                record.registered,
                record.state.as_str(),
                record.port,
                record.pid,
                record.pid_start.map(u64_to_i64),
                record.container_id,
                record.cwd,
                record.branch,
                record.worktree,
                record.launcher_pid,
                record.launcher_chain,
                record.reason,
                record.token_hash,
                record.reserved_at,
                record.started_at,
                record.expires_at,
                u64_to_i64(record.leased_total_s),
                record.warned,
                record.healthy,
                record.ended_at,
                record.end_reason,
                record.launcher_start.map(u64_to_i64),
            ],
        )?;
        Ok(())
    }

    /// Every instance that still counts (see [`State::is_live`]), oldest first.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn live_instances(&self) -> Result<Vec<InstanceRecord>, StoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {INSTANCE_COLUMNS} FROM instances \
             WHERE state IN ('reserved','running','stopping','expired_alive','down') ORDER BY id"
        ))?;
        let rows = statement.query_map([], |row| Ok(read_instance(row)))?;
        rows.map(|row| row?).collect()
    }

    /// The instance whose stop token hashes to `token_hash`.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn instance_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<InstanceRecord>, StoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {INSTANCE_COLUMNS} FROM instances WHERE token_hash = ?1"
        ))?;
        let row = statement
            .query_row([token_hash], |row| Ok(read_instance(row)))
            .optional()?;
        row.transpose()
    }

    /// Appends an event to the log.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn append_event(
        &self,
        ts: i64,
        kind: &str,
        project: Option<&str>,
        service: Option<&str>,
        instance_ref: Option<&str>,
        payload: &str,
    ) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO events (ts, kind, project, service, instance_ref, payload) \
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![ts, kind, project, service, instance_ref, payload],
        )?;
        Ok(())
    }

    /// Events at or after `since`, oldest first, optionally for one project.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn events_since(
        &self,
        since: i64,
        project: Option<&str>,
    ) -> Result<Vec<EventRecord>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, ts, kind, project, service, instance_ref, payload FROM events \
             WHERE ts >= ?1 AND (?2 IS NULL OR project = ?2) ORDER BY ts, id",
        )?;
        let rows = statement.query_map(params![since, project], |row| {
            Ok(EventRecord {
                id: row.get(0)?,
                ts: row.get(1)?,
                kind: row.get(2)?,
                project: row.get(3)?,
                service: row.get(4)?,
                instance_ref: row.get(5)?,
                payload: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// What the person chose to keep (see [`Self::add_ignored`]).
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn ignored_keys(&self) -> Result<Vec<String>, StoreError> {
        let mut statement = self.connection.prepare("SELECT key FROM ignored")?;
        let keys = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(keys)
    }

    /// The person's classification corrections, by rule key.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn classification_overrides(&self) -> Result<Vec<(String, String)>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT key, class FROM classifications")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Saves or replaces the correction stored under `key`.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn set_classification(&self, key: &str, class: &str, at: i64) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO classifications (key, class, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET class = excluded.class, created_at = excluded.created_at",
            params![key, class, at],
        )?;
        Ok(())
    }

    /// Forgets the correction stored under `key`.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn clear_classification(&self, key: &str) -> Result<(), StoreError> {
        self.connection
            .execute("DELETE FROM classifications WHERE key = ?1", [key])?;
        Ok(())
    }

    /// The saved preferences. A key never saved, or saved with a value this build
    /// does not know, reads as `system`: a preference can never stop DevPorts from
    /// starting.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn preferences(&self) -> Result<Preferences, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT key, value FROM preferences")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut preferences = Preferences::default();
        for (key, value) in rows {
            match key.as_str() {
                "theme" => preferences.theme = ThemePreference::parse(&value).unwrap_or_default(),
                "language" => {
                    preferences.language = LanguagePreference::parse(&value).unwrap_or_default();
                }
                _ => {}
            }
        }
        Ok(preferences)
    }

    /// Saves both preferences at once, so a reader never sees half of a change.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn set_preferences(&self, preferences: Preferences, at: i64) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        for (key, value) in [
            ("theme", preferences.theme.as_str()),
            ("language", preferences.language.as_str()),
        ] {
            transaction.execute(
                "INSERT INTO preferences (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value, at],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Drops the events table so a read fails, to prove failures are not hidden.
    #[cfg(test)]
    pub(crate) fn break_events_for_test(&self) {
        self.connection
            .execute_batch("DROP TABLE events")
            .expect("events table dropped");
    }

    /// Every saved rule with the moment it was saved.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn ignored_rules(&self) -> Result<Vec<(String, i64)>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT key, created_at FROM ignored")?;
        let rules = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rules)
    }

    /// Remembers that the person chose to keep the thing with this key.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn add_ignored(&self, key: &str, at: i64) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT OR IGNORE INTO ignored (key, created_at) VALUES (?1, ?2)",
            params![key, at],
        )?;
        Ok(())
    }

    /// Forgets a previous [`Self::add_ignored`].
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn remove_ignored(&self, key: &str) -> Result<(), StoreError> {
        self.connection
            .execute("DELETE FROM ignored WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Deletes events older than `before`; returns how many.
    ///
    /// # Errors
    ///
    /// See [`StoreError`].
    pub fn prune_events(&self, before: i64) -> Result<usize, StoreError> {
        Ok(self
            .connection
            .execute("DELETE FROM events WHERE ts < ?1", [before])?)
    }
}

fn u64_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn read_instance(row: &Row<'_>) -> Result<InstanceRecord, StoreError> {
    let role: String = row.get(4)?;
    let state: String = row.get(6)?;
    let pid_start: Option<i64> = row.get(9)?;
    let leased: i64 = row.get(21)?;
    let launcher_start: Option<i64> = row.get(26)?;
    Ok(InstanceRecord {
        id: row.get(0)?,
        reference: row.get(1)?,
        project: row.get(2)?,
        service: row.get(3)?,
        role: Role::parse(&role)?,
        registered: row.get(5)?,
        state: State::parse(&state)?,
        port: row.get(7)?,
        pid: row.get(8)?,
        pid_start: pid_start.and_then(|value| u64::try_from(value).ok()),
        container_id: row.get(10)?,
        cwd: row.get(11)?,
        branch: row.get(12)?,
        worktree: row.get(13)?,
        launcher_pid: row.get(14)?,
        launcher_chain: row.get(15)?,
        reason: row.get(16)?,
        token_hash: row.get(17)?,
        reserved_at: row.get(18)?,
        started_at: row.get(19)?,
        expires_at: row.get(20)?,
        leased_total_s: u64::try_from(leased).unwrap_or(0),
        warned: row.get(22)?,
        healthy: row.get(23)?,
        ended_at: row.get(24)?,
        end_reason: row.get(25)?,
        launcher_start: launcher_start.and_then(|value| u64::try_from(value).ok()),
    })
}

#[cfg(test)]
mod tests;
