//! Auth database operations (users, sessions, app_settings, card_definitions tables).
//!
//! ## Migration System
//!
//! This module uses a version-gated migration system. Each migration:
//! 1. Checks if the current schema version is less than the target version
//! 2. Runs the migration SQL within a transaction (for atomicity)
//! 3. Records the new version in `db_version` table
//!
//! Migrations only run once - the version check ensures idempotency.
//! New databases get all tables created via `migrate_v0_to_v1`, then
//! subsequent migrations are skipped (version already at latest).

use chrono::{Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result};

/// Current schema version for app.db
/// Increment this when adding a new migration
pub const AUTH_DB_VERSION: i32 = 9;

/// Initialize the auth database schema with version-gated migrations
pub fn init_auth_schema(conn: &Connection) -> Result<()> {
    // Bootstrap: ensure db_version table exists (needed to check version)
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS db_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL,
            description TEXT
        );
        "#,
    )?;

    let current_version = get_schema_version(conn)?;
    tracing::debug!("app.db schema version: {}", current_version);

    // Run migrations in order, each checks version before executing
    if current_version < 1 {
        migrate_v0_to_v1(conn)?;
    }
    if current_version < 2 {
        migrate_v1_to_v2(conn)?;
    }
    if current_version < 3 {
        migrate_v2_to_v3(conn)?;
    }
    if current_version < 4 {
        migrate_v3_to_v4(conn)?;
    }
    if current_version < 5 {
        migrate_v4_to_v5(conn)?;
    }
    if current_version < 6 {
        migrate_v5_to_v6(conn)?;
    }
    if current_version < 7 {
        migrate_v6_to_v7(conn)?;
    }
    if current_version < 8 {
        migrate_v7_to_v8(conn)?;
    }
    if current_version < 9 {
        migrate_v8_to_v9(conn)?;
    }

    // Seed baseline cards if card_definitions is empty (idempotent)
    seed_baseline_cards(conn)?;

    Ok(())
}

// ============================================================
// VERSION-GATED MIGRATIONS
// Each migration runs exactly once based on version check
// ============================================================

/// v0→v1: Create base tables (users, sessions, app_settings)
fn migrate_v0_to_v1(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v0→v1: Create base tables");

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE COLLATE NOCASE,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_login_at TEXT
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            last_access_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);

        -- Default app settings
        INSERT OR IGNORE INTO app_settings (key, value) VALUES ('max_users', NULL);
        INSERT OR IGNORE INTO app_settings (key, value) VALUES ('max_guests', NULL);
        INSERT OR IGNORE INTO app_settings (key, value) VALUES ('guest_expiry_hours', '24');
        "#,
    )?;

    record_version(conn, 1, "Create base tables (users, sessions, app_settings)")?;
    Ok(())
}

/// v1→v2: Add guest user support columns
fn migrate_v1_to_v2(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v1→v2: Add guest user columns");

    add_column_if_missing(conn, "users", "is_guest", "INTEGER DEFAULT 0")?;
    add_column_if_missing(conn, "users", "last_activity_at", "TEXT")?;

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_users_is_guest ON users(is_guest);",
    )?;

    record_version(conn, 2, "Add guest user support (is_guest, last_activity_at)")?;
    Ok(())
}

/// v2→v3: Add content pack system (content_packs, card_definitions)
fn migrate_v2_to_v3(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v2→v3: Add content pack system");

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS content_packs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            pack_type TEXT NOT NULL,
            version TEXT,
            description TEXT,
            source_path TEXT NOT NULL,
            scope TEXT NOT NULL,
            installed_at TEXT NOT NULL,
            installed_by TEXT,
            metadata TEXT
        );

        CREATE TABLE IF NOT EXISTS card_definitions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            front TEXT NOT NULL,
            main_answer TEXT NOT NULL,
            description TEXT,
            card_type TEXT NOT NULL,
            tier INTEGER NOT NULL,
            audio_hint TEXT,
            is_reverse INTEGER NOT NULL DEFAULT 0,
            pack_id TEXT,
            FOREIGN KEY (pack_id) REFERENCES content_packs(id)
        );

        CREATE INDEX IF NOT EXISTS idx_content_packs_scope ON content_packs(scope);
        CREATE INDEX IF NOT EXISTS idx_content_packs_type ON content_packs(pack_type);
        CREATE INDEX IF NOT EXISTS idx_card_definitions_pack ON card_definitions(pack_id);
        CREATE INDEX IF NOT EXISTS idx_card_definitions_tier ON card_definitions(tier);
        "#,
    )?;

    record_version(conn, 3, "Add content pack system (content_packs, card_definitions)")?;
    Ok(())
}

