//! Reference content loading - reads grammar/lesson content from pack JSON files.
//!
//! Reference packs contain structured educational content organized by lessons and sections.
//! Unlike card packs (which are for SRS flashcards), reference packs are for reading/learning.

use serde::Deserialize;
use std::fs;
use std::path::Path;

use super::packs::ReferenceConfig;

/// Root structure for reference.json files.
#[derive(Debug, Clone, Deserialize)]
pub struct ReferencePackData {
    pub lessons: Vec<ReferenceLesson>,
}

/// A single lesson in the reference pack.
#[derive(Debug, Clone, Deserialize)]
pub struct ReferenceLesson {
    /// Unique ID for this lesson (e.g., "lesson-1")
    pub id: String,

    /// Lesson number (1, 2, 3, ...)
    pub number: u8,

    /// Lesson title (e.g., "Basic Sentence Structure")
    pub title: String,

    /// Brief description of the lesson
    #[serde(default)]
    pub description: Option<String>,

    /// Sections within the lesson
    pub sections: Vec<ReferenceSection>,

    /// Practice tips for this lesson
    #[serde(default)]
    pub practice_tips: Vec<String>,
}

/// A section within a lesson.
#[derive(Debug, Clone, Deserialize)]
pub struct ReferenceSection {
    /// Unique ID for this section (e.g., "topic-marker")
    pub id: String,

    /// Section title (e.g., "Topic Marker 은/는")
    pub title: String,

    /// Type of content in this section
    #[serde(rename = "type")]
    pub section_type: SectionType,

    /// Main content/explanation text
    #[serde(default)]
    pub content: Option<String>,

    /// Example sentences
    #[serde(default)]
    pub examples: Vec<ReferenceExample>,

    /// Grammar rules (for grammar_point sections)
    #[serde(default)]
    pub rules: Vec<GrammarRule>,

    /// Additional note or caveat
    #[serde(default)]
    pub note: Option<String>,

    /// Pattern card for future SRS integration
    #[serde(default)]
    pub pattern_card: Option<PatternCard>,
}

/// Type of content section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionType {
    /// General explanation text
    Explanation,
    /// Specific grammar point with rules
    GrammarPoint,
    /// Sentence pattern (future SRS candidate)
    Pattern,
    /// Related vocabulary
    Vocabulary,
    /// Cultural context note
    CulturalNote,
    /// Comparison of similar concepts (e.g., 나 vs 저, 은/는 vs 이/가)
    Comparison,
    /// Common learner mistakes to avoid
    CommonMistake,
    /// Set expressions/phrases to memorize as units
    SetExpression,
    /// Quick reference/cheat sheet section
    QuickReference,
}

impl SectionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SectionType::Explanation => "explanation",
            SectionType::GrammarPoint => "grammar_point",
            SectionType::Pattern => "pattern",
            SectionType::Vocabulary => "vocabulary",
            SectionType::CulturalNote => "cultural_note",
            SectionType::Comparison => "comparison",
            SectionType::CommonMistake => "common_mistake",
            SectionType::SetExpression => "set_expression",
            SectionType::QuickReference => "quick_reference",
        }
    }
}

/// Example sentence with optional breakdown.
#[derive(Debug, Clone, Deserialize)]
pub struct ReferenceExample {
    /// Korean sentence
    pub korean: String,

    /// Romanization (optional)
    #[serde(default)]
    pub romanization: Option<String>,

    /// English translation
    pub english: String,

    /// Word-by-word breakdown (optional)
    #[serde(default)]
    pub breakdown: Vec<WordBreakdown>,
}

/// Word-by-word breakdown for an example.
#[derive(Debug, Clone, Deserialize)]
pub struct WordBreakdown {
    /// The Korean text
    pub text: String,

    /// Grammatical role (e.g., "subject", "topic marker", "verb")
    pub role: String,

    /// English meaning (optional, for content words)
    #[serde(default)]
    pub meaning: Option<String>,
}

/// Grammar rule with condition and form.
#[derive(Debug, Clone, Deserialize)]
pub struct GrammarRule {
    /// When to use this form (e.g., "After consonant")
    pub condition: String,

    /// The form to use (e.g., "은")
    pub form: String,

    /// Example usage (optional)
    #[serde(default)]
    pub example: Option<String>,
}

/// Pattern card for future SRS integration.
#[derive(Debug, Clone, Deserialize)]
pub struct PatternCard {
    /// Card front (question/prompt)
    pub front: String,

    /// Expected answer
    pub answer: String,

    /// Tier for this card (default 5)
    #[serde(default = "default_tier")]
    pub tier: u8,
}

fn default_tier() -> u8 {
    5
}

