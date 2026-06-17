use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::agent::AgentSnapshot;
use crate::config;

const SESSION_STORE_SCHEMA_VERSION: i64 = 1;
const DEFAULT_MIGRATED_JSON_RETENTION: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const DEFAULT_INACTIVE_SESSION_RETENTION: Duration = Duration::from_secs(180 * 24 * 60 * 60);
const DEFAULT_MIN_SESSIONS_TO_KEEP: usize = 50;

#[derive(Debug, Clone)]
pub(crate) struct SessionStore {
    db_path: PathBuf,
    legacy_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionRecord {
    pub(crate) name: String,
    pub(crate) updated_at: i64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct MigrationReport {
    pub(crate) scanned: usize,
    pub(crate) imported: usize,
    pub(crate) skipped_existing: usize,
    pub(crate) moved_backups: usize,
    pub(crate) failed: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CleanupPolicy {
    pub(crate) migrated_json_retention: Duration,
    pub(crate) inactive_session_retention: Option<Duration>,
    pub(crate) min_sessions_to_keep: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct CleanupReport {
    pub(crate) scanned_json_backups: usize,
    pub(crate) removed_json_backups: usize,
    pub(crate) scanned_sessions: usize,
    pub(crate) removed_sessions: usize,
    pub(crate) failed: Vec<String>,
}

impl Default for CleanupPolicy {
    fn default() -> Self {
        Self {
            migrated_json_retention: DEFAULT_MIGRATED_JSON_RETENTION,
            inactive_session_retention: Some(DEFAULT_INACTIVE_SESSION_RETENTION),
            min_sessions_to_keep: DEFAULT_MIN_SESSIONS_TO_KEEP,
        }
    }
}

impl SessionStore {
    pub(crate) fn open_default() -> Result<Self> {
        Self::open_at(config::sessions_db_path()?, config::sessions_dir()?)
    }

    pub(crate) fn open_at(db_path: PathBuf, legacy_dir: PathBuf) -> Result<Self> {
        let store = Self {
            db_path,
            legacy_dir,
        };
        store.ensure_schema()?;
        Ok(store)
    }

    pub(crate) fn migrate_json_sessions(&self) -> Result<MigrationReport> {
        let mut report = MigrationReport::default();
        if !self.legacy_dir.exists() {
            return Ok(report);
        }

        for entry in std::fs::read_dir(&self.legacy_dir)
            .with_context(|| format!("failed to list {}", self.legacy_dir.display()))?
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    report.failed.push(error.to_string());
                    continue;
                }
            };
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            report.scanned += 1;
            let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
                report
                    .failed
                    .push(format!("invalid session file name: {}", path.display()));
                continue;
            };
            if !config::is_valid_session_name(name) {
                report.failed.push(format!(
                    "invalid session name `{name}` in {}",
                    path.display()
                ));
                continue;
            }

            let body = match std::fs::read_to_string(&path) {
                Ok(body) => body,
                Err(error) => {
                    report
                        .failed
                        .push(format!("failed to read {}: {error}", path.display()));
                    continue;
                }
            };
            let snapshot = match serde_json::from_str::<AgentSnapshot>(&body) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    report
                        .failed
                        .push(format!("failed to parse {}: {error}", path.display()));
                    continue;
                }
            };

            if self.exists(name)? {
                report.skipped_existing += 1;
            } else {
                self.save_with_source(name, &snapshot, Some(&path))?;
                report.imported += 1;
            }

