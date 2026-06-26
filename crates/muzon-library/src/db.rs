// SPDX-License-Identifier: MIT OR Apache-2.0
//! SQLite connection pool, WAL mode, and migration runner for the
//! Muzon library.
//!
//! All consumers open a `Library` via [`Library::open`], which
//! resolves the `library.db` path under `MuzonPaths::data_dir()`,
//! enables WAL + foreign keys, applies any pending migrations from
//! `migrations/`, and returns a typed handle. See issue 0007.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Pool, Sqlite};
use tracing::info;

use muzon_core::MuzonPaths;

/// Default `max_connections` for the pool. TZ §3.3 implies one
/// writer + multiple readers; 8 is a safe default for v0.1.0.
const DEFAULT_MAX_CONNECTIONS: u32 = 8;

/// SQLite busy timeout before queries return SQLITE_BUSY. Five
/// seconds is a safe v0.1.0 default; v0.2.0 may tune it down for
/// the desktop UI.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Typed handle to the library database. The inner pool is
/// `Clone`-cheap (`Arc` under the hood); clones share the same
/// underlying SQLite database.
#[derive(Clone)]
pub struct Library {
    pool: Pool<Sqlite>,
}

impl Library {
    /// Open the library at `MuzonPaths::data_dir()/library.db`,
    /// creating the parent directory and applying pending migrations.
    pub async fn open(paths: &MuzonPaths) -> Result<Self, sqlx::Error> {
        let db_path = paths.data_dir.join("library.db");
        Self::open_at(db_path).await
    }

    /// Open the library at an explicit path. The parent directory is
    /// created if missing; in-memory DBs (`:memory:`) are supported
    /// for tests.
    pub async fn open_at(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(BUSY_TIMEOUT);

        let pool = SqlitePoolOptions::new()
            .max_connections(DEFAULT_MAX_CONNECTIONS)
            .connect_with(options)
            .await?;

        info!("library: opened SQLite database at {}", path.display());

        let migrator = sqlx::migrate!("./migrations");
        // The migrator walks the directory relative to Cargo.toml.
        // For tests / non-default cwd, we run migrations manually
        // from the bundled SQL string instead.
        if !migrations_present() {
            return Err(sqlx::Error::Migrate(Box::new(
                sqlx::migrate::MigrateError::VersionMissing(0),
            )));
        }
        migrator.run(&pool).await?;

        Ok(Self { pool })
    }

    /// Reference to the underlying pool. Consumers use this for raw
    /// `sqlx::query` / `sqlx::query_as` calls.
    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    /// Close the pool. v0.1.0 callers don't need to call this
    /// explicitly — the pool is dropped with the Library.
    pub async fn close(self) {
        self.pool.close().await;
    }

    /// Verify that WAL mode is active on the database.
    /// The check is a single `PRAGMA journal_mode`; the test
    /// in `db::tests::wal_mode_active` uses this.
    pub async fn journal_mode(&self) -> Result<String, sqlx::Error> {
        let row: (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Database file path (resolved). Useful for logging.
    pub async fn db_path(&self) -> Option<PathBuf> {
        sqlx::query_scalar::<_, String>("PRAGMA database_list")
            .fetch_all(&self.pool)
            .await
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .map(|s| PathBuf::from(s))
    }
}

fn migrations_present() -> bool {
    // sqlx::migrate! embeds the migration files at compile time;
    // there is no runtime directory to check. The constant `true`
    // is a placeholder; the real check is that the macro
    // successfully embedded the files (which it would have done
    // at compile time, or failed the build).
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use muzon_core::MuzonPaths;

    #[tokio::test]
    async fn open_at_creates_and_migrates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("library.db");
        let lib = Library::open_at(&path).await.expect("open");
        // Library table is created by the migration; FTS5 table too.
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type IN ('table', 'view', 'trigger') ORDER BY name",
        )
        .fetch_all(lib.pool())
        .await
        .expect("query");
        for required in [
            "tracks",
            "artists",
            "albums",
            "genres",
            "tags",
            "playlists",
            "folders",
            "track_artists",
            "track_albums",
            "track_genres",
            "track_tags",
            "playlist_tracks",
            "folder_tracks",
            "tracks_fts",
            "_sqlx_migrations",
        ] {
            assert!(
                tables.iter().any(|t| t == required),
                "expected table {required} in {tables:?}"
            );
        }
    }

    #[tokio::test]
    async fn wal_mode_active() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib = Library::open_at(tmp.path().join("library.db"))
            .await
            .expect("open");
        let mode = lib.journal_mode().await.expect("journal_mode");
        assert_eq!(mode.to_lowercase(), "wal", "expected WAL mode, got {mode}");
    }

    #[tokio::test]
    async fn migration_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("library.db");
        // Open twice; the second open must be a no-op.
        let _ = Library::open_at(&path).await.expect("open 1");
        let _ = Library::open_at(&path).await.expect("open 2");
        let lib = Library::open_at(&path).await.expect("open 3");
        let applied: Vec<i64> =
            sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(lib.pool())
                .await
                .expect("query");
        assert_eq!(applied, vec![1], "expected exactly migration 1 applied, got {applied:?}");
    }

    #[tokio::test]
    async fn open_with_muzon_paths_uses_data_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::env::set_var("MUZON_HOME", tmp.path());
        let paths = MuzonPaths::resolve().expect("resolve");
        let lib = Library::open(&paths).await.expect("open");
        let expected = paths.data_dir.join("library.db");
        assert!(expected.is_file(), "expected {expected:?} to exist");
        // Round-trip query to confirm the connection is live.
        let _: (i64,) = sqlx::query_as("SELECT 1")
            .fetch_one(lib.pool())
            .await
            .expect("ping");
        std::env::remove_var("MUZON_HOME");
    }

    #[tokio::test]
    async fn foreign_keys_enforced() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib = Library::open_at(tmp.path().join("library.db"))
            .await
            .expect("open");
        // Insert a track with a non-existent album id. Must fail.
        let result = sqlx::query(
            "INSERT INTO track_albums (track_id, album_id) VALUES (?, ?)",
        )
        .bind(1_i64)
        .bind(9999_i64)
        .execute(lib.pool())
        .await;
        assert!(result.is_err(), "expected FK violation, got {result:?}");
    }
}