/// Load reference content using the pack's configuration.
/// Supports both single-file and directory-based reference content.
pub fn load_reference(
    pack_dir: &Path,
    config: &ReferenceConfig,
) -> Result<ReferencePackData, ReferenceLoadError> {
    // Directory takes precedence over file
    if let Some(ref dir) = config.directory {
        load_reference_from_directory(pack_dir, dir)
    } else if let Some(ref file) = config.file {
        load_reference_from_file(pack_dir, file)
    } else {
        Err(ReferenceLoadError::FileNotFound(
            "No file or directory specified in reference config".to_string(),
        ))
    }
}

/// Load reference content from a single JSON file.
pub fn load_reference_from_file(
    pack_dir: &Path,
    ref_file: &str,
) -> Result<ReferencePackData, ReferenceLoadError> {
    let ref_path = pack_dir.join(ref_file);

    if !ref_path.exists() {
        return Err(ReferenceLoadError::FileNotFound(
            ref_path.display().to_string(),
        ));
    }

    let content = fs::read_to_string(&ref_path).map_err(|e| {
        ReferenceLoadError::IoError(ref_path.display().to_string(), e.to_string())
    })?;

    let data: ReferencePackData = serde_json::from_str(&content).map_err(|e| {
        ReferenceLoadError::ParseError(ref_path.display().to_string(), e.to_string())
    })?;

    Ok(data)
}

/// Load reference content from a directory of per-lesson JSON files.
/// Files should be named `lesson_01.json`, `lesson_02.json`, etc.
pub fn load_reference_from_directory(
    pack_dir: &Path,
    ref_dir: &str,
) -> Result<ReferencePackData, ReferenceLoadError> {
    let dir_path = pack_dir.join(ref_dir);

    if !dir_path.exists() || !dir_path.is_dir() {
        return Err(ReferenceLoadError::FileNotFound(
            dir_path.display().to_string(),
        ));
    }

    // Read all lesson_*.json files from the directory
    let entries = fs::read_dir(&dir_path).map_err(|e| {
        ReferenceLoadError::IoError(dir_path.display().to_string(), e.to_string())
    })?;

    let mut all_lessons = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| {
            ReferenceLoadError::IoError(dir_path.display().to_string(), e.to_string())
        })?;

        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Only process lesson_*.json files
        if !file_name.starts_with("lesson_") || !file_name.ends_with(".json") {
            continue;
        }

        let content = fs::read_to_string(&path).map_err(|e| {
            ReferenceLoadError::IoError(path.display().to_string(), e.to_string())
        })?;

        let data: ReferencePackData = serde_json::from_str(&content).map_err(|e| {
            ReferenceLoadError::ParseError(path.display().to_string(), e.to_string())
        })?;

        // Add all lessons from this file
        all_lessons.extend(data.lessons);
    }

    if all_lessons.is_empty() {
        return Err(ReferenceLoadError::FileNotFound(format!(
            "No lesson_*.json files found in {}",
            dir_path.display()
        )));
    }

    // Sort lessons by number
    all_lessons.sort_by_key(|l| l.number);

    Ok(ReferencePackData {
        lessons: all_lessons,
    })
}

/// Load reference content from a pack's reference.json file.
/// DEPRECATED: Use `load_reference` with ReferenceConfig instead.
pub fn load_reference_from_pack(
    pack_dir: &Path,
    ref_file: &str,
) -> Result<ReferencePackData, ReferenceLoadError> {
    load_reference_from_file(pack_dir, ref_file)
}

/// Find a specific lesson by number.
pub fn find_lesson(data: &ReferencePackData, lesson_num: u8) -> Option<&ReferenceLesson> {
    data.lessons.iter().find(|l| l.number == lesson_num)
}

/// Reference loading errors.
#[derive(Debug)]
pub enum ReferenceLoadError {
    FileNotFound(String),
    IoError(String, String),
    ParseError(String, String),
}

impl std::fmt::Display for ReferenceLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReferenceLoadError::FileNotFound(path) => {
                write!(f, "Reference file not found: {}", path)
            }
            ReferenceLoadError::IoError(path, err) => {
                write!(f, "IO error reading {}: {}", path, err)
            }
            ReferenceLoadError::ParseError(path, err) => {
                write!(f, "Parse error in {}: {}", path, err)
            }
        }
    }
}

impl ReferenceLoadError {
    /// Returns a user-facing error message without exposing filesystem paths.
    pub fn user_message(&self) -> &'static str {
        match self {
            ReferenceLoadError::FileNotFound(_) => "Reference content not found",
            ReferenceLoadError::IoError(_, _) => "Failed to read reference content",
            ReferenceLoadError::ParseError(_, _) => "Failed to parse reference content",
        }
    }
}