            match self.move_legacy_backup(&path) {
                Ok(()) => report.moved_backups += 1,
                Err(error) => report
                    .failed
                    .push(format!("failed to move {}: {error}", path.display())),
            }
        }

        Ok(report)
    }

    pub(crate) fn cleanup_old_sessions(
        &self,
        policy: CleanupPolicy,
        protected_session_name: Option<&str>,
    ) -> Result<CleanupReport> {
        let mut report = CleanupReport::default();
        self.cleanup_old_migrated_json(policy, &mut report)?;
        self.cleanup_old_sqlite_sessions(policy, protected_session_name, &mut report)?;
        Ok(report)
    }

    fn cleanup_old_migrated_json(
        &self,
        policy: CleanupPolicy,
        report: &mut CleanupReport,
    ) -> Result<()> {
        let backup_dir = self.migrated_backup_dir();
        if !backup_dir.exists() {
            return Ok(());
        }
        let now = SystemTime::now();
        for entry in std::fs::read_dir(&backup_dir)
            .with_context(|| format!("failed to list {}", backup_dir.display()))?
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    report.failed.push(error.to_string());
                    continue;
                }
            };
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            report.scanned_json_backups += 1;
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) => {
                    report
                        .failed
                        .push(format!("failed to stat {}: {error}", path.display()));
                    continue;
                }
            };
            let age = metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .unwrap_or_default();
            if age < policy.migrated_json_retention {
                continue;
            }
            match std::fs::remove_file(&path) {
                Ok(()) => report.removed_json_backups += 1,
                Err(error) => report
                    .failed
                    .push(format!("failed to delete {}: {error}", path.display())),
            }
        }
        Ok(())
    }

    fn cleanup_old_sqlite_sessions(
        &self,
        policy: CleanupPolicy,
        protected_session_name: Option<&str>,
        report: &mut CleanupReport,
    ) -> Result<()> {
        let Some(retention) = policy.inactive_session_retention else {
            return Ok(());
        };
        if let Some(name) = protected_session_name {
            validate_session_name(name)?;
        }
        let cutoff = unix_time_secs().saturating_sub(retention.as_secs() as i64);
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT name, COALESCE(last_opened_at, updated_at, created_at) AS touched_at
             FROM sessions
             ORDER BY touched_at DESC, name ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        report.scanned_sessions = rows.len();

        let removable = rows
            .iter()
            .enumerate()
            .filter_map(|(index, (name, touched_at))| {
                if index < policy.min_sessions_to_keep {
                    return None;
                }
                if protected_session_name.is_some_and(|protected| protected == name) {
                    return None;
                }
                (*touched_at < cutoff).then(|| name.clone())
            })
            .collect::<Vec<_>>();

        for name in removable {
            match conn.execute("DELETE FROM sessions WHERE name = ?1", [&name]) {
                Ok(deleted) => report.removed_sessions += deleted,
                Err(error) => report
                    .failed
                    .push(format!("failed to delete old session `{name}`: {error}")),
            }
        }
        if report.removed_sessions > 0 {
            conn.execute_batch("VACUUM")
                .context("failed to vacuum session database after cleanup")?;
        }
        Ok(())
    }

    pub(crate) fn list(&self) -> Result<Vec<SessionRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare("SELECT name, updated_at FROM sessions ORDER BY name ASC")?;
        let records = stmt
            .query_map([], |row| {
                Ok(SessionRecord {
                    name: row.get(0)?,
                    updated_at: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(records)
    }

    pub(crate) fn list_names(&self) -> Result<Vec<String>> {
        Ok(self.list()?.into_iter().map(|record| record.name).collect())
    }

    pub(crate) fn exists(&self, name: &str) -> Result<bool> {
        validate_session_name(name)?;
        let conn = self.connection()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE name = ?1",
            [name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub(crate) fn load(&self, name: &str) -> Result<Option<AgentSnapshot>> {
        validate_session_name(name)?;
        let conn = self.connection()?;
        let snapshot_json = conn
            .query_row(
                "SELECT snapshot_json FROM sessions WHERE name = ?1",
                [name],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(snapshot_json) = snapshot_json else {
            return Ok(None);
        };
        conn.execute(
            "UPDATE sessions SET last_opened_at = ?2 WHERE name = ?1",
            params![name, unix_time_secs()],
        )?;
        let snapshot = serde_json::from_str::<AgentSnapshot>(&snapshot_json)
            .with_context(|| format!("failed to parse stored session `{name}`"))?;
        Ok(Some(snapshot))
    }

    pub(crate) fn save(&self, name: &str, snapshot: &AgentSnapshot) -> Result<()> {
        self.save_with_source(name, snapshot, None)
    }

    pub(crate) fn rename(&self, old: &str, new: &str) -> Result<()> {
        validate_session_name(old)?;
        validate_session_name(new)?;
        if self.exists(new)? {
            anyhow::bail!("session `{new}` already exists");
        }
        let conn = self.connection()?;
        let updated = conn.execute(
            "UPDATE sessions SET name = ?2, updated_at = ?3 WHERE name = ?1",
            params![old, new, unix_time_secs()],
        )?;
        if updated == 0 {
            anyhow::bail!("session `{old}` does not exist");
        }
        Ok(())
    }

    pub(crate) fn delete(&self, name: &str) -> Result<()> {
        validate_session_name(name)?;
        let conn = self.connection()?;
        let deleted = conn.execute("DELETE FROM sessions WHERE name = ?1", [name])?;
        if deleted == 0 {
            anyhow::bail!("session `{name}` does not exist");
        }
        Ok(())
    }

    fn save_with_source(
        &self,
        name: &str,
        snapshot: &AgentSnapshot,
        migrated_from_path: Option<&Path>,
    ) -> Result<()> {
        validate_session_name(name)?;
        let conn = self.connection()?;
        let now = unix_time_secs();
        let snapshot_json = serde_json::to_string_pretty(snapshot)?;
        conn.execute(
            "INSERT INTO sessions (
                name, snapshot_json, schema_version, created_at, updated_at, migrated_from_path
            ) VALUES (?1, ?2, ?3, ?4, ?4, ?5)
            ON CONFLICT(name) DO UPDATE SET
                snapshot_json = excluded.snapshot_json,
                schema_version = excluded.schema_version,
                updated_at = excluded.updated_at",
            params![
                name,
                snapshot_json,
                snapshot.schema_version,
                now,
                migrated_from_path.map(|path| path.display().to_string())
            ],
        )?;
        Ok(())
    }

    fn ensure_schema(&self) -> Result<()> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let conn = self.connection()?;
        conn.execute_batch(
            "
            PRAGMA user_version = 1;
            CREATE TABLE IF NOT EXISTS sessions (
                name TEXT PRIMARY KEY,
                snapshot_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_opened_at INTEGER,
                migrated_from_path TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at);
            ",
        )?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)
            .with_context(|| format!("failed to open {}", self.db_path.display()))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > SESSION_STORE_SCHEMA_VERSION {
            anyhow::bail!(
                "session database schema version {version} is newer than supported {SESSION_STORE_SCHEMA_VERSION}"
            );
        }
        Ok(conn)
    }

    fn move_legacy_backup(&self, source: &Path) -> Result<()> {
        let backup_dir = self.migrated_backup_dir();
        std::fs::create_dir_all(&backup_dir)
            .with_context(|| format!("failed to create {}", backup_dir.display()))?;
        let file_name = source
            .file_name()
            .context("legacy session path did not include a file name")?;
        let mut target = backup_dir.join(file_name);
        if target.exists() {
            let stem = source
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("session");
            target = backup_dir.join(format!("{stem}.{}.json", unix_time_secs()));
        }
        std::fs::rename(source, &target).with_context(|| {
            format!(
                "failed to move {} to {}",
                source.display(),
                target.display()
            )
        })?;
        Ok(())
    }

    fn migrated_backup_dir(&self) -> PathBuf {
        self.legacy_dir.join("migrated")
    }
}

pub(crate) fn prepare_default_store(protected_session_name: Option<&str>) -> Result<()> {
    let store = SessionStore::open_default()?;
    let migration = store.migrate_json_sessions()?;
    let cleanup = store.cleanup_old_sessions(CleanupPolicy::default(), protected_session_name)?;
    for failure in migration.failed.iter().chain(cleanup.failed.iter()) {
        tracing::warn!(error = %failure, "session store maintenance failed");
    }
    Ok(())
}

fn validate_session_name(name: &str) -> Result<()> {
    if !config::is_valid_session_name(name) {
        anyhow::bail!(
            "invalid session name `{name}`; use letters, numbers, dots, dashes, and underscores"
        );
    }
    Ok(())
}

fn unix_time_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiler::AgentProfiler;
    use crate::tools::AgentMode;

    fn snapshot_with_input(text: &str) -> AgentSnapshot {
        AgentSnapshot {
            schema_version: 1,
            input: vec![serde_json::json!({
                "role": "user",
                "content": [{"type": "input_text", "text": text}]
            })],
            request_seq: 7,
            profiler: AgentProfiler::default(),
            loaded_skills: vec!["rust-patterns".to_string()],
            mode: AgentMode::Work,
            reasoning_effort: crate::client::DEFAULT_SPARK_AGENT_REASONING_EFFORT.to_string(),
            goal: None,
        }
    }

    fn temp_store() -> (tempfile::TempDir, SessionStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("sessions.sqlite3");
        let legacy_dir = dir.path().join("sessions");
        let store = SessionStore::open_at(db_path, legacy_dir).expect("open store");
        (dir, store)
    }

    #[test]
    fn saves_loads_lists_renames_and_deletes_sessions() {
        let (_dir, store) = temp_store();
        let snapshot = snapshot_with_input("hello");

        store.save("demo", &snapshot).expect("save");
        assert_eq!(store.list_names().expect("list"), vec!["demo"]);

        let loaded = store
            .load("demo")
            .expect("load")
            .expect("session should exist");
        assert_eq!(loaded.request_seq, 7);
        assert_eq!(loaded.loaded_skills, vec!["rust-patterns"]);

        store.rename("demo", "renamed").expect("rename");
        assert!(!store.exists("demo").expect("old exists"));
        assert!(store.exists("renamed").expect("new exists"));

        store.delete("renamed").expect("delete");
        assert!(store.list_names().expect("list empty").is_empty());
    }

    #[test]
    fn migrates_json_sessions_once_and_moves_legacy_files() {
        let (dir, store) = temp_store();
        std::fs::create_dir_all(&store.legacy_dir).expect("legacy dir");
        let legacy_path = store.legacy_dir.join("old.json");
        std::fs::write(
            &legacy_path,
            serde_json::to_vec_pretty(&snapshot_with_input("old")).expect("serialize"),
        )
        .expect("write legacy");

        let report = store.migrate_json_sessions().expect("migrate");

        assert_eq!(report.scanned, 1);
        assert_eq!(report.imported, 1);
        assert_eq!(report.moved_backups, 1);
        assert!(report.failed.is_empty());
        assert!(store.exists("old").expect("exists"));
        assert!(!legacy_path.exists());
        assert!(
            dir.path()
                .join("sessions")
                .join("migrated")
                .join("old.json")
                .exists()
        );

        let second = store.migrate_json_sessions().expect("migrate again");
        assert_eq!(second.scanned, 0);
    }

    #[test]
    fn migration_reports_bad_json_without_aborting_valid_sessions() {
        let (_dir, store) = temp_store();
        std::fs::create_dir_all(&store.legacy_dir).expect("legacy dir");
        std::fs::write(store.legacy_dir.join("bad.json"), "{").expect("write bad");
        std::fs::write(
            store.legacy_dir.join("good.json"),
            serde_json::to_vec_pretty(&snapshot_with_input("good")).expect("serialize"),
        )
        .expect("write good");

        let report = store.migrate_json_sessions().expect("migrate");

        assert_eq!(report.scanned, 2);
        assert_eq!(report.imported, 1);
        assert_eq!(report.failed.len(), 1);
        assert!(store.exists("good").expect("good exists"));
        assert!(store.legacy_dir.join("bad.json").exists());
    }

    #[test]
    fn cleanup_removes_expired_migrated_json_backups() {
        let (_dir, store) = temp_store();
        let backup_dir = store.migrated_backup_dir();
        std::fs::create_dir_all(&backup_dir).expect("backup dir");
        let backup = backup_dir.join("old.json");
        std::fs::write(&backup, "{}").expect("write backup");

        let report = store
            .cleanup_old_sessions(
                CleanupPolicy {
                    migrated_json_retention: Duration::ZERO,
                    inactive_session_retention: None,
                    min_sessions_to_keep: 0,
                },
                None,
            )
            .expect("cleanup");

        assert_eq!(report.scanned_json_backups, 1);
        assert_eq!(report.removed_json_backups, 1);
        assert!(!backup.exists());
    }

    #[test]
    fn cleanup_prunes_old_sqlite_sessions_but_keeps_recent_floor_and_protected_session() {
        let (_dir, store) = temp_store();
        let old_time = unix_time_secs() - (365 * 24 * 60 * 60);
        let recent_time = unix_time_secs();
        store
            .save("old-a", &snapshot_with_input("a"))
            .expect("save a");
        store
            .save("old-b", &snapshot_with_input("b"))
            .expect("save b");
        store
            .save("recent", &snapshot_with_input("recent"))
            .expect("save recent");
        let conn = store.connection().expect("connection");
        conn.execute(
            "UPDATE sessions SET created_at = ?2, updated_at = ?2, last_opened_at = ?2 WHERE name IN (?1)",
            params!["old-a", old_time],
        )
        .expect("age old a");
        conn.execute(
            "UPDATE sessions SET created_at = ?2, updated_at = ?2, last_opened_at = ?2 WHERE name IN (?1)",
            params!["old-b", old_time],
        )
        .expect("age old b");
        conn.execute(
            "UPDATE sessions SET created_at = ?2, updated_at = ?2, last_opened_at = ?2 WHERE name IN (?1)",
            params!["recent", recent_time],
        )
        .expect("touch recent");

        let report = store
            .cleanup_old_sessions(
                CleanupPolicy {
                    migrated_json_retention: Duration::from_secs(30),
                    inactive_session_retention: Some(Duration::from_secs(30)),
                    min_sessions_to_keep: 1,
                },
                Some("old-b"),
            )
            .expect("cleanup");

        assert_eq!(report.scanned_sessions, 3);
        assert_eq!(report.removed_sessions, 1);
        assert_eq!(store.list_names().expect("list"), vec!["old-b", "recent"]);
    }
}
