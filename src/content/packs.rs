//! Pack manifest parsing and validation.
//!
//! Each pack is a directory containing a `pack.json` manifest file.

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Pack type determines what kind of content the pack provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackType {
    /// Audio files for pronunciation (syllables, rows, columns)
    Audio,
    /// Generator/scraper that creates content
    Generator,
    /// Card definitions (vocabulary, grammar, etc.)
    Cards,
}

impl PackType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PackType::Audio => "audio",
            PackType::Generator => "generator",
            PackType::Cards => "cards",
        }
    }
}

impl std::fmt::Display for PackType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for PackType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "audio" => Ok(PackType::Audio),
            "generator" => Ok(PackType::Generator),
            "cards" => Ok(PackType::Cards),
            _ => Err(format!("Invalid pack type: {}", s)),
        }
    }
}

impl ToSql for PackType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for PackType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value
            .as_str()?
            .parse()
            .map_err(|e: String| FromSqlError::Other(e.into()))
    }
}

/// Audio pack configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Which lessons/content sets this audio enhances
    #[serde(default)]
    pub enhances: Vec<String>,

    /// File path patterns for audio files
    #[serde(default)]
    pub structure: AudioStructure,
}

/// Audio file path patterns.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioStructure {
    /// Pattern for row audio: e.g., "rows/row_{romanization}.mp3"
    pub rows: Option<String>,
    /// Pattern for column audio: e.g., "columns/col_{romanization}.mp3"
    pub columns: Option<String>,
    /// Pattern for syllable audio: e.g., "syllables/{romanization}.mp3"
    pub syllables: Option<String>,
}

/// Generator pack configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorConfig {
    /// Command to run the generator
    pub command: String,

    /// Available subcommands/targets
    #[serde(default)]
    pub subcommands: Vec<GeneratorSubcommand>,

    /// Type of content generated (becomes this pack type when run)
    #[serde(default)]
    pub output_type: Option<String>,
}

/// A single generator subcommand/target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorSubcommand {
    /// Unique ID for this subcommand
    pub id: String,
    /// Command-line arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Output directory (relative to generated content dir)
    pub output: String,
}

/// Card pack configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardConfig {
    /// Path to cards JSON file (relative to pack directory)
    pub file: String,

    /// Default tier for cards in this pack
    #[serde(default = "default_tier")]
    pub tier: u8,

    /// Card types included in this pack
    #[serde(default)]
    pub card_types: Vec<String>,

    /// Whether to create reverse cards automatically
    #[serde(default)]
    pub create_reverse: bool,
}

fn default_tier() -> u8 {
    5
}

/// Pack manifest (pack.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    /// Unique pack identifier (e.g., "htsk-audio")
    pub id: String,

    /// Human-readable pack name
    pub name: String,

    /// Pack version (semver)
    #[serde(default)]
    pub version: Option<String>,

    /// Pack type
    #[serde(rename = "type")]
    pub pack_type: PackType,

    /// Pack description
    #[serde(default)]
    pub description: Option<String>,

    /// Content types this pack provides (e.g., ["vocabulary"])
    /// Used to determine which UI features/pages to show
    #[serde(default)]
    pub provides: Vec<String>,

    /// Audio pack configuration (if type == audio)
    #[serde(default)]
    pub audio: Option<AudioConfig>,

    /// Generator pack configuration (if type == generator)
    #[serde(default)]
    pub generator: Option<GeneratorConfig>,

    /// Card pack configuration (if type == cards)
    #[serde(default)]
    pub cards: Option<CardConfig>,
}