impl std::error::Error for ReferenceLoadError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_reference() {
        let json = r#"{
            "lessons": [
                {
                    "id": "lesson-1",
                    "number": 1,
                    "title": "Basic Structure",
                    "sections": []
                }
            ]
        }"#;

        let data: ReferencePackData = serde_json::from_str(json).unwrap();
        assert_eq!(data.lessons.len(), 1);
        assert_eq!(data.lessons[0].number, 1);
        assert_eq!(data.lessons[0].title, "Basic Structure");
    }

    #[test]
    fn test_parse_full_reference() {
        let json = r#"{
            "lessons": [
                {
                    "id": "lesson-1",
                    "number": 1,
                    "title": "Basic Sentence Structure",
                    "description": "SOV word order and particles",
                    "sections": [
                        {
                            "id": "sov",
                            "title": "Word Order",
                            "type": "explanation",
                            "content": "Korean uses SOV order."
                        },
                        {
                            "id": "topic-marker",
                            "title": "Topic Marker",
                            "type": "grammar_point",
                            "content": "Marks the topic",
                            "rules": [
                                {"condition": "After consonant", "form": "은", "example": "사람은"},
                                {"condition": "After vowel", "form": "는", "example": "나는"}
                            ],
                            "note": "Different from subject marker"
                        },
                        {
                            "id": "pattern-1",
                            "title": "A is B",
                            "type": "pattern",
                            "pattern_card": {
                                "front": "A is B",
                                "answer": "[noun]은/는 [noun]이에요",
                                "tier": 5
                            }
                        }
                    ],
                    "practice_tips": ["Focus on SOV"]
                }
            ]
        }"#;

        let data: ReferencePackData = serde_json::from_str(json).unwrap();
        assert_eq!(data.lessons.len(), 1);

        let lesson = &data.lessons[0];
        assert_eq!(lesson.sections.len(), 3);
        assert_eq!(lesson.practice_tips.len(), 1);

        // Check grammar point
        let grammar = &lesson.sections[1];
        assert_eq!(grammar.section_type, SectionType::GrammarPoint);
        assert_eq!(grammar.rules.len(), 2);
        assert!(grammar.note.is_some());

        // Check pattern
        let pattern = &lesson.sections[2];
        assert_eq!(pattern.section_type, SectionType::Pattern);
        assert!(pattern.pattern_card.is_some());
        let card = pattern.pattern_card.as_ref().unwrap();
        assert_eq!(card.tier, 5);
    }

    #[test]
    fn test_parse_example_with_breakdown() {
        let json = r#"{
            "lessons": [
                {
                    "id": "lesson-1",
                    "number": 1,
                    "title": "Test",
                    "sections": [
                        {
                            "id": "ex",
                            "title": "Example",
                            "type": "explanation",
                            "examples": [
                                {
                                    "korean": "나는 밥을 먹어요",
                                    "romanization": "naneun babeul meogeoyo",
                                    "english": "I eat rice",
                                    "breakdown": [
                                        {"text": "나", "role": "subject", "meaning": "I"},
                                        {"text": "는", "role": "topic marker"},
                                        {"text": "밥", "role": "object", "meaning": "rice"},
                                        {"text": "을", "role": "object marker"},
                                        {"text": "먹어요", "role": "verb", "meaning": "eat"}
                                    ]
                                }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let data: ReferencePackData = serde_json::from_str(json).unwrap();
        let example = &data.lessons[0].sections[0].examples[0];
        assert_eq!(example.korean, "나는 밥을 먹어요");
        assert_eq!(example.breakdown.len(), 5);
        assert_eq!(example.breakdown[0].meaning, Some("I".to_string()));
        assert_eq!(example.breakdown[1].meaning, None); // marker has no meaning
    }

    #[test]
    fn test_find_lesson() {
        let data = ReferencePackData {
            lessons: vec![
                ReferenceLesson {
                    id: "lesson-1".to_string(),
                    number: 1,
                    title: "First".to_string(),
                    description: None,
                    sections: vec![],
                    practice_tips: vec![],
                },
                ReferenceLesson {
                    id: "lesson-2".to_string(),
                    number: 2,
                    title: "Second".to_string(),
                    description: None,
                    sections: vec![],
                    practice_tips: vec![],
                },
            ],
        };

        assert!(find_lesson(&data, 1).is_some());
        assert_eq!(find_lesson(&data, 1).unwrap().title, "First");
        assert!(find_lesson(&data, 2).is_some());
        assert!(find_lesson(&data, 3).is_none());
    }

    #[test]
    fn test_section_type_parsing() {
        let types = [
            ("explanation", SectionType::Explanation),
            ("grammar_point", SectionType::GrammarPoint),
            ("pattern", SectionType::Pattern),
            ("vocabulary", SectionType::Vocabulary),
            ("cultural_note", SectionType::CulturalNote),
            ("comparison", SectionType::Comparison),
            ("common_mistake", SectionType::CommonMistake),
            ("set_expression", SectionType::SetExpression),
            ("quick_reference", SectionType::QuickReference),
        ];

        for (s, expected) in types {
            let json = format!(r#"{{"type": "{}"}}"#, s);
            #[derive(Deserialize)]
            struct Test {
                #[serde(rename = "type")]
                t: SectionType,
            }
            let parsed: Test = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.t, expected);
        }
    }
}
