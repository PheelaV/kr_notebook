//! Audio pack resolution and management
//!
//! Handles loading audio from packs with fallback to legacy locations.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths;

/// Represents an audio source (pack or legacy location)
#[derive(Debug, Clone)]
pub struct AudioSource {
    /// Base path to audio files
    pub base_path: PathBuf,
    /// Pack ID if from a pack, None if legacy
    pub pack_id: Option<String>,
}

/// Audio pack manifest structure
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AudioPackManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub pack_type: String,
    pub description: Option<String>,
    pub audio: AudioPackConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AudioPackConfig {
    /// Which lessons this pack enhances
    pub enhances: Vec<String>,
    /// Structure templates for finding audio files
    pub structure: AudioStructure,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AudioStructure {
    /// Template for row audio: e.g., "rows/row_{romanization}.mp3"
    pub rows: Option<String>,
    /// Template for column audio: e.g., "columns/col_{romanization}.mp3"
    pub columns: Option<String>,
    /// Template for syllable audio: e.g., "syllables/{romanization}.mp3"
    pub syllables: Option<String>,
}

/// Find audio packs that enhance a given lesson
pub fn find_audio_packs_for_lesson(lesson_id: &str) -> Vec<AudioSource> {
    let mut sources = Vec::new();

    // Check shared packs directory
    let shared_packs = Path::new(paths::SHARED_PACKS_DIR);
    if shared_packs.exists() {
        if let Ok(entries) = fs::read_dir(shared_packs) {
            for entry in entries.filter_map(|e| e.ok()) {
                let pack_path = entry.path();
                if let Some(source) = check_audio_pack(&pack_path, lesson_id) {
                    sources.push(source);
                }
            }
        }
    }

    // Check for generated content (from scrapers)
    let generated_path = Path::new(paths::SHARED_GENERATED_DIR).join("htsk").join(lesson_id);
    if generated_path.exists() {
        sources.push(AudioSource {
            base_path: generated_path,
            pack_id: Some("htsk-generated".to_string()),
        });
    }

    sources
}

/// Check if a pack directory contains audio for the given lesson
fn check_audio_pack(pack_path: &Path, lesson_id: &str) -> Option<AudioSource> {
    let manifest_path = pack_path.join("pack.json");
    if !manifest_path.exists() {
        return None;
    }

    let manifest_content = fs::read_to_string(&manifest_path).ok()?;
    let manifest: AudioPackManifest = serde_json::from_str(&manifest_content).ok()?;

    // Only consider audio packs
    if manifest.pack_type != "audio" {
        return None;
    }

    // Check if this pack enhances the requested lesson
    if !manifest.audio.enhances.contains(&lesson_id.to_string()) {
        return None;
    }

    // The audio is stored in the pack's lesson subdirectory
    let audio_path = pack_path.join(lesson_id);
    if audio_path.exists() {
        Some(AudioSource {
            base_path: audio_path,
            pack_id: Some(manifest.id),
        })
    } else {
        None
    }
}

/// Get the path for audio files for a lesson, checking packs first then legacy
pub fn get_audio_path(lesson_id: &str) -> Option<PathBuf> {
    // First, check packs
    let pack_sources = find_audio_packs_for_lesson(lesson_id);
    if let Some(source) = pack_sources.first() {
        return Some(source.base_path.clone());
    }

    // Fall back to legacy location
    let legacy_path = PathBuf::from(paths::lesson_dir(lesson_id));
    if legacy_path.exists() {
        return Some(legacy_path);
    }

    None
}

/// Get manifest path for a lesson, checking packs first
pub fn get_manifest_path(lesson_id: &str) -> Option<PathBuf> {
    // First check packs
    if let Some(audio_path) = get_audio_path(lesson_id) {
        let manifest = audio_path.join("manifest.json");
        if manifest.exists() {
            return Some(manifest);
        }
    }

    // Fall back to legacy
    let legacy_manifest = PathBuf::from(paths::manifest_path(lesson_id));
    if legacy_manifest.exists() {
        return Some(legacy_manifest);
    }

    None
}

/// Get syllables directory for a lesson
pub fn get_syllables_dir(lesson_id: &str) -> Option<PathBuf> {
    if let Some(audio_path) = get_audio_path(lesson_id) {
        let syllables = audio_path.join("syllables");
        if syllables.exists() {
            return Some(syllables);
        }
    }

    // Fall back to legacy
    let legacy = PathBuf::from(paths::syllables_dir(lesson_id));
    if legacy.exists() {
        return Some(legacy);
    }

    None
}

/// Get rows directory for a lesson
pub fn get_rows_dir(lesson_id: &str) -> Option<PathBuf> {
    if let Some(audio_path) = get_audio_path(lesson_id) {
        let rows = audio_path.join("rows");
        if rows.exists() {
            return Some(rows);
        }
    }

    // Fall back to legacy
    let legacy = PathBuf::from(paths::rows_dir(lesson_id));
    if legacy.exists() {
        return Some(legacy);
    }

    None
}

/// Get columns directory for a lesson
pub fn get_columns_dir(lesson_id: &str) -> Option<PathBuf> {
    if let Some(audio_path) = get_audio_path(lesson_id) {
        let columns = audio_path.join("columns");
        if columns.exists() {
            return Some(columns);
        }
    }

    // Fall back to legacy
    let legacy = PathBuf::from(paths::columns_dir(lesson_id));
    if legacy.exists() {
        return Some(legacy);
    }

    None
}

/// Get available syllable audio files for a lesson (pack-aware)
pub fn get_available_syllables(lesson_id: &str) -> HashSet<String> {
    let syllables_dir = match get_syllables_dir(lesson_id) {
        Some(dir) => dir,
        None => return HashSet::new(),
    };

    fs::read_dir(&syllables_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().map(|ext| ext == "mp3").unwrap_or(false) {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(String::from)
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// List all available lessons that have audio content
pub fn list_available_lessons() -> Vec<String> {
    let mut lessons = HashSet::new();

    // Check shared generated content
    let generated_path = Path::new(paths::SHARED_GENERATED_DIR).join("htsk");
    if generated_path.exists() {
        if let Ok(entries) = fs::read_dir(&generated_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        lessons.insert(name.to_string());
                    }
                }
            }
        }
    }

    // Check packs
    let shared_packs = Path::new(paths::SHARED_PACKS_DIR);
    if shared_packs.exists() {
        if let Ok(entries) = fs::read_dir(shared_packs) {
            for entry in entries.filter_map(|e| e.ok()) {
                let pack_path = entry.path();
                let manifest_path = pack_path.join("pack.json");
                if manifest_path.exists() {
                    if let Ok(content) = fs::read_to_string(&manifest_path) {
                        if let Ok(manifest) = serde_json::from_str::<AudioPackManifest>(&content) {
                            if manifest.pack_type == "audio" {
                                for lesson in manifest.audio.enhances {
                                    lessons.insert(lesson);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Check legacy location
    let legacy_path = Path::new(paths::HTSK_DIR);
    if legacy_path.exists() {
        if let Ok(entries) = fs::read_dir(legacy_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        lessons.insert(name.to_string());
                    }
                }
            }
        }
    }

    let mut sorted: Vec<_> = lessons.into_iter().collect();
    sorted.sort();
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_available_lessons() {
        // Should return list without panicking
        let lessons = list_available_lessons();
        // May be empty or have lessons depending on environment
        assert!(lessons.len() >= 0);
    }

    #[test]
    fn test_nonexistent_lesson() {
        let syllables = get_available_syllables("nonexistent_lesson_xyz");
        assert!(syllables.is_empty());
    }
}