/// v3→v4: Add user roles and group-based permissions
fn migrate_v3_to_v4(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v3→v4: Add user roles and groups");

    add_column_if_missing(conn, "users", "role", "TEXT DEFAULT 'user'")?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS user_groups (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS user_group_members (
            group_id TEXT NOT NULL,
            user_id INTEGER NOT NULL,
            added_at TEXT NOT NULL,
            PRIMARY KEY (group_id, user_id),
            FOREIGN KEY (group_id) REFERENCES user_groups(id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS pack_permissions (
            pack_id TEXT NOT NULL,
            group_id TEXT NOT NULL DEFAULT '',
            allowed INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (pack_id, group_id),
            FOREIGN KEY (pack_id) REFERENCES content_packs(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_user_group_members_user ON user_group_members(user_id);
        CREATE INDEX IF NOT EXISTS idx_pack_permissions_group ON pack_permissions(group_id);
        "#,
    )?;

    record_version(conn, 4, "Add user roles and group-based permissions")?;
    Ok(())
}

/// v4→v5: Add direct user pack permissions
fn migrate_v4_to_v5(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v4→v5: Add direct user pack permissions");

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS pack_user_permissions (
            pack_id TEXT NOT NULL,
            user_id INTEGER NOT NULL,
            allowed INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (pack_id, user_id),
            FOREIGN KEY (pack_id) REFERENCES content_packs(id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_pack_user_permissions_user ON pack_user_permissions(user_id);
        "#,
    )?;

    record_version(conn, 5, "Add direct user pack permissions (pack_user_permissions)")?;
    Ok(())
}

/// v5→v6: Add external pack path registration
fn migrate_v5_to_v6(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v5→v6: Add external pack paths");

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS registered_pack_paths (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            name TEXT,
            registered_by TEXT NOT NULL,
            registered_at TEXT NOT NULL,
            is_active INTEGER DEFAULT 1
        );
        "#,
    )?;

    record_version(conn, 6, "Add external pack path registration")?;
    Ok(())
}

/// v6→v7: Add lesson-based pack progression
fn migrate_v6_to_v7(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v6→v7: Add lesson-based pack progression");

    add_column_if_missing(conn, "card_definitions", "lesson", "INTEGER")?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS pack_ui_metadata (
            pack_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            unit_name TEXT DEFAULT 'Lessons',
            section_prefix TEXT DEFAULT 'Lesson',
            lesson_labels TEXT,
            unlock_threshold INTEGER DEFAULT 80,
            total_lessons INTEGER,
            progress_section_title TEXT,
            study_filter_label TEXT,
            FOREIGN KEY (pack_id) REFERENCES content_packs(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_card_definitions_pack_lesson ON card_definitions(pack_id, lesson);
        "#,
    )?;

    record_version(conn, 7, "Add lesson-based pack progression")?;
    Ok(())
}

/// v7→v8: Add global pack enable/disable
fn migrate_v7_to_v8(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v7→v8: Add global pack enable/disable");

    add_column_if_missing(conn, "content_packs", "is_enabled", "INTEGER DEFAULT 1")?;

    record_version(conn, 8, "Add global pack enable/disable (is_enabled column)")?;
    Ok(())
}

/// v8→v9: Register baseline pack and add public permission for global packs
fn migrate_v8_to_v9(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v8→v9: Register baseline pack and add public permissions");

    // Register baseline pack if not exists
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO content_packs (id, name, version, description, pack_type, scope, source_path, installed_at)
         VALUES ('baseline', 'Baseline Hangul Characters', '1.0.0', 'Core Hangul consonants and vowels', 'cards', 'global', 'data/content/packs/baseline', ?1)",
        params![now],
    )?;

    // Add public permission for all global packs
    conn.execute(
        "INSERT OR IGNORE INTO pack_permissions (pack_id, group_id, allowed)
         SELECT id, '', 1 FROM content_packs WHERE scope = 'global'",
        [],
    )?;

    record_version(conn, 9, "Register baseline pack and add public permissions")?;
    Ok(())
}

// ============================================================
// MIGRATION HELPERS
// ============================================================

/// Record a schema version after successful migration
fn record_version(conn: &Connection, version: i32, description: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO db_version (version, applied_at, description) VALUES (?1, ?2, ?3)",
        params![version, now, description],
    )?;
    tracing::info!("Recorded schema version {} - {}", version, description);
    Ok(())
}

/// Get current schema version (0 if no versions recorded)
pub fn get_schema_version(conn: &Connection) -> Result<i32> {
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM db_version",
        [],
        |row| row.get(0),
    )
}

/// Seed baseline cards into card_definitions if empty
fn seed_baseline_cards(conn: &Connection) -> Result<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM card_definitions", [], |row| row.get(0))?;
    if count > 0 {
        return Ok(());
    }

    // Register baseline pack in content_packs with public permission
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO content_packs (id, name, version, description, pack_type, scope, source_path, installed_at)
         VALUES ('baseline', 'Baseline Hangul Characters', '1.0.0', 'Core Hangul consonants and vowels', 'cards', 'global', 'data/content/packs/baseline', ?1)",
        params![now],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO pack_permissions (pack_id, group_id, allowed) VALUES ('baseline', '', 1)",
        [],
    )?;

    // Try to load from baseline pack, fall back to hardcoded data
    if let Some(cards) = crate::content::load_baseline_cards() {
        for card in cards {
            conn.execute(
                r#"INSERT INTO card_definitions
                   (front, main_answer, description, card_type, tier, is_reverse, pack_id)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)"#,
                params![
                    card.front,
                    card.main_answer,
                    card.description,
                    card.card_type.as_str(),
                    card.tier,
                    card.is_reverse,
                ],
            )?;
        }
        tracing::info!("Seeded {} baseline cards from pack", count);
    } else {
        // Fallback to hardcoded data
        seed_hardcoded_baseline_cards(conn)?;
    }

    Ok(())
}

