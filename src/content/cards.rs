//! Card pack loading - reads card definitions from pack JSON files.

use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::domain::CardType;

/// Card definition from a pack's cards.json file.
#[derive(Debug, Clone, Deserialize)]
pub struct CardDefinition {
    pub front: String,
    pub main_answer: String,
    #[serde(default)]
    pub description: Option<String>,
    pub card_type: CardType,
    pub tier: u8,
    #[serde(default)]
    pub is_reverse: bool,
}

/// Container for cards in a pack's cards.json file.
#[derive(Debug, Deserialize)]
pub struct CardPackData {
    pub cards: Vec<CardDefinition>,
}

/// Load cards from a pack's cards.json file.
pub fn load_cards_from_pack(pack_dir: &Path, cards_file: &str) -> Result<Vec<CardDefinition>, CardLoadError> {
    let cards_path = pack_dir.join(cards_file);

    if !cards_path.exists() {
        return Err(CardLoadError::FileNotFound(cards_path.display().to_string()));
    }

    let content = fs::read_to_string(&cards_path)
        .map_err(|e| CardLoadError::IoError(cards_path.display().to_string(), e.to_string()))?;

    let data: CardPackData = serde_json::from_str(&content)
        .map_err(|e| CardLoadError::ParseError(cards_path.display().to_string(), e.to_string()))?;

    Ok(data.cards)
}

/// Load cards from the baseline pack.
///
/// Looks for the baseline pack at `data/content/packs/baseline/cards.json`.
/// Returns None if the pack doesn't exist (fallback to hardcoded data).
pub fn load_baseline_cards() -> Option<Vec<CardDefinition>> {
    let baseline_dir = Path::new(crate::paths::SHARED_PACKS_DIR).join("baseline");

    match load_cards_from_pack(&baseline_dir, "cards.json") {
        Ok(cards) => {
            tracing::debug!("Loaded {} cards from baseline pack", cards.len());
            Some(cards)
        }
        Err(e) => {
            tracing::debug!("Baseline pack not available, using hardcoded data: {}", e);
            None
        }
    }
}

/// Card loading errors.
#[derive(Debug)]
pub enum CardLoadError {
    FileNotFound(String),
    IoError(String, String),
    ParseError(String, String),
}

impl std::fmt::Display for CardLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardLoadError::FileNotFound(path) => write!(f, "Card file not found: {}", path),
            CardLoadError::IoError(path, err) => write!(f, "IO error reading {}: {}", path, err),
            CardLoadError::ParseError(path, err) => write!(f, "Parse error in {}: {}", path, err),
        }
    }
}

impl std::error::Error for CardLoadError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_cards_from_pack() {
        let temp = TempDir::new().unwrap();

        let cards_json = r#"{
            "cards": [
                {
                    "front": "ㄱ",
                    "main_answer": "g / k",
                    "description": "Test description",
                    "card_type": "Consonant",
                    "tier": 1,
                    "is_reverse": false
                },
                {
                    "front": "g / k",
                    "main_answer": "ㄱ",
                    "card_type": "Consonant",
                    "tier": 1,
                    "is_reverse": true
                }
            ]
        }"#;

        fs::write(temp.path().join("cards.json"), cards_json).unwrap();

        let cards = load_cards_from_pack(temp.path(), "cards.json").unwrap();
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].front, "ㄱ");
        assert_eq!(cards[0].card_type, CardType::Consonant);
        assert!(!cards[0].is_reverse);
        assert!(cards[1].is_reverse);
    }

    #[test]
    fn test_missing_file() {
        let temp = TempDir::new().unwrap();
        let result = load_cards_from_pack(temp.path(), "cards.json");
        assert!(matches!(result, Err(CardLoadError::FileNotFound(_))));
    }
}
