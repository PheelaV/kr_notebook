//! Unified pack management service.
//!
//! Centralizes pack discovery and permission checking that was previously
//! duplicated across 7+ handlers. All handlers should use these functions
//! instead of directly calling discovery and auth functions.

use rusqlite::Connection;
use std::path::PathBuf;

use crate::auth::db as auth_db;
use crate::content::{
    discover_packs_with_external, find_packs_providing_with_external, PackLocation, PackType,
};
use crate::paths;

/// Filter options for pack discovery
#[derive(Debug, Clone, Default)]
pub struct PackFilter {
    /// Filter to packs that provide this content type (e.g., "vocabulary", "audio")
    pub provides: Option<String>,
    /// Filter to specific pack type
    pub pack_type: Option<PackType>,
}

impl PackFilter {
    /// Create a filter for packs providing specific content
    pub fn provides(content_type: &str) -> Self {
        Self {
            provides: Some(content_type.to_string()),
            pack_type: None,
        }
    }

    /// Create a filter for specific pack type
    pub fn pack_type(pack_type: PackType) -> Self {
        Self {
            provides: None,
            pack_type: Some(pack_type),
        }
    }
}

/// Get all external pack paths from the database.
///
/// This consolidates the boilerplate that was repeated in 7+ handlers:
/// ```ignore
/// let external_paths: Vec<PathBuf> = auth_db::get_active_registered_paths(&app_conn)
///     .unwrap_or_default()
///     .into_iter()
///     .map(|p| PathBuf::from(p.path))
///     .collect();
/// ```
pub fn get_external_paths(auth_db: &Connection) -> Vec<PathBuf> {
    auth_db::get_active_registered_paths(auth_db)
        .unwrap_or_default()
        .into_iter()
        .map(|p| PathBuf::from(p.path))
        .collect()
}

/// Get the shared packs directory path.
///
/// Consolidates pack directory access that was repeated in 8+ places.
/// Returns PathBuf since path is now dynamically constructed from DATA_DIR.
pub fn shared_packs_dir() -> PathBuf {
    PathBuf::from(paths::shared_packs_dir())
}

/// Discover all packs from all sources (shared + external).
///
/// Does NOT filter by user permissions - returns all discoverable packs.
pub fn discover_all_packs(auth_db: &Connection) -> Vec<PackLocation> {
    let external_paths = get_external_paths(auth_db);
    discover_packs_with_external(&shared_packs_dir(), None, None, &external_paths)
}

/// Get packs accessible to a specific user.
///
/// Discovers packs and filters by:
/// 1. User permissions (via `can_user_access_pack`)
/// 2. Optional filter criteria (provides, pack_type)
///
/// This is the main entry point handlers should use.
pub fn get_accessible_packs(
    auth_db: &Connection,
    user_id: i64,
    filter: Option<PackFilter>,
) -> Vec<PackLocation> {
    let external_paths = get_external_paths(auth_db);
    let filter = filter.unwrap_or_default();

    // If filtering by provides, use the optimized function
    if let Some(ref content_type) = filter.provides {
        let packs =
            find_packs_providing_with_external(&shared_packs_dir(), &external_paths, content_type);

        return packs
            .into_iter()
            .filter(|p| {
                // Apply pack_type filter if specified
                if let Some(ref pt) = filter.pack_type {
                    if &p.manifest.pack_type != pt {
                        return false;
                    }
                }
                // Check user access
                auth_db::can_user_access_pack(auth_db, user_id, &p.manifest.id).unwrap_or(false)
            })
            .collect();
    }

    // Otherwise discover all and filter
    let all_packs = discover_packs_with_external(&shared_packs_dir(), None, None, &external_paths);

    all_packs
        .into_iter()
        .filter(|p| {
            // Apply pack_type filter if specified
            if let Some(ref pt) = filter.pack_type {
                if &p.manifest.pack_type != pt {
                    return false;
                }
            }
            // Check user access
            auth_db::can_user_access_pack(auth_db, user_id, &p.manifest.id).unwrap_or(false)
        })
        .collect()
}

/// Check if a user can access a specific pack.
///
/// Wrapper around `auth_db::can_user_access_pack` for convenience.
pub fn can_access(auth_db: &Connection, user_id: i64, pack_id: &str) -> bool {
    auth_db::can_user_access_pack(auth_db, user_id, pack_id).unwrap_or(false)
}

/// Check if any pack provides a specific content type (for user).
///
/// Returns true if at least one accessible pack provides the content type.
pub fn any_accessible_pack_provides(
    auth_db: &Connection,
    user_id: i64,
    content_type: &str,
) -> bool {
    let packs = get_accessible_packs(auth_db, user_id, Some(PackFilter::provides(content_type)));
    !packs.is_empty()
}

/// Find a specific pack by ID from all sources.
///
/// Returns the PackLocation if found, regardless of permissions.
/// Caller should check permissions separately if needed.
pub fn find_pack_by_id(auth_db: &Connection, pack_id: &str) -> Option<PackLocation> {
    let all_packs = discover_all_packs(auth_db);
    all_packs.into_iter().find(|p| p.manifest.id == pack_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_filter_provides() {
        let filter = PackFilter::provides("vocabulary");
        assert_eq!(filter.provides, Some("vocabulary".to_string()));
        assert!(filter.pack_type.is_none());
    }

    #[test]
    fn test_pack_filter_pack_type() {
        let filter = PackFilter::pack_type(PackType::Cards);
        assert!(filter.provides.is_none());
        assert_eq!(filter.pack_type, Some(PackType::Cards));
    }

    #[test]
    fn test_shared_packs_dir() {
        let dir = shared_packs_dir();
        assert!(dir.to_str().unwrap().contains("packs"));
    }

    // Additional PackFilter tests

    #[test]
    fn test_pack_filter_default() {
        let filter = PackFilter::default();
        assert!(filter.provides.is_none());
        assert!(filter.pack_type.is_none());
    }

    #[test]
    fn test_pack_filter_provides_audio() {
        let filter = PackFilter::provides("audio");
        assert_eq!(filter.provides, Some("audio".to_string()));
    }

    #[test]
    fn test_pack_filter_provides_empty() {
        let filter = PackFilter::provides("");
        assert_eq!(filter.provides, Some("".to_string()));
    }

    #[test]
    fn test_pack_filter_pack_type_audio() {
        let filter = PackFilter::pack_type(PackType::Audio);
        assert_eq!(filter.pack_type, Some(PackType::Audio));
    }

    #[test]
    fn test_pack_filter_pack_type_generator() {
        let filter = PackFilter::pack_type(PackType::Generator);
        assert_eq!(filter.pack_type, Some(PackType::Generator));
    }

    #[test]
    fn test_pack_filter_clone() {
        let filter = PackFilter::provides("vocabulary");
        let cloned = filter.clone();
        assert_eq!(cloned.provides, filter.provides);
        assert_eq!(cloned.pack_type, filter.pack_type);
    }

    #[test]
    fn test_pack_filter_debug() {
        let filter = PackFilter::provides("audio");
        let debug = format!("{:?}", filter);
        assert!(debug.contains("PackFilter"));
        assert!(debug.contains("audio"));
    }

    #[test]
    fn test_shared_packs_dir_is_static() {
        // Should return the same reference each time
        let dir1 = shared_packs_dir();
        let dir2 = shared_packs_dir();
        assert_eq!(dir1, dir2);
    }

    #[test]
    fn test_shared_packs_dir_is_valid_path() {
        let dir = shared_packs_dir();
        // Should be a valid path that can be converted to str
        assert!(dir.to_str().is_some());
    }
}