/// Hardcoded baseline cards as fallback
fn seed_hardcoded_baseline_cards(conn: &Connection) -> Result<()> {
    use crate::domain::CardType;

    let tier1_consonants = [
        ("ㄱ", "g / k", "Like 'g' in 'go' at the start, 'k' in 'kite' at the end"),
        ("ㄴ", "n", "Like 'n' in 'no'"),
        ("ㄷ", "d / t", "Like 'd' in 'do' at the start, 't' in 'top' at the end"),
        ("ㄹ", "r / l", "Like 'r' in 'run' at the start, 'l' in 'ball' at the end"),
        ("ㅁ", "m", "Like 'm' in 'mom'"),
        ("ㅂ", "b / p", "Like 'b' in 'boy' at the start, 'p' in 'put' at the end"),
        ("ㅅ", "s", "Like 's' in 'sun'"),
        ("ㅈ", "j", "Like 'j' in 'just'"),
        ("ㅎ", "h", "Like 'h' in 'hi'"),
    ];

    for (front, main, desc) in tier1_consonants {
        insert_card_def(conn, front, main, Some(desc), CardType::Consonant, 1, false)?;
        insert_card_def(conn, main, front, None, CardType::Consonant, 1, true)?;
    }

    let tier1_vowels = [
        ("ㅏ", "a", "Like 'a' in 'father'"),
        ("ㅓ", "eo", "Like 'u' in 'fun' or 'uh'"),
        ("ㅗ", "o", "Like 'o' in 'go'"),
        ("ㅜ", "u", "Like 'oo' in 'moon'"),
        ("ㅡ", "eu", "Like 'u' in 'put', with unrounded lips"),
        ("ㅣ", "i", "Like 'ee' in 'see'"),
    ];

    for (front, main, desc) in tier1_vowels {
        insert_card_def(conn, front, main, Some(desc), CardType::Vowel, 1, false)?;
        insert_card_def(conn, main, front, None, CardType::Vowel, 1, true)?;
    }

    // Tier 2
    insert_card_def(conn, "ㅇ (initial)", "Silent", Some("No sound when at the start of a syllable"), CardType::Consonant, 2, false)?;
    insert_card_def(conn, "ㅇ (final)", "ng", Some("Like 'ng' in 'sing' when at the end"), CardType::Consonant, 2, false)?;

    let tier2_vowels = [
        ("ㅑ", "ya", "Like 'ya' in 'yacht'"),
        ("ㅕ", "yeo", "Like 'yu' in 'yuck'"),
        ("ㅛ", "yo", "Like 'yo' in 'yoga'"),
        ("ㅠ", "yu", "Like 'you'"),
        ("ㅐ", "ae", "Like 'a' in 'can' or 'e' in 'bed'"),
        ("ㅔ", "e", "Like 'e' in 'bed' (sounds same as ㅐ in modern Korean)"),
    ];

    for (front, main, desc) in tier2_vowels {
        insert_card_def(conn, front, main, Some(desc), CardType::Vowel, 2, false)?;
        insert_card_def(conn, main, front, None, CardType::Vowel, 2, true)?;
    }

    // Tier 3: Aspirated
    let tier3_aspirated = [
        ("ㅋ", "k (aspirated)", "Stronger 'k' with a puff of breath, like 'k' in 'kick'"),
        ("ㅍ", "p (aspirated)", "Stronger 'p' with a puff of breath, like 'p' in 'pop'"),
        ("ㅌ", "t (aspirated)", "Stronger 't' with a puff of breath, like 't' in 'top'"),
        ("ㅊ", "ch (aspirated)", "Stronger 'ch' with a puff of breath, like 'ch' in 'church'"),
    ];

    for (front, main, desc) in tier3_aspirated {
        insert_card_def(conn, front, main, Some(desc), CardType::AspiratedConsonant, 3, false)?;
        insert_card_def(conn, main, front, None, CardType::AspiratedConsonant, 3, true)?;
    }

    // Tier 3: Tense
    let tier3_tense = [
        ("ㄲ", "kk (tense)", "Tense 'k' with no breath, like 'ck' in 'sticky'"),
        ("ㅃ", "pp (tense)", "Tense 'p' with no breath, like 'pp' in 'happy'"),
        ("ㄸ", "tt (tense)", "Tense 't' with no breath, like 'tt' in 'butter'"),
        ("ㅆ", "ss (tense)", "Tense 's', like 'ss' in 'hiss'"),
        ("ㅉ", "jj (tense)", "Tense 'j', like 'dg' in 'edge'"),
    ];

    for (front, main, desc) in tier3_tense {
        insert_card_def(conn, front, main, Some(desc), CardType::TenseConsonant, 3, false)?;
        insert_card_def(conn, main, front, None, CardType::TenseConsonant, 3, true)?;
    }

    // Tier 4: Compound vowels
    let tier4_compound = [
        ("ㅘ", "wa", "Like 'wa' in 'want'"),
        ("ㅝ", "wo", "Like 'wo' in 'won'"),
        ("ㅟ", "wi", "Like 'wee'"),
        ("ㅚ", "oe", "Like 'we' in 'wet'"),
        ("ㅢ", "ui", "Like 'oo-ee' said quickly"),
        ("ㅙ", "wae", "Like 'wa' in 'wax'"),
        ("ㅞ", "we", "Like 'we' in 'wet'"),
        ("ㅒ", "yae", "Like 'ya' in 'yam'"),
        ("ㅖ", "ye", "Like 'ye' in 'yes'"),
    ];

    for (front, main, desc) in tier4_compound {
        insert_card_def(conn, front, main, Some(desc), CardType::CompoundVowel, 4, false)?;
        insert_card_def(conn, main, front, None, CardType::CompoundVowel, 4, true)?;
    }

    tracing::info!("Seeded 80 baseline cards from hardcoded data");
    Ok(())
}

