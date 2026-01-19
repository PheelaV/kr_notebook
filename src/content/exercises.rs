//! Exercise loading and types for grammar practice.
//!
//! Exercises are pack-defined content for interactive grammar practice.
//! The platform provides generic exercise infrastructure; packs define the
//! actual exercises, answers, and distractors.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Type of exercise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExerciseType {
    /// Fill-in-the-blank cloze exercise (particle practice)
    Cloze,
}

impl ExerciseType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cloze => "cloze",
        }
    }
}

/// A blank position in a cloze exercise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClozeBlank {
    /// Position in the sentence (1-indexed, matches ___1___, ___2___, etc.)
    pub position: u8,
    /// The correct answer for this blank
    pub answer: String,
    /// Distractor options (incorrect but plausible answers)
    #[serde(default)]
    pub distractors: Vec<String>,
    /// Optional hint for this blank
    #[serde(default)]
    pub hint: Option<String>,
}

/// A single exercise definition (from pack JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exercise {
    /// Unique exercise ID within the pack (e.g., "L1-C001")
    pub id: String,
    /// Exercise type
    #[serde(rename = "type")]
    pub exercise_type: ExerciseType,
    /// The sentence with blanks marked as ___ (for cloze)
    pub sentence: String,
    /// Blanks to fill in (for cloze exercises)
    #[serde(default)]
    pub blanks: Vec<ClozeBlank>,
    /// English translation (for display after answer)
    #[serde(default)]
    pub english: Option<String>,
    /// Grammar point being tested (e.g., "topic_object_markers")
    #[serde(default)]
    pub grammar_point: Option<String>,
    /// Optional lesson number (if not in filename)
    #[serde(default)]
    pub lesson: Option<u8>,
}

/// A lesson's worth of exercises.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseLesson {
    /// Lesson number
    pub lesson: u8,
    /// Title of the lesson
    #[serde(default)]
    pub title: Option<String>,
    /// Exercises in this lesson
    pub exercises: Vec<Exercise>,
}

/// All exercises loaded from a pack.
#[derive(Debug, Clone, Default)]
pub struct ExercisePackData {
    /// Pack ID
    pub pack_id: String,
    /// Lessons with exercises
    pub lessons: Vec<ExerciseLesson>,
}

/// Error loading exercises.
#[derive(Debug)]
pub enum ExerciseLoadError {
    IoError(String),
    ParseError(String),
    InvalidExercise(String),
}

impl std::fmt::Display for ExerciseLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExerciseLoadError::IoError(e) => write!(f, "IO error: {}", e),
            ExerciseLoadError::ParseError(e) => write!(f, "Parse error: {}", e),
            ExerciseLoadError::InvalidExercise(e) => write!(f, "Invalid exercise: {}", e),
        }
    }
}

impl std::error::Error for ExerciseLoadError {}

