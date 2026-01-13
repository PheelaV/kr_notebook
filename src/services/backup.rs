//! Backup module for portable export/import of user learning data.
//!
//! This module provides stable card identification via content hashing,
//! enabling progress to be transferred between installs even when card IDs differ.
//!
//! ## Export Format
//! ```text
//! kr_notebook_{username}_{date}.zip
//! ├── learning.db    # User's progress database
//! └── manifest.json  # Metadata with hashed card mappings
//! ```
//!
//! ## Privacy
//! Card hashes are one-way (SHA256) - no content leakage.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read as IoRead, Write as IoWrite};
use std::path::Path;
use zip::write::SimpleFileOptions;

/// Export manifest format version
pub const MANIFEST_VERSION: u32 = 1;

/// Card ID to hash mapping for export
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CardMapping {
    pub id: i64,
    pub hash: String,
}

/// Export manifest containing metadata and card mappings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    /// Format version for future compatibility
    pub format_version: u32,
    /// ISO8601 timestamp of export
    pub exported_at: String,
    /// Application version at export time
    pub app_version: String,
    /// Card ID to hash mappings
    pub card_mappings: Vec<CardMapping>,
}

/// Result of import operation
#[derive(Debug)]
pub struct ImportResult {
    /// Number of cards successfully remapped
    pub cards_matched: usize,
    /// IDs from export that couldn't be matched locally
    pub unmapped_ids: Vec<i64>,
    /// True if there was a version mismatch warning
    pub version_warning: bool,
}

/// Compute stable hash for a card based on its content.
///
/// Uses SHA256 of `pack_id:front:main_answer:card_type`.
/// Returns first 32 hex chars (128 bits) for practical uniqueness.
pub fn compute_card_hash(pack_id: &str, front: &str, main_answer: &str, card_type: &str) -> String {
    let input = format!("{}:{}:{}:{}", pack_id, front, main_answer, card_type);
    let hash = Sha256::digest(input.as_bytes());
    hex::encode(&hash[..16]) // 16 bytes = 32 hex chars
}

/// Build card mappings for export by querying all cards with progress.
///
/// Requires app.db to be attached as 'app' in the connection.
pub fn build_export_mappings(conn: &Connection) -> Result<Vec<CardMapping>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        r#"
        SELECT cp.card_id, COALESCE(cd.pack_id, 'baseline'), cd.front, cd.main_answer, cd.card_type
        FROM card_progress cp
        INNER JOIN app.card_definitions cd ON cp.card_id = cd.id
        "#,
    )?;

    let mappings = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let pack_id: String = row.get(1)?;
            let front: String = row.get(2)?;
            let main_answer: String = row.get(3)?;
            let card_type: String = row.get(4)?;
            Ok(CardMapping {
                id,
                hash: compute_card_hash(&pack_id, &front, &main_answer, &card_type),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(mappings)
}

/// Build local hash lookup table from card_definitions.
///
/// Returns HashMap from hash to card ID.
pub fn build_local_hash_table(conn: &Connection) -> Result<HashMap<String, i64>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, COALESCE(pack_id, 'baseline'), front, main_answer, card_type
        FROM card_definitions
        "#,
    )?;

    let mut hash_map = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let pack_id: String = row.get(1)?;
        let front: String = row.get(2)?;
        let main_answer: String = row.get(3)?;
        let card_type: String = row.get(4)?;
        Ok((id, pack_id, front, main_answer, card_type))
    })?;

    for row in rows {
        let (id, pack_id, front, main_answer, card_type) = row?;
        let hash = compute_card_hash(&pack_id, &front, &main_answer, &card_type);
        hash_map.insert(hash, id);
    }

    Ok(hash_map)
}

/// Build remap table from exported mappings and local hashes.
///
/// Returns:
/// - HashMap from old_id to new_id (for successful matches)
/// - Vec of old_ids that couldn't be matched
pub fn build_remap_table(
    mappings: &[CardMapping],
    local_hashes: &HashMap<String, i64>,
) -> (HashMap<i64, i64>, Vec<i64>) {
    let mut remap = HashMap::new();
    let mut unmapped = Vec::new();

    for mapping in mappings {
        if let Some(&new_id) = local_hashes.get(&mapping.hash) {
            remap.insert(mapping.id, new_id);
        } else {
            unmapped.push(mapping.id);
        }
    }

    (remap, unmapped)
}