fn insert_card_def(
    conn: &Connection,
    front: &str,
    main: &str,
    desc: Option<&str>,
    card_type: crate::domain::CardType,
    tier: u8,
    is_reverse: bool,
) -> Result<()> {
    conn.execute(
        r#"INSERT INTO card_definitions
           (front, main_answer, description, card_type, tier, is_reverse, pack_id)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)"#,
        params![front, main, desc, card_type.as_str(), tier, is_reverse],
    )?;
    Ok(())
}

/// Check if a column exists in a table
fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    conn
        .prepare(&format!("SELECT {} FROM {} LIMIT 1", column, table))
        .is_ok()
}

/// Add a column if it doesn't already exist
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    column_def: &str,
) -> Result<()> {
    if !column_exists(conn, table, column) {
        conn.execute(
            &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, column_def),
            [],
        )?;
    }
    Ok(())
}

/// Create a new user, returns the user ID
pub fn create_user(conn: &Connection, username: &str, password_hash: &str) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
        params![username, password_hash, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Get user by username, returns (user_id, password_hash)
pub fn get_user_by_username(conn: &Connection, username: &str) -> Result<Option<(i64, String)>> {
    let mut stmt = conn.prepare("SELECT id, password_hash FROM users WHERE username = ?1")?;
    let result = stmt.query_row(params![username], |row| Ok((row.get(0)?, row.get(1)?)));
    match result {
        Ok(user) => Ok(Some(user)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Check if a username already exists
pub fn username_exists(conn: &Connection, username: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM users WHERE username = ?1",
        params![username],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Check if a user is an admin (by role or legacy username='admin')
/// Backwards compatible: users named "admin" are always admins
pub fn is_user_admin(conn: &Connection, user_id: i64) -> Result<bool> {
    let is_admin: i64 = conn.query_row(
        r#"SELECT CASE
            WHEN COALESCE(role, 'user') = 'admin' THEN 1
            WHEN LOWER(username) = 'admin' THEN 1
            ELSE 0
        END FROM users WHERE id = ?1"#,
        params![user_id],
        |row| row.get(0),
    )?;
    Ok(is_admin == 1)
}

/// Get user role (defaults to 'user' if NULL)
pub fn get_user_role(conn: &Connection, user_id: i64) -> Result<String> {
    conn.query_row(
        "SELECT COALESCE(role, 'user') FROM users WHERE id = ?1",
        params![user_id],
        |row| row.get(0),
    )
}

/// Set user role ('user' or 'admin')
pub fn set_user_role(conn: &Connection, user_id: i64, role: &str) -> Result<()> {
    conn.execute(
        "UPDATE users SET role = ?1 WHERE id = ?2",
        params![role, user_id],
    )?;
    Ok(())
}

/// Create a new session
pub fn create_session(
    conn: &Connection,
    user_id: i64,
    session_id: &str,
    duration_hours: i64,
) -> Result<()> {
    let now = Utc::now();
    let expires = now + Duration::hours(duration_hours);
    conn.execute(
        "INSERT INTO sessions (id, user_id, created_at, expires_at, last_access_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            session_id,
            user_id,
            now.to_rfc3339(),
            expires.to_rfc3339(),
            now.to_rfc3339()
        ],
    )?;
    Ok(())
}

/// Validate session and get user info, returns (user_id, username)
pub fn get_session_user(conn: &Connection, session_id: &str) -> Result<Option<(i64, String)>> {
    let now = Utc::now().to_rfc3339();
    let mut stmt = conn.prepare(
        r#"
        SELECT u.id, u.username
        FROM sessions s
        JOIN users u ON s.user_id = u.id
        WHERE s.id = ?1 AND s.expires_at > ?2
    "#,
    )?;
    let result = stmt.query_row(params![session_id, now], |row| Ok((row.get(0)?, row.get(1)?)));
    match result {
        Ok((user_id, username)) => {
            // Update last access time
            let _ = conn.execute(
                "UPDATE sessions SET last_access_at = ?1 WHERE id = ?2",
                params![now, session_id],
            );
            Ok(Some((user_id, username)))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Delete a session (logout)
pub fn delete_session(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute("DELETE FROM sessions WHERE id = ?1", params![session_id])?;
    Ok(())
}

/// Delete all sessions for a user
pub fn delete_user_sessions(conn: &Connection, user_id: i64) -> Result<usize> {
    let count = conn.execute("DELETE FROM sessions WHERE user_id = ?1", params![user_id])?;
    Ok(count)
}

/// Cleanup expired sessions, returns count of deleted sessions
pub fn cleanup_expired_sessions(conn: &Connection) -> Result<usize> {
    let now = Utc::now().to_rfc3339();
    let count = conn.execute("DELETE FROM sessions WHERE expires_at < ?1", params![now])?;
    Ok(count)
}

/// Update user's last login timestamp
pub fn update_last_login(conn: &Connection, user_id: i64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE users SET last_login_at = ?1 WHERE id = ?2",
        params![now, user_id],
    )?;
    Ok(())
}

/// Get user count (for migration check)
pub fn get_user_count(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
}

/// User info for admin display
pub struct UserInfo {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub is_guest: bool,
    pub created_at: String,
}

/// Get all users for admin display
pub fn get_all_users(conn: &Connection) -> Result<Vec<UserInfo>> {
    let mut stmt = conn.prepare(
        r#"SELECT id, username, COALESCE(role, 'user'), COALESCE(is_guest, 0), created_at
           FROM users
           ORDER BY created_at DESC"#
    )?;
    let users = stmt
        .query_map([], |row| {
            Ok(UserInfo {
                id: row.get(0)?,
                username: row.get(1)?,
                role: row.get(2)?,
                is_guest: row.get::<_, i64>(3)? == 1,
                created_at: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(users)
}

/// Get a user by ID
pub fn get_user_by_id(conn: &Connection, user_id: i64) -> Result<Option<UserInfo>> {
    let mut stmt = conn.prepare(
        r#"SELECT id, username, COALESCE(role, 'user'), COALESCE(is_guest, 0), created_at
           FROM users
           WHERE id = ?1"#
    )?;
    let user = stmt
        .query_row([user_id], |row| {
            Ok(UserInfo {
                id: row.get(0)?,
                username: row.get(1)?,
                role: row.get(2)?,
                is_guest: row.get::<_, i64>(3)? == 1,
                created_at: row.get(4)?,
            })
        })
        .optional()?;
    Ok(user)
}

// ==================== Guest Operations ====================

/// Create a guest user, returns the user ID
pub fn create_guest_user(conn: &Connection, username: &str, password_hash: &str) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO users (username, password_hash, created_at, is_guest, last_activity_at) VALUES (?1, ?2, ?3, 1, ?4)",
        params![username, password_hash, now, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Get count of regular (non-guest) users
pub fn get_regular_user_count(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM users WHERE is_guest = 0",
        [],
        |row| row.get(0),
    )
}

/// Get count of guest users
pub fn get_guest_count(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM users WHERE is_guest = 1",
        [],
        |row| row.get(0),
    )
}

/// Check if a user is a guest
pub fn is_guest_user(conn: &Connection, user_id: i64) -> Result<bool> {
    let is_guest: i64 = conn.query_row(
        "SELECT COALESCE(is_guest, 0) FROM users WHERE id = ?1",
        params![user_id],
        |row| row.get(0),
    )?;
    Ok(is_guest == 1)
}

/// Update user's last activity timestamp
pub fn update_last_activity(conn: &Connection, user_id: i64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE users SET last_activity_at = ?1 WHERE id = ?2",
        params![now, user_id],
    )?;
    Ok(())
}

/// Cleanup expired guest accounts, returns count of deleted users
/// Also deletes their sessions and returns list of usernames for directory cleanup
pub fn cleanup_expired_guests(conn: &Connection, expiry_hours: i64) -> Result<Vec<String>> {
    let cutoff = (Utc::now() - Duration::hours(expiry_hours)).to_rfc3339();

    // Get usernames of guests to delete (for directory cleanup)
    let mut stmt = conn.prepare(
        "SELECT username FROM users WHERE is_guest = 1 AND last_activity_at < ?1",
    )?;
    let usernames: Vec<String> = stmt
        .query_map(params![cutoff], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // Delete expired guests (sessions cascade delete)
    conn.execute(
        "DELETE FROM users WHERE is_guest = 1 AND last_activity_at < ?1",
        params![cutoff],
    )?;

    Ok(usernames)
}

/// Delete all guest accounts, returns list of usernames for directory cleanup
pub fn delete_all_guests(conn: &Connection) -> Result<Vec<String>> {
    // Get usernames first
    let mut stmt = conn.prepare("SELECT username FROM users WHERE is_guest = 1")?;
    let usernames: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // Delete all guests
    conn.execute("DELETE FROM users WHERE is_guest = 1", [])?;

    Ok(usernames)
}

/// Delete a specific user by ID, returns username for directory cleanup
pub fn delete_user(conn: &Connection, user_id: i64) -> Result<Option<String>> {
    // Get username first
    let username: Option<String> = conn
        .query_row(
            "SELECT username FROM users WHERE id = ?1",
            params![user_id],
            |row| row.get(0),
        )
        .ok();

    if username.is_some() {
        conn.execute("DELETE FROM users WHERE id = ?1", params![user_id])?;
    }

    Ok(username)
}

// ==================== App Settings ====================

/// Get an app setting value
pub fn get_app_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let result = conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    );
    match result {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Set an app setting value
pub fn set_app_setting(conn: &Connection, key: &str, value: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

/// Get max users limit (None = infinite, Some(0) = disabled, Some(n) = limit)
pub fn get_max_users(conn: &Connection) -> Result<Option<i64>> {
    get_app_setting(conn, "max_users")?
        .map(|s| s.parse::<i64>())
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)
}

/// Get max guests limit (None = infinite, Some(0) = disabled, Some(n) = limit)
pub fn get_max_guests(conn: &Connection) -> Result<Option<i64>> {
    get_app_setting(conn, "max_guests")?
        .map(|s| s.parse::<i64>())
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)
}

/// Get guest expiry hours (default 24)
pub fn get_guest_expiry_hours(conn: &Connection) -> Result<i64> {
    get_app_setting(conn, "guest_expiry_hours")?
        .and_then(|s| s.parse::<i64>().ok())
        .map(Ok)
        .unwrap_or(Ok(24))
}

/// Check if registration is allowed (under max_users limit)
pub fn can_register_user(conn: &Connection) -> Result<bool> {
    match get_max_users(conn)? {
        None => Ok(true),         // No limit
        Some(0) => Ok(false),     // Registration disabled
        Some(max) => {
            let count = get_regular_user_count(conn)?;
            Ok(count < max)
        }
    }
}

/// Check if guest login is allowed (under max_guests limit)
pub fn can_create_guest(conn: &Connection) -> Result<bool> {
    match get_max_guests(conn)? {
        None => Ok(true),         // No limit
        Some(0) => Ok(false),     // Guests disabled
        Some(max) => {
            let count = get_guest_count(conn)?;
            Ok(count < max)
        }
    }
}

// ==================== User Groups ====================

/// User group info
pub struct UserGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
}

/// Create a new user group
pub fn create_user_group(
    conn: &Connection,
    id: &str,
    name: &str,
    description: Option<&str>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO user_groups (id, name, description, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, name, description, now],
    )?;
    Ok(())
}

/// Get all user groups
pub fn get_all_groups(conn: &Connection) -> Result<Vec<UserGroup>> {
    let mut stmt = conn.prepare("SELECT id, name, description, created_at FROM user_groups ORDER BY name")?;
    let groups = stmt
        .query_map([], |row| {
            Ok(UserGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(groups)
}

/// Get a user group by ID
pub fn get_group(conn: &Connection, group_id: &str) -> Result<Option<UserGroup>> {
    let result = conn.query_row(
        "SELECT id, name, description, created_at FROM user_groups WHERE id = ?1",
        params![group_id],
        |row| {
            Ok(UserGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
            })
        },
    );
    match result {
        Ok(group) => Ok(Some(group)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Delete a user group
pub fn delete_group(conn: &Connection, group_id: &str) -> Result<()> {
    conn.execute("DELETE FROM user_groups WHERE id = ?1", params![group_id])?;
    Ok(())
}

/// Add a user to a group
pub fn add_user_to_group(conn: &Connection, user_id: i64, group_id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO user_group_members (group_id, user_id, added_at) VALUES (?1, ?2, ?3)",
        params![group_id, user_id, now],
    )?;
    Ok(())
}

/// Remove a user from a group
pub fn remove_user_from_group(conn: &Connection, user_id: i64, group_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM user_group_members WHERE group_id = ?1 AND user_id = ?2",
        params![group_id, user_id],
    )?;
    Ok(())
}

/// Get all groups a user belongs to
pub fn get_user_groups(conn: &Connection, user_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT group_id FROM user_group_members WHERE user_id = ?1",
    )?;
    let groups = stmt
        .query_map(params![user_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(groups)
}

/// Check if a user is in a specific group
pub fn is_user_in_group(conn: &Connection, user_id: i64, group_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM user_group_members WHERE group_id = ?1 AND user_id = ?2",
        params![group_id, user_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Get all users in a group
pub fn get_group_members(conn: &Connection, group_id: &str) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        r#"SELECT u.id, u.username
           FROM users u
           JOIN user_group_members m ON u.id = m.user_id
           WHERE m.group_id = ?1
           ORDER BY u.username"#,
    )?;
    let members = stmt
        .query_map(params![group_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(members)
}

// ==================== Pack Permissions ====================

/// Set pack permission for a group (or all users if group_id is empty string)
pub fn set_pack_permission(
    conn: &Connection,
    pack_id: &str,
    group_id: &str,  // Empty string "" = all users
    allowed: bool,
) -> Result<()> {
    conn.execute(
        r#"INSERT OR REPLACE INTO pack_permissions (pack_id, group_id, allowed)
           VALUES (?1, ?2, ?3)"#,
        params![pack_id, group_id, allowed as i32],
    )?;
    Ok(())
}

/// Remove pack permission entry
pub fn remove_pack_permission(conn: &Connection, pack_id: &str, group_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM pack_permissions WHERE pack_id = ?1 AND group_id = ?2",
        params![pack_id, group_id],
    )?;
    Ok(())
}

/// Check if a pack is globally enabled (admin has activated it)
pub fn is_pack_globally_enabled(conn: &Connection, pack_id: &str) -> Result<bool> {
    let enabled: i64 = conn
        .query_row(
            "SELECT COALESCE(is_enabled, 1) FROM content_packs WHERE id = ?1",
            params![pack_id],
            |row| row.get(0),
        )
        .unwrap_or(0); // Pack not in content_packs = disabled by default
    Ok(enabled == 1)
}

/// Set global enabled state for a pack
pub fn set_pack_globally_enabled(conn: &Connection, pack_id: &str, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE content_packs SET is_enabled = ?1 WHERE id = ?2",
        params![if enabled { 1 } else { 0 }, pack_id],
    )?;
    Ok(())
}

/// Check if a pack is public (available to all users)
pub fn is_pack_public(conn: &Connection, pack_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pack_permissions WHERE pack_id = ?1 AND group_id = '' AND allowed = 1",
        params![pack_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Make a pack public (available to all users)
pub fn set_pack_public(conn: &Connection, pack_id: &str, public: bool) -> Result<()> {
    if public {
        conn.execute(
            "INSERT OR REPLACE INTO pack_permissions (pack_id, group_id, allowed) VALUES (?1, '', 1)",
            params![pack_id],
        )?;
    } else {
        conn.execute(
            "DELETE FROM pack_permissions WHERE pack_id = ?1 AND group_id = ''",
            params![pack_id],
        )?;
    }
    Ok(())
}

/// Check if a user can access a pack.
/// Returns true if any of the following conditions are met:
/// - Pack is globally enabled AND user is an admin
/// - Pack has public access (group_id='' and allowed=1)
/// - User is in an allowed group
/// - User has direct access permission
pub fn can_user_access_pack(conn: &Connection, user_id: i64, pack_id: &str) -> Result<bool> {
    // First check if pack is globally enabled
    if !is_pack_globally_enabled(conn, pack_id)? {
        return Ok(false);
    }

    // Admins can access everything that's enabled
    if is_user_admin(conn, user_id)? {
        return Ok(true);
    }

    // Packs without permission entries are admin-only by default
    if !is_pack_restricted(conn, pack_id)? {
        return Ok(false);
    }

    // Pack has permission entries - check specific permissions

    // Check for public flag (group_id = '', allowed = 1)
    let is_public: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pack_permissions WHERE pack_id = ?1 AND group_id = '' AND allowed = 1",
        params![pack_id],
        |row| row.get(0),
    )?;

    if is_public > 0 {
        return Ok(true);
    }

    // Check if user has direct access
    let has_direct_access: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pack_user_permissions WHERE pack_id = ?1 AND user_id = ?2 AND allowed = 1",
        params![pack_id, user_id],
        |row| row.get(0),
    )?;

    if has_direct_access > 0 {
        return Ok(true);
    }

    // Check if user is in any allowed group
    let in_allowed_group: i64 = conn.query_row(
        r#"SELECT COUNT(*) FROM pack_permissions p
           JOIN user_group_members m ON p.group_id = m.group_id
           WHERE p.pack_id = ?1 AND p.allowed = 1 AND m.user_id = ?2 AND p.group_id != ''"#,
        params![pack_id, user_id],
        |row| row.get(0),
    )?;

    Ok(in_allowed_group > 0)
}

/// Get all pack IDs that a user can access (for global packs)
/// This returns packs where: pack is globally enabled AND (user is admin, pack is public, user has direct access, or user is in allowed group)
pub fn list_accessible_pack_ids(conn: &Connection, user_id: i64) -> Result<Vec<String>> {
    let is_admin = is_user_admin(conn, user_id)?;

    if is_admin {
        // Admin can access all ENABLED packs that have cards
        let mut stmt = conn.prepare(
            r#"SELECT DISTINCT cd.pack_id FROM card_definitions cd
               JOIN content_packs cp ON cd.pack_id = cp.id
               WHERE cd.pack_id IS NOT NULL AND COALESCE(cp.is_enabled, 1) = 1"#
        )?;
        let packs = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        return Ok(packs);
    }

    // For non-admins: get ENABLED packs they have access to via permissions
    let mut stmt = conn.prepare(
        r#"SELECT DISTINCT pack_id FROM (
            -- Public packs (group_id = '', allowed = 1)
            SELECT pp.pack_id FROM pack_permissions pp
            JOIN content_packs cp ON pp.pack_id = cp.id
            WHERE pp.group_id = '' AND pp.allowed = 1 AND COALESCE(cp.is_enabled, 1) = 1
            UNION
            -- Direct user permission
            SELECT pup.pack_id FROM pack_user_permissions pup
            JOIN content_packs cp ON pup.pack_id = cp.id
            WHERE pup.user_id = ?1 AND pup.allowed = 1 AND COALESCE(cp.is_enabled, 1) = 1
            UNION
            -- Group permission
            SELECT p.pack_id FROM pack_permissions p
            JOIN user_group_members m ON p.group_id = m.group_id
            JOIN content_packs cp ON p.pack_id = cp.id
            WHERE m.user_id = ?1 AND p.allowed = 1 AND p.group_id != '' AND COALESCE(cp.is_enabled, 1) = 1
        )"#
    )?;
    let packs = stmt
        .query_map(params![user_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(packs)
}

/// Get the groups that have access to a specific pack
/// Returns empty vec if pack is available to all (no restrictions)
pub fn get_pack_allowed_groups(conn: &Connection, pack_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT group_id FROM pack_permissions WHERE pack_id = ?1 AND allowed = 1 AND group_id != ''"
    )?;
    let groups = stmt
        .query_map(params![pack_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(groups)
}

/// Check if a pack has any permission entries (used for access logic)
pub fn is_pack_restricted(conn: &Connection, pack_id: &str) -> Result<bool> {
    let group_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pack_permissions WHERE pack_id = ?1",
        params![pack_id],
        |row| row.get(0),
    )?;
    let user_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pack_user_permissions WHERE pack_id = ?1",
        params![pack_id],
        |row| row.get(0),
    )?;
    Ok(group_count > 0 || user_count > 0)
}

/// Check if a pack has specific user/group restrictions (for UI label)
/// Returns false if pack only has public permission or no permissions
pub fn is_pack_restricted_for_ui(conn: &Connection, pack_id: &str) -> Result<bool> {
    let group_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pack_permissions WHERE pack_id = ?1 AND group_id != ''",
        params![pack_id],
        |row| row.get(0),
    )?;
    let user_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pack_user_permissions WHERE pack_id = ?1",
        params![pack_id],
        |row| row.get(0),
    )?;
    Ok(group_count > 0 || user_count > 0)
}

// ==================== User-Level Pack Permissions ====================

/// Set pack permission for a specific user
pub fn set_pack_user_permission(
    conn: &Connection,
    pack_id: &str,
    user_id: i64,
    allowed: bool,
) -> Result<()> {
    conn.execute(
        r#"INSERT OR REPLACE INTO pack_user_permissions (pack_id, user_id, allowed)
           VALUES (?1, ?2, ?3)"#,
        params![pack_id, user_id, allowed as i32],
    )?;
    Ok(())
}

/// Remove pack permission for a specific user
pub fn remove_pack_user_permission(conn: &Connection, pack_id: &str, user_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM pack_user_permissions WHERE pack_id = ?1 AND user_id = ?2",
        params![pack_id, user_id],
    )?;
    Ok(())
}

/// Get the users that have direct access to a specific pack
/// Returns vec of (user_id, username) tuples
pub fn get_pack_allowed_users(conn: &Connection, pack_id: &str) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        r#"SELECT pup.user_id, u.username
           FROM pack_user_permissions pup
           JOIN users u ON pup.user_id = u.id
           WHERE pup.pack_id = ?1 AND pup.allowed = 1"#
    )?;
    let users = stmt
        .query_map(params![pack_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(users)
}

/// Clear all permissions for a pack (both groups and users)
pub fn clear_pack_permissions(conn: &Connection, pack_id: &str) -> Result<()> {
    conn.execute("DELETE FROM pack_permissions WHERE pack_id = ?1", params![pack_id])?;
    conn.execute("DELETE FROM pack_user_permissions WHERE pack_id = ?1", params![pack_id])?;
    Ok(())
}

/// Get all packs a user can access
pub fn get_accessible_packs(conn: &Connection, user_id: i64) -> Result<Vec<String>> {
    // If admin, return all packs
    if is_user_admin(conn, user_id)? {
        let mut stmt = conn.prepare("SELECT id FROM content_packs")?;
        let packs = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        return Ok(packs);
    }

    // Get all pack IDs
    let mut stmt = conn.prepare("SELECT id FROM content_packs")?;
    let all_packs: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // Filter by permission
    let accessible: Vec<String> = all_packs
        .into_iter()
        .filter(|pack_id| can_user_access_pack(conn, user_id, pack_id).unwrap_or(false))
        .collect();

    Ok(accessible)
}

// ============================================================================
// Registered Pack Paths (Admin-configured external directories)
// ============================================================================

/// A registered external pack path
#[derive(Debug, Clone)]
pub struct RegisteredPackPath {
    pub id: i64,
    pub path: String,
    pub name: Option<String>,
    pub registered_by: String,
    pub registered_at: String,
    pub is_active: bool,
}

/// Register a new external pack path
pub fn register_pack_path(
    conn: &Connection,
    path: &str,
    name: Option<&str>,
    registered_by: &str,
) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        r#"INSERT INTO registered_pack_paths (path, name, registered_by, registered_at, is_active)
           VALUES (?1, ?2, ?3, ?4, 1)"#,
        params![path, name, registered_by, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Unregister (delete) a pack path
pub fn unregister_pack_path(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM registered_pack_paths WHERE id = ?1", params![id])?;
    Ok(())
}

/// Toggle a pack path's active status
pub fn toggle_pack_path_active(conn: &Connection, id: i64) -> Result<bool> {
    conn.execute(
        "UPDATE registered_pack_paths SET is_active = NOT is_active WHERE id = ?1",
        params![id],
    )?;
    // Return the new state
    let is_active: bool = conn.query_row(
        "SELECT is_active FROM registered_pack_paths WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    Ok(is_active)
}

/// Get all registered pack paths
pub fn get_all_registered_paths(conn: &Connection) -> Result<Vec<RegisteredPackPath>> {
    let mut stmt = conn.prepare(
        r#"SELECT id, path, name, registered_by, registered_at, is_active
           FROM registered_pack_paths
           ORDER BY registered_at DESC"#,
    )?;
    let paths = stmt
        .query_map([], |row| {
            Ok(RegisteredPackPath {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                registered_by: row.get(3)?,
                registered_at: row.get(4)?,
                is_active: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(paths)
}

/// Get only active registered pack paths
pub fn get_active_registered_paths(conn: &Connection) -> Result<Vec<RegisteredPackPath>> {
    let mut stmt = conn.prepare(
        r#"SELECT id, path, name, registered_by, registered_at, is_active
           FROM registered_pack_paths
           WHERE is_active = 1
           ORDER BY registered_at DESC"#,
    )?;
    let paths = stmt
        .query_map([], |row| {
            Ok(RegisteredPackPath {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                registered_by: row.get(3)?,
                registered_at: row.get(4)?,
                is_active: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(paths)
}

/// Get a single registered pack path by ID
pub fn get_registered_path(conn: &Connection, id: i64) -> Result<Option<RegisteredPackPath>> {
    let result = conn.query_row(
        r#"SELECT id, path, name, registered_by, registered_at, is_active
           FROM registered_pack_paths
           WHERE id = ?1"#,
        params![id],
        |row| {
            Ok(RegisteredPackPath {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                registered_by: row.get(3)?,
                registered_at: row.get(4)?,
                is_active: row.get(5)?,
            })
        },
    );
    match result {
        Ok(path) => Ok(Some(path)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Check if a path is already registered
pub fn is_path_registered(conn: &Connection, path: &str) -> Result<bool> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM registered_pack_paths WHERE path = ?1",
        params![path],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}