/// Load exercises from a pack's exercise directory.
///
/// Expects files named `lesson_01.json`, `lesson_02.json`, etc.
/// Each file should contain an array of Exercise objects.
pub fn load_exercises_from_pack(
    pack_path: &Path,
    exercise_dir: &str,
) -> Result<ExercisePackData, ExerciseLoadError> {
    let exercises_path = pack_path.join(exercise_dir);

    if !exercises_path.exists() || !exercises_path.is_dir() {
        return Ok(ExercisePackData::default());
    }

    let mut lessons = Vec::new();

    // Read directory and find lesson files
    let entries = fs::read_dir(&exercises_path)
        .map_err(|e| ExerciseLoadError::IoError(e.to_string()))?;

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip non-JSON files
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        // Extract lesson number from filename (lesson_01.json -> 1)
        let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let lesson_num = parse_lesson_number(filename);

        if let Some(num) = lesson_num {
            match load_exercises_from_file(&path) {
                Ok(mut exercises) => {
                    // Set lesson number on exercises if not set
                    for ex in &mut exercises {
                        if ex.lesson.is_none() {
                            ex.lesson = Some(num);
                        }
                    }

                    lessons.push(ExerciseLesson {
                        lesson: num,
                        title: None,
                        exercises,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to load exercises from {}: {}", path.display(), e);
                }
            }
        }
    }

    // Sort lessons by number
    lessons.sort_by_key(|l| l.lesson);

    Ok(ExercisePackData {
        pack_id: String::new(),
        lessons,
    })
}

/// Parse lesson number from filename (e.g., "lesson_01" -> Some(1))
fn parse_lesson_number(filename: &str) -> Option<u8> {
    // Handle "lesson_XX" format
    if let Some(num_str) = filename.strip_prefix("lesson_") {
        return num_str.parse().ok();
    }
    // Handle "lessonXX" format
    if let Some(num_str) = filename.strip_prefix("lesson") {
        return num_str.parse().ok();
    }
    None
}

/// Load exercises from a single JSON file.
fn load_exercises_from_file(path: &Path) -> Result<Vec<Exercise>, ExerciseLoadError> {
    let content = fs::read_to_string(path)
        .map_err(|e| ExerciseLoadError::IoError(e.to_string()))?;

    let exercises: Vec<Exercise> = serde_json::from_str(&content)
        .map_err(|e| ExerciseLoadError::ParseError(format!("{}: {}", path.display(), e)))?;

    // Validate exercises
    for ex in &exercises {
        validate_exercise(ex)?;
    }

    Ok(exercises)
}

/// Validate an exercise definition.
fn validate_exercise(ex: &Exercise) -> Result<(), ExerciseLoadError> {
    if ex.id.is_empty() {
        return Err(ExerciseLoadError::InvalidExercise("Exercise missing ID".to_string()));
    }

    match ex.exercise_type {
        ExerciseType::Cloze => {
            if ex.sentence.is_empty() {
                return Err(ExerciseLoadError::InvalidExercise(
                    format!("Cloze exercise {} missing sentence", ex.id),
                ));
            }
            if ex.blanks.is_empty() {
                return Err(ExerciseLoadError::InvalidExercise(
                    format!("Cloze exercise {} has no blanks", ex.id),
                ));
            }
            for blank in &ex.blanks {
                if blank.answer.is_empty() {
                    return Err(ExerciseLoadError::InvalidExercise(
                        format!("Cloze exercise {} has blank with empty answer", ex.id),
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Render a cloze sentence with blanks replaced by input fields or answers.
/// Returns the sentence with blanks replaced by numbered markers.
pub fn render_cloze_display(sentence: &str, blanks: &[ClozeBlank]) -> String {
    let mut result = sentence.to_string();

    // Sort blanks by position descending to avoid index shifting
    let mut sorted_blanks: Vec<&ClozeBlank> = blanks.iter().collect();
    sorted_blanks.sort_by_key(|b| std::cmp::Reverse(b.position));

    // Replace each blank marker with a numbered span
    for blank in sorted_blanks {
        let marker = format!("___{}", blank.position);
        let replacement = format!("[{}]", blank.position);
        result = result.replace(&marker, &replacement);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lesson_number() {
        assert_eq!(parse_lesson_number("lesson_01"), Some(1));
        assert_eq!(parse_lesson_number("lesson_08"), Some(8));
        assert_eq!(parse_lesson_number("lesson_12"), Some(12));
        assert_eq!(parse_lesson_number("lesson1"), Some(1));
        assert_eq!(parse_lesson_number("lesson8"), Some(8));
        assert_eq!(parse_lesson_number("other"), None);
        assert_eq!(parse_lesson_number(""), None);
    }

    #[test]
    fn test_exercise_type_as_str() {
        assert_eq!(ExerciseType::Cloze.as_str(), "cloze");
    }

    #[test]
    fn test_parse_exercise_json() {
        let json = r#"[
            {
                "id": "L1-C001",
                "type": "cloze",
                "sentence": "저___1밥___2먹어요",
                "blanks": [
                    {"position": 1, "answer": "는", "distractors": ["가", "을", "이"]},
                    {"position": 2, "answer": "을", "distractors": ["는", "이", "가"]}
                ],
                "english": "I eat rice",
                "grammar_point": "topic_object_markers"
            }
        ]"#;

        let exercises: Vec<Exercise> = serde_json::from_str(json).unwrap();
        assert_eq!(exercises.len(), 1);

        let ex = &exercises[0];
        assert_eq!(ex.id, "L1-C001");
        assert_eq!(ex.exercise_type, ExerciseType::Cloze);
        assert_eq!(ex.blanks.len(), 2);
        assert_eq!(ex.blanks[0].answer, "는");
        assert_eq!(ex.blanks[1].answer, "을");
        assert_eq!(ex.english, Some("I eat rice".to_string()));
    }

    #[test]
    fn test_validate_exercise_valid() {
        let ex = Exercise {
            id: "L1-C001".to_string(),
            exercise_type: ExerciseType::Cloze,
            sentence: "저___1밥먹어요".to_string(),
            blanks: vec![ClozeBlank {
                position: 1,
                answer: "는".to_string(),
                distractors: vec![],
                hint: None,
            }],
            english: None,
            grammar_point: None,
            lesson: None,
        };

        assert!(validate_exercise(&ex).is_ok());
    }

    #[test]
    fn test_validate_exercise_missing_id() {
        let ex = Exercise {
            id: "".to_string(),
            exercise_type: ExerciseType::Cloze,
            sentence: "test".to_string(),
            blanks: vec![ClozeBlank {
                position: 1,
                answer: "는".to_string(),
                distractors: vec![],
                hint: None,
            }],
            english: None,
            grammar_point: None,
            lesson: None,
        };

        assert!(validate_exercise(&ex).is_err());
    }

    #[test]
    fn test_validate_exercise_no_blanks() {
        let ex = Exercise {
            id: "L1-C001".to_string(),
            exercise_type: ExerciseType::Cloze,
            sentence: "test".to_string(),
            blanks: vec![],
            english: None,
            grammar_point: None,
            lesson: None,
        };

        assert!(validate_exercise(&ex).is_err());
    }

    #[test]
    fn test_render_cloze_display() {
        let sentence = "저___1밥___2먹어요";
        let blanks = vec![
            ClozeBlank {
                position: 1,
                answer: "는".to_string(),
                distractors: vec![],
                hint: None,
            },
            ClozeBlank {
                position: 2,
                answer: "을".to_string(),
                distractors: vec![],
                hint: None,
            },
        ];

        let result = render_cloze_display(sentence, &blanks);
        assert_eq!(result, "저[1]밥[2]먹어요");
    }

    #[test]
    fn test_load_test_exercises_pack_fixture() {
        use std::path::PathBuf;

        // Load the test_exercises_pack fixture used by E2E tests
        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/integration/fixtures/test_exercises_pack");

        // Skip if fixture doesn't exist (e.g., in CI without fixtures)
        if !fixture_path.exists() {
            return;
        }

        let result = load_exercises_from_pack(&fixture_path, "exercises");
        assert!(result.is_ok(), "Failed to load test_exercises_pack: {:?}", result.err());

        let data = result.unwrap();
        assert!(!data.lessons.is_empty(), "test_exercises_pack should have lessons");

        // Verify lesson 1 exists and has exercises
        let lesson_1 = data.lessons.iter().find(|l| l.lesson == 1);
        assert!(lesson_1.is_some(), "test_exercises_pack should have lesson 1");

        let lesson = lesson_1.unwrap();
        assert!(
            !lesson.exercises.is_empty(),
            "lesson 1 should have exercises"
        );

        // Verify each exercise is valid
        for ex in &lesson.exercises {
            assert!(
                validate_exercise(ex).is_ok(),
                "Exercise {} should be valid",
                ex.id
            );
        }
    }
}