/// Remap card_ids in the imported database.
///
/// Updates card_progress, review_logs, and confusions tables.
pub fn remap_card_ids(conn: &Connection, remap: &HashMap<i64, i64>) -> Result<(), rusqlite::Error> {
    if remap.is_empty() {
        return Ok(());
    }

    // Build a temporary table with the remapping
    conn.execute_batch(
        r#"
        CREATE TEMP TABLE IF NOT EXISTS id_remap (
            old_id INTEGER PRIMARY KEY,
            new_id INTEGER NOT NULL
        );
        DELETE FROM id_remap;
        "#,
    )?;

    let mut insert_stmt = conn.prepare("INSERT INTO id_remap (old_id, new_id) VALUES (?1, ?2)")?;
    for (&old_id, &new_id) in remap {
        insert_stmt.execute([old_id, new_id])?;
    }
    drop(insert_stmt);

    // Update card_progress
    conn.execute(
        r#"
        UPDATE card_progress
        SET card_id = (SELECT new_id FROM id_remap WHERE old_id = card_progress.card_id)
        WHERE card_id IN (SELECT old_id FROM id_remap)
        "#,
        [],
    )?;

    // Update review_logs
    conn.execute(
        r#"
        UPDATE review_logs
        SET card_id = (SELECT new_id FROM id_remap WHERE old_id = review_logs.card_id)
        WHERE card_id IN (SELECT old_id FROM id_remap)
        "#,
        [],
    )?;

    // Update confusions
    conn.execute(
        r#"
        UPDATE confusions
        SET card_id = (SELECT new_id FROM id_remap WHERE old_id = confusions.card_id)
        WHERE card_id IN (SELECT old_id FROM id_remap)
        "#,
        [],
    )?;

    // Clean up
    conn.execute("DROP TABLE IF EXISTS id_remap", [])?;

    Ok(())
}

/// Delete card_progress entries for unmapped cards.
///
/// These are cards that existed in the export but don't exist locally.
pub fn delete_unmapped_progress(conn: &Connection, unmapped_ids: &[i64]) -> Result<usize, rusqlite::Error> {
    if unmapped_ids.is_empty() {
        return Ok(0);
    }

    let placeholders = unmapped_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    let deleted = conn.execute(
        &format!("DELETE FROM card_progress WHERE card_id IN ({})", placeholders),
        rusqlite::params_from_iter(unmapped_ids.iter()),
    )?;

    // Also delete orphaned review_logs and confusions
    conn.execute(
        &format!("DELETE FROM review_logs WHERE card_id IN ({})", placeholders),
        rusqlite::params_from_iter(unmapped_ids.iter()),
    )?;

    conn.execute(
        &format!("DELETE FROM confusions WHERE card_id IN ({})", placeholders),
        rusqlite::params_from_iter(unmapped_ids.iter()),
    )?;

    Ok(deleted)
}

/// Check if export version is compatible with current version.
///
/// We allow any version within the same major version number.
/// Pre-1.0 versions (0.x.y) are all considered compatible with each other.
pub fn check_version_compatible(export_version: &str, current_version: &str) -> bool {
    let parse_major = |v: &str| -> Option<u32> {
        v.split('.').next()?.parse().ok()
    };

    match (parse_major(export_version), parse_major(current_version)) {
        (Some(export_major), Some(current_major)) => {
            // For pre-1.0, all 0.x versions are compatible
            if export_major == 0 && current_major == 0 {
                return true;
            }
            export_major == current_major
        }
        _ => false,
    }
}

