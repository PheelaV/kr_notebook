//! Pack discovery - scanning directories for pack manifests.

use std::fs;
use std::path::{Path, PathBuf};

use super::packs::{PackError, PackManifest};
use super::PackScope;

/// Location where a pack was discovered.
#[derive(Debug, Clone)]
pub struct PackLocation {
    /// The pack manifest
    pub manifest: PackManifest,
    /// Absolute path to the pack directory
    pub path: PathBuf,
    /// Whether this is a shared or user pack
    pub scope: PackScope,
    /// Username if this is a user pack
    pub username: Option<String>,
}

/// Discover all packs in a directory.
///
/// Scans immediate subdirectories for `pack.json` files.
/// Returns successfully parsed packs; logs errors for invalid ones.
pub fn scan_pack_directory(
    dir: &Path,
    scope: PackScope,
    username: Option<&str>,
) -> Vec<PackLocation> {
    let mut packs = Vec::new();

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return packs, // Directory doesn't exist or not readable
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        match PackManifest::load(&path) {
            Ok(manifest) => {
                packs.push(PackLocation {
                    manifest,
                    path,
                    scope,
                    username: username.map(String::from),
                });
            }
            Err(PackError::ManifestNotFound(_)) => {
                // Not a pack directory, skip silently
            }
            Err(e) => {
                // Log parse/validation errors but continue scanning
                tracing::warn!("Invalid pack at {}: {}", path.display(), e);
            }
        }
    }

    packs
}

/// Discover all packs (shared and user-specific).
///
/// # Arguments
/// * `shared_packs_dir` - Path to shared packs (e.g., `data/content/packs`)
/// * `user_packs_dir` - Optional path to user packs (e.g., `data/users/{username}/content/packs`)
/// * `username` - Username for user packs
pub fn discover_packs(
    shared_packs_dir: &Path,
    user_packs_dir: Option<&Path>,
    username: Option<&str>,
) -> Vec<PackLocation> {
    let mut packs = Vec::new();

    // Scan shared packs
    packs.extend(scan_pack_directory(shared_packs_dir, PackScope::Shared, None));

    // Scan user packs if provided
    if let (Some(user_dir), Some(user)) = (user_packs_dir, username) {
        packs.extend(scan_pack_directory(user_dir, PackScope::User, Some(user)));
    }

    packs
}

/// Discover all packs including external registered paths.
///
/// # Arguments
/// * `shared_packs_dir` - Path to shared packs (e.g., `data/content/packs`)
/// * `user_packs_dir` - Optional path to user packs
/// * `username` - Username for user packs
/// * `external_paths` - List of admin-registered external pack paths
pub fn discover_packs_with_external(
    shared_packs_dir: &Path,
    user_packs_dir: Option<&Path>,
    username: Option<&str>,
    external_paths: &[PathBuf],
) -> Vec<PackLocation> {
    // Start with standard discovery
    let mut packs = discover_packs(shared_packs_dir, user_packs_dir, username);

    // Add packs from external paths
    for path in external_paths {
        packs.extend(scan_pack_directory(path, PackScope::External, None));
    }

    packs
}

/// Count valid packs in a directory (for UI feedback).
pub fn count_packs_in_directory(dir: &Path) -> usize {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };

    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| e.path().join("pack.json").exists())
        .count()
}

/// Check if a specific pack exists at a path.
pub fn pack_exists(pack_dir: &Path) -> bool {
    pack_dir.join("pack.json").exists()
}

/// Check if any available pack provides a specific content type.
///
/// Scans shared packs directory for packs with matching `provides` field.
pub fn any_pack_provides(shared_packs_dir: &Path, content_type: &str) -> bool {
    let packs = scan_pack_directory(shared_packs_dir, PackScope::Shared, None);
    packs
        .iter()
        .any(|p| p.manifest.provides.iter().any(|t| t == content_type))
}

/// Find all packs that provide a specific content type.
///
/// Returns pack IDs of packs with matching `provides` field.
pub fn find_packs_providing(shared_packs_dir: &Path, content_type: &str) -> Vec<String> {
    let packs = scan_pack_directory(shared_packs_dir, PackScope::Shared, None);
    packs
        .into_iter()
        .filter(|p| p.manifest.provides.iter().any(|t| t == content_type))
        .map(|p| p.manifest.id)
        .collect()
}

