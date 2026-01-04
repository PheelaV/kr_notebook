//! Content pack system for modular content management.
//!
//! This module provides infrastructure for:
//! - **Audio packs**: Pronunciation audio for syllables, rows, columns
//! - **Generator packs**: Scrapers that create content (audio, etc.)
//! - **Card packs**: Additional card sets (vocabulary, grammar, etc.)
//!
//! # Pack Locations
//!
//! - Shared packs: `data/content/packs/` (admin-installed)
//! - User packs: `data/users/{username}/content/packs/` (personal)
//! - Generated content: `data/content/generated/` or user equivalent
//!
//! # Pack Lifecycle
//!
//! 1. **Discovery**: Scan directories for `pack.json` manifests
//! 2. **Registration**: Store pack metadata in `content_packs` table
//! 3. **Enable**: User enables pack, creating entries in `enabled_packs`
//! 4. **Activation**: For card packs, cards are created on enable

pub mod cards;
pub mod discovery;
pub mod packs;

pub use cards::{load_baseline_cards, load_cards_from_pack, CardDefinition};
pub use discovery::{discover_packs, PackLocation};
pub use packs::{AudioConfig, CardConfig, GeneratorConfig, PackManifest, PackType};

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

/// Pack scope determines where the pack is stored and who can access it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackScope {
    /// Shared pack installed by admin, available to all users
    Shared,
    /// User-specific pack, only available to the installing user
    User,
}

impl PackScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            PackScope::Shared => "shared",
            PackScope::User => "user",
        }
    }
}

impl std::fmt::Display for PackScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for PackScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "shared" => Ok(PackScope::Shared),
            "user" => Ok(PackScope::User),
            _ => Err(format!("Invalid pack scope: {}", s)),
        }
    }
}

impl ToSql for PackScope {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for PackScope {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value
            .as_str()?
            .parse()
            .map_err(|e: String| FromSqlError::Other(e.into()))
    }
}

/// Installed pack record (stored in content_packs table).
#[derive(Debug, Clone)]
pub struct InstalledPack {
    pub id: String,
    pub name: String,
    pub pack_type: PackType,
    pub version: Option<String>,
    pub description: Option<String>,
    pub source_path: String,
    pub scope: PackScope,
    pub installed_at: String,
    pub installed_by: Option<String>,
    pub metadata: Option<String>, // JSON blob for type-specific config
}

/// User's enabled pack record (stored in enabled_packs table).
#[derive(Debug, Clone)]
pub struct EnabledPack {
    pub pack_id: String,
    pub enabled_at: String,
    pub cards_created: bool,
    pub config: Option<String>, // JSON blob for user-specific settings
}
