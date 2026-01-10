//! Test utilities for database setup.
//!
//! Provides helpers that reuse authoritative schema initialization,
//! eliminating schema duplication in test code.

use rusqlite::Connection;
use std::path::Path;
use tempfile::TempDir;

/// Test environment with app.db and learning.db using authoritative schemas.
///
/// Provides both database connections in a single temporary directory,
/// ensuring automatic cleanup when dropped.
pub struct TestEnv {
    /// Temporary directory (kept alive for database file persistence)
    pub temp: TempDir,
    /// app.db connection with full auth schema (all migrations)
    pub app_conn: Connection,
    /// learning.db connection with full learning schema (all migrations)
    pub user_conn: Connection,
}

impl TestEnv {
    /// Create a test environment with both databases initialized.
    ///
    /// Uses authoritative schema initialization functions:
    /// - `crate::auth::db::init_auth_schema()` for app.db
    /// - `crate::db::schema::run_migrations()` for learning.db
    ///
    /// After initialization, clears seeded baseline data to provide
    /// a clean slate for tests.
    pub fn new() -> rusqlite::Result<Self> {
        let temp =
            TempDir::new().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        // Create app.db with full auth schema
        let app_db_path = temp.path().join("app.db");
        let app_conn = Connection::open(&app_db_path)?;
        crate::auth::db::init_auth_schema(&app_conn)?;

        // Clear seeded baseline data for clean test slate
        // (production init seeds baseline cards, but tests need clean tables)
        app_conn.execute_batch(
            r#"
            DELETE FROM card_definitions;
            DELETE FROM content_packs;
            DELETE FROM pack_permissions;
            "#,
        )?;

        // Create learning.db with full learning schema
        let user_db_path = temp.path().join("learning.db");
        let user_conn = Connection::open(&user_db_path)?;
        crate::db::schema::run_migrations(&user_conn)?;

        Ok(Self {
            temp,
            app_conn,
            user_conn,
        })
    }

    /// Get the temporary directory path for creating test files.
    pub fn path(&self) -> &Path {
        self.temp.path()
    }
}