/// Check if any pack provides a specific content type, including external paths.
///
/// Scans shared packs directory and any provided external paths.
pub fn any_pack_provides_with_external(
    shared_packs_dir: &Path,
    external_paths: &[PathBuf],
    content_type: &str,
) -> bool {
    // Check shared packs first
    let mut packs = scan_pack_directory(shared_packs_dir, PackScope::Shared, None);

    // Add external paths
    for path in external_paths {
        packs.extend(scan_pack_directory(path, PackScope::External, None));
    }

    packs
        .iter()
        .any(|p| p.manifest.provides.iter().any(|t| t == content_type))
}

/// Find all packs that provide a specific content type, including external paths.
///
/// Returns PackLocations (with paths) so caller knows where to load content from.
pub fn find_packs_providing_with_external(
    shared_packs_dir: &Path,
    external_paths: &[PathBuf],
    content_type: &str,
) -> Vec<PackLocation> {
    // Scan shared packs
    let mut packs = scan_pack_directory(shared_packs_dir, PackScope::Shared, None);

    // Add external paths
    for path in external_paths {
        packs.extend(scan_pack_directory(path, PackScope::External, None));
    }

    // Filter to those providing the requested content type
    packs
        .into_iter()
        .filter(|p| p.manifest.provides.iter().any(|t| t == content_type))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_pack(dir: &Path, id: &str, pack_type: &str) {
        let pack_dir = dir.join(id);
        fs::create_dir_all(&pack_dir).unwrap();

        let config_section = match pack_type {
            "audio" => r#""audio": {"enhances": []}"#,
            "generator" => r#""generator": {"command": "test", "subcommands": []}"#,
            "cards" => r#""cards": {"file": "cards.json"}"#,
            _ => "",
        };

        let manifest = format!(
            r#"{{"id": "{}", "name": "Test {}", "type": "{}", {}}}"#,
            id, id, pack_type, config_section
        );
        fs::write(pack_dir.join("pack.json"), manifest).unwrap();
    }

    #[test]
    fn test_scan_empty_directory() {
        let temp = TempDir::new().unwrap();
        let packs = scan_pack_directory(temp.path(), PackScope::Shared, None);
        assert!(packs.is_empty());
    }

    #[test]
    fn test_scan_with_packs() {
        let temp = TempDir::new().unwrap();
        create_test_pack(temp.path(), "pack1", "audio");
        create_test_pack(temp.path(), "pack2", "cards");

        let packs = scan_pack_directory(temp.path(), PackScope::Shared, None);
        assert_eq!(packs.len(), 2);

        let ids: Vec<_> = packs.iter().map(|p| p.manifest.id.as_str()).collect();
        assert!(ids.contains(&"pack1"));
        assert!(ids.contains(&"pack2"));
    }

    #[test]
    fn test_scan_ignores_files() {
        let temp = TempDir::new().unwrap();
        create_test_pack(temp.path(), "valid-pack", "audio");
        fs::write(temp.path().join("not-a-pack.txt"), "hello").unwrap();

        let packs = scan_pack_directory(temp.path(), PackScope::Shared, None);
        assert_eq!(packs.len(), 1);
    }

    #[test]
    fn test_scan_ignores_dirs_without_manifest() {
        let temp = TempDir::new().unwrap();
        create_test_pack(temp.path(), "valid-pack", "audio");
        fs::create_dir_all(temp.path().join("empty-dir")).unwrap();

        let packs = scan_pack_directory(temp.path(), PackScope::Shared, None);
        assert_eq!(packs.len(), 1);
    }

    #[test]
    fn test_discover_combined() {
        let shared = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();

        create_test_pack(shared.path(), "shared-pack", "audio");
        create_test_pack(user.path(), "user-pack", "cards");

        let packs = discover_packs(shared.path(), Some(user.path()), Some("testuser"));
        assert_eq!(packs.len(), 2);

        let shared_pack = packs.iter().find(|p| p.manifest.id == "shared-pack").unwrap();
        assert_eq!(shared_pack.scope, PackScope::Shared);
        assert!(shared_pack.username.is_none());

        let user_pack = packs.iter().find(|p| p.manifest.id == "user-pack").unwrap();
        assert_eq!(user_pack.scope, PackScope::User);
        assert_eq!(user_pack.username.as_deref(), Some("testuser"));
    }
}