/// Create export ZIP archive containing database and manifest.
pub fn create_export_zip(
    db_path: &Path,
    manifest: &ExportManifest,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let db_bytes = std::fs::read(db_path)?;
    let manifest_json = serde_json::to_string_pretty(manifest)?;

    let mut zip_buffer = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buffer));
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // Add learning.db
        zip.start_file("learning.db", options)?;
        zip.write_all(&db_bytes)?;

        // Add manifest.json
        zip.start_file("manifest.json", options)?;
        zip.write_all(manifest_json.as_bytes())?;

        zip.finish()?;
    }

    Ok(zip_buffer)
}

/// Extract and validate import ZIP archive.
///
/// Returns the database bytes and parsed manifest.
pub fn extract_import_zip(
    bytes: &[u8],
) -> Result<(Vec<u8>, ExportManifest), Box<dyn std::error::Error + Send + Sync>> {
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader)?;

    // Extract manifest first
    let manifest: ExportManifest = {
        let mut manifest_file = zip.by_name("manifest.json").map_err(|_| {
            "Invalid export file: missing manifest.json. Please use a ZIP file exported from this app."
        })?;
        let mut manifest_content = String::new();
        manifest_file.read_to_string(&mut manifest_content)?;
        serde_json::from_str(&manifest_content)?
    };

    // Check manifest version
    if manifest.format_version > MANIFEST_VERSION {
        return Err(format!(
            "Export format version {} is newer than supported version {}. Please update the app.",
            manifest.format_version, MANIFEST_VERSION
        ).into());
    }

    // Extract database
    let db_bytes = {
        let mut db_file = zip.by_name("learning.db").map_err(|_| {
            "Invalid export file: missing learning.db"
        })?;
        let mut bytes = Vec::new();
        db_file.read_to_end(&mut bytes)?;
        bytes
    };

    // Validate SQLite header
    if db_bytes.len() < 16 || &db_bytes[0..16] != b"SQLite format 3\0" {
        return Err("Invalid export file: learning.db is not a valid SQLite database".into());
    }

    Ok((db_bytes, manifest))
}

/// Check if bytes look like a ZIP file (magic number check)
pub fn is_zip_file(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[0..4] == b"PK\x03\x04"
}