impl PackManifest {
    /// Load a pack manifest from a directory.
    pub fn load(pack_dir: &Path) -> Result<Self, PackError> {
        let manifest_path = pack_dir.join("pack.json");
        if !manifest_path.exists() {
            return Err(PackError::ManifestNotFound(pack_dir.display().to_string()));
        }

        let content = fs::read_to_string(&manifest_path)
            .map_err(|e| PackError::IoError(manifest_path.display().to_string(), e.to_string()))?;

        let manifest: PackManifest = serde_json::from_str(&content)
            .map_err(|e| PackError::ParseError(manifest_path.display().to_string(), e.to_string()))?;

        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate the manifest for internal consistency.
    pub fn validate(&self) -> Result<(), PackError> {
        // Check that type-specific config is present
        match self.pack_type {
            PackType::Audio => {
                if self.audio.is_none() {
                    return Err(PackError::ValidationError(
                        self.id.clone(),
                        "Audio pack missing 'audio' configuration".to_string(),
                    ));
                }
            }
            PackType::Generator => {
                if self.generator.is_none() {
                    return Err(PackError::ValidationError(
                        self.id.clone(),
                        "Generator pack missing 'generator' configuration".to_string(),
                    ));
                }
            }
            PackType::Cards => {
                if self.cards.is_none() {
                    return Err(PackError::ValidationError(
                        self.id.clone(),
                        "Card pack missing 'cards' configuration".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Serialize type-specific config to JSON for storage.
    pub fn type_config_json(&self) -> Option<String> {
        match self.pack_type {
            PackType::Audio => self
                .audio
                .as_ref()
                .and_then(|c| serde_json::to_string(c).ok()),
            PackType::Generator => self
                .generator
                .as_ref()
                .and_then(|c| serde_json::to_string(c).ok()),
            PackType::Cards => self
                .cards
                .as_ref()
                .and_then(|c| serde_json::to_string(c).ok()),
        }
    }
}

/// Pack-related errors.
#[derive(Debug)]
pub enum PackError {
    ManifestNotFound(String),
    IoError(String, String),
    ParseError(String, String),
    ValidationError(String, String),
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackError::ManifestNotFound(path) => {
                write!(f, "Pack manifest not found: {}/pack.json", path)
            }
            PackError::IoError(path, err) => write!(f, "IO error reading {}: {}", path, err),
            PackError::ParseError(path, err) => write!(f, "Parse error in {}: {}", path, err),
            PackError::ValidationError(id, err) => {
                write!(f, "Validation error for pack '{}': {}", id, err)
            }
        }
    }
}

impl std::error::Error for PackError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_type_roundtrip() {
        for pt in [PackType::Audio, PackType::Generator, PackType::Cards] {
            let s = pt.as_str();
            let parsed: PackType = s.parse().unwrap();
            assert_eq!(pt, parsed);
        }
    }

    #[test]
    fn test_audio_manifest_parse() {
        let json = r#"{
            "id": "htsk-audio",
            "name": "HTSK Audio",
            "type": "audio",
            "version": "1.0.0",
            "audio": {
                "enhances": ["lesson1", "lesson2"],
                "structure": {
                    "syllables": "syllables/{romanization}.mp3"
                }
            }
        }"#;

        let manifest: PackManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.id, "htsk-audio");
        assert_eq!(manifest.pack_type, PackType::Audio);
        assert!(manifest.audio.is_some());
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_generator_manifest_parse() {
        let json = r#"{
            "id": "htsk-scraper",
            "name": "HTSK Scraper",
            "type": "generator",
            "generator": {
                "command": "uv run kr-scraper",
                "subcommands": [
                    {"id": "lesson1", "args": ["lesson1"], "output": "lesson1/"}
                ],
                "output_type": "audio"
            }
        }"#;

        let manifest: PackManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.pack_type, PackType::Generator);
        assert!(manifest.generator.is_some());
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_card_manifest_parse() {
        let json = r#"{
            "id": "vocab-basic",
            "name": "Basic Vocabulary",
            "type": "cards",
            "cards": {
                "file": "cards.json",
                "tier": 5,
                "create_reverse": true
            }
        }"#;

        let manifest: PackManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.pack_type, PackType::Cards);
        assert!(manifest.cards.is_some());
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_missing_type_config() {
        let json = r#"{
            "id": "broken",
            "name": "Broken Pack",
            "type": "audio"
        }"#;

        let manifest: PackManifest = serde_json::from_str(json).unwrap();
        assert!(manifest.validate().is_err());
    }
}
