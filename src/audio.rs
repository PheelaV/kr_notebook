//! Audio utilities for syllable and manifest handling
//!
//! Shared functions used by listen and pronunciation handlers.
//! Uses the pack system for audio resolution with fallback to legacy locations.

use std::collections::HashSet;
use std::fs;

use crate::content::audio as pack_audio;

// Re-export list_available_lessons for external use
pub use pack_audio::list_available_lessons;

/// Parsed manifest data shared between listen and pronunciation handlers
#[derive(Debug, Clone)]
pub struct ManifestData {
    pub vowels_order: Vec<String>,
    pub consonants_order: Vec<String>,
    pub rows: serde_json::Value,
    pub columns: serde_json::Value,
    pub syllable_table: serde_json::Value,
}

/// Syllable info extracted from manifest
#[derive(Debug, Clone)]
pub struct SyllableInfo {
    pub character: String,
    pub romanization: String,
}

/// Load and parse a manifest file for a lesson
/// Uses pack system with fallback to legacy location
pub fn load_manifest(lesson_id: &str) -> Option<ManifestData> {
    let manifest_path = pack_audio::get_manifest_path(lesson_id)?;
    let manifest_content = fs::read_to_string(&manifest_path).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_content).ok()?;

    let vowels_order = manifest["vowels_order"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let consonants_order = manifest["consonants_order"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Some(ManifestData {
        vowels_order,
        consonants_order,
        rows: manifest["rows"].clone(),
        columns: manifest["columns"].clone(),
        syllable_table: manifest["syllable_table"].clone(),
    })
}

/// Get the fallback romanization for a vowel character
pub fn vowel_romanization(vowel: &str) -> &'static str {
    match vowel {
        // Lesson 1 basic vowels
        "ㅣ" => "i",
        "ㅏ" => "a",
        "ㅓ" => "eo",
        "ㅡ" => "eu",
        "ㅜ" => "u",
        "ㅗ" => "o",
        // Lesson 3 combined vowels
        "ㅐ" => "ae",
        "ㅔ" => "e",
        "ㅒ" => "yae",
        "ㅖ" => "ye",
        // Lesson 3 diphthongs
        "ㅘ" => "wa",
        "ㅙ" => "wae",
        "ㅚ" => "oe",
        "ㅝ" => "wo",
        "ㅞ" => "we",
        "ㅟ" => "wi",
        "ㅢ" => "ui",
        _ => "",
    }
}

/// Get syllables from a consonant row in the manifest
pub fn get_row_syllables(
    manifest: &ManifestData,
    consonant: &str,
) -> Vec<SyllableInfo> {
    let row = match manifest.rows.get(consonant) {
        Some(r) => r,
        None => return Vec::new(),
    };

    row["syllables"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let character = s.as_str()?.to_string();
                    let romanization = manifest
                        .syllable_table
                        .get(&character)
                        .and_then(|st| st["romanization"].as_str())
                        .unwrap_or("")
                        .to_string();
                    Some(SyllableInfo {
                        character,
                        romanization,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Get romanization for a consonant row
pub fn get_row_romanization(manifest: &ManifestData, consonant: &str) -> String {
    manifest
        .rows
        .get(consonant)
        .and_then(|row| row["romanization"].as_str())
        .unwrap_or("")
        .to_string()
}

/// Check if a consonant row has audio
pub fn row_has_audio(manifest: &ManifestData, consonant: &str) -> bool {
    manifest
        .rows
        .get(consonant)
        .and_then(|row| row["file"].as_str())
        .map(|f| !f.is_empty())
        .unwrap_or(false)
}

/// Get available syllable audio files for a lesson
///
/// Returns a set of syllable romanizations that have corresponding .mp3 files.
/// Uses pack system with fallback to legacy location.
pub fn get_available_syllables(lesson: &str) -> HashSet<String> {
    pack_audio::get_available_syllables(lesson)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonexistent_lesson_returns_empty() {
        let result = get_available_syllables("nonexistent_lesson_xyz");
        assert!(result.is_empty());
    }

    #[test]
    fn test_returns_hashset() {
        // Even for non-existent path, should return valid HashSet
        let result = get_available_syllables("test");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_vowel_romanization() {
        // Lesson 1 basic vowels
        assert_eq!(vowel_romanization("ㅣ"), "i");
        assert_eq!(vowel_romanization("ㅏ"), "a");
        assert_eq!(vowel_romanization("ㅓ"), "eo");
        assert_eq!(vowel_romanization("ㅡ"), "eu");
        assert_eq!(vowel_romanization("ㅜ"), "u");
        assert_eq!(vowel_romanization("ㅗ"), "o");
        // Lesson 3 combined vowels
        assert_eq!(vowel_romanization("ㅐ"), "ae");
        assert_eq!(vowel_romanization("ㅔ"), "e");
        assert_eq!(vowel_romanization("ㅒ"), "yae");
        assert_eq!(vowel_romanization("ㅖ"), "ye");
        // Lesson 3 diphthongs
        assert_eq!(vowel_romanization("ㅘ"), "wa");
        assert_eq!(vowel_romanization("ㅙ"), "wae");
        assert_eq!(vowel_romanization("ㅚ"), "oe");
        assert_eq!(vowel_romanization("ㅝ"), "wo");
        assert_eq!(vowel_romanization("ㅞ"), "we");
        assert_eq!(vowel_romanization("ㅟ"), "wi");
        assert_eq!(vowel_romanization("ㅢ"), "ui");
        // Unknown vowel
        assert_eq!(vowel_romanization("ㅑ"), "");
    }

    #[test]
    fn test_load_manifest_nonexistent() {
        let result = load_manifest("nonexistent_lesson_xyz");
        assert!(result.is_none());
    }
}