/// Check if bytes look like a raw SQLite file
pub fn is_sqlite_file(bytes: &[u8]) -> bool {
    bytes.len() >= 16 && &bytes[0..16] == b"SQLite format 3\0"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_card_hash_deterministic() {
        let h1 = compute_card_hash("pack1", "hello", "안녕", "vocab");
        let h2 = compute_card_hash("pack1", "hello", "안녕", "vocab");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_card_hash_different_for_different_cards() {
        let h1 = compute_card_hash("pack1", "hello", "안녕", "vocab");
        let h2 = compute_card_hash("pack1", "goodbye", "안녕", "vocab");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_compute_card_hash_different_for_different_packs() {
        let h1 = compute_card_hash("pack1", "hello", "안녕", "vocab");
        let h2 = compute_card_hash("pack2", "hello", "안녕", "vocab");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_compute_card_hash_length() {
        let h = compute_card_hash("pack1", "hello", "안녕", "vocab");
        assert_eq!(h.len(), 32); // 16 bytes = 32 hex chars
    }

    #[test]
    fn test_build_remap_table_full_match() {
        let mappings = vec![
            CardMapping { id: 100, hash: "abc123".into() },
            CardMapping { id: 200, hash: "def456".into() },
        ];
        let local = HashMap::from([
            ("abc123".into(), 1),
            ("def456".into(), 2),
        ]);

        let (remap, unmapped) = build_remap_table(&mappings, &local);

        assert_eq!(remap.get(&100), Some(&1));
        assert_eq!(remap.get(&200), Some(&2));
        assert!(unmapped.is_empty());
    }

    #[test]
    fn test_build_remap_table_partial_match() {
        let mappings = vec![
            CardMapping { id: 100, hash: "abc123".into() },
            CardMapping { id: 200, hash: "xyz789".into() }, // Not in local
        ];
        let local = HashMap::from([("abc123".into(), 1)]);

        let (remap, unmapped) = build_remap_table(&mappings, &local);

        assert_eq!(remap.len(), 1);
        assert_eq!(remap.get(&100), Some(&1));
        assert_eq!(unmapped, vec![200]);
    }

    #[test]
    fn test_build_remap_table_no_match() {
        let mappings = vec![
            CardMapping { id: 100, hash: "abc123".into() },
        ];
        let local = HashMap::new();

        let (remap, unmapped) = build_remap_table(&mappings, &local);

        assert!(remap.is_empty());
        assert_eq!(unmapped, vec![100]);
    }

    #[test]
    fn test_version_compatible_same_major() {
        assert!(check_version_compatible("1.0.0", "1.5.0"));
        assert!(check_version_compatible("1.2.3", "1.0.0"));
        assert!(check_version_compatible("2.0.0", "2.99.99"));
    }

    #[test]
    fn test_version_compatible_different_major() {
        assert!(!check_version_compatible("1.0.0", "2.0.0"));
        assert!(!check_version_compatible("2.5.0", "1.0.0"));
    }

    #[test]
    fn test_version_compatible_pre_release() {
        // All 0.x versions are compatible with each other
        assert!(check_version_compatible("0.1.0", "0.2.0"));
        assert!(check_version_compatible("0.2.0", "0.5.0"));
        assert!(check_version_compatible("0.9.9", "0.1.0"));
    }

    #[test]
    fn test_version_compatible_pre_to_stable() {
        // 0.x is not compatible with 1.x
        assert!(!check_version_compatible("0.9.0", "1.0.0"));
        assert!(!check_version_compatible("1.0.0", "0.9.0"));
    }

    #[test]
    fn test_version_compatible_invalid() {
        assert!(!check_version_compatible("invalid", "1.0.0"));
        assert!(!check_version_compatible("1.0.0", ""));
    }

    #[test]
    fn test_is_zip_file() {
        let zip_header = b"PK\x03\x04test";
        let not_zip = b"notazip";
        let sqlite = b"SQLite format 3\0";

        assert!(is_zip_file(zip_header));
        assert!(!is_zip_file(not_zip));
        assert!(!is_zip_file(sqlite));
    }

    #[test]
    fn test_is_sqlite_file() {
        let sqlite = b"SQLite format 3\0extradata";
        let not_sqlite = b"notadatabase";
        let zip = b"PK\x03\x04test";

        assert!(is_sqlite_file(sqlite));
        assert!(!is_sqlite_file(not_sqlite));
        assert!(!is_sqlite_file(zip));
    }

    #[test]
    fn test_manifest_serialization() {
        let manifest = ExportManifest {
            format_version: 1,
            exported_at: "2026-01-12T10:30:00Z".to_string(),
            app_version: "0.2.0".to_string(),
            card_mappings: vec![
                CardMapping { id: 100, hash: "abc123".into() },
            ],
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: ExportManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.format_version, 1);
        assert_eq!(parsed.card_mappings.len(), 1);
        assert_eq!(parsed.card_mappings[0].id, 100);
    }

    #[test]
    fn test_create_and_extract_zip() {
        use tempfile::NamedTempFile;

        // Create a minimal SQLite database
        let temp_db = NamedTempFile::new().unwrap();
        let conn = Connection::open(temp_db.path()).unwrap();
        conn.execute_batch("CREATE TABLE test (id INTEGER);").unwrap();
        drop(conn);

        let manifest = ExportManifest {
            format_version: 1,
            exported_at: "2026-01-12T10:30:00Z".to_string(),
            app_version: "0.2.0".to_string(),
            card_mappings: vec![
                CardMapping { id: 100, hash: "abc123".into() },
            ],
        };

        // Create ZIP
        let zip_bytes = create_export_zip(temp_db.path(), &manifest).unwrap();

        // Verify it's a ZIP
        assert!(is_zip_file(&zip_bytes));

        // Extract and verify
        let (db_bytes, extracted_manifest) = extract_import_zip(&zip_bytes).unwrap();

        assert!(is_sqlite_file(&db_bytes));
        assert_eq!(extracted_manifest.format_version, 1);
        assert_eq!(extracted_manifest.card_mappings.len(), 1);
    }
}
