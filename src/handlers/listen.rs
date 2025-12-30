use askama::Template;
use axum::{
    extract::{Query, Form},
    response::{Html, IntoResponse},
};
use rand::prelude::IndexedRandom;
use serde::Deserialize;

use super::settings::{has_lesson1, has_lesson2, has_lesson3};
use crate::auth::AuthContext;
use crate::filters;
use crate::audio::{
    get_available_syllables, get_row_romanization, get_row_syllables, load_manifest,
    vowel_romanization,
};
use crate::config;

/// A syllable with audio info for listening practice
#[derive(Clone)]
pub struct ListenSyllable {
    pub character: String,
    pub romanization: String,
    pub audio_path: String,
}

/// A choice option for the listening quiz
#[derive(Clone)]
pub struct ListenChoice {
    pub character: String,
    pub romanization: String,
    pub number: u8,
}

/// A consonant row containing syllables
#[derive(Clone)]
pub struct ListenRow {
    pub consonant: String,
    pub romanization: String,
    pub syllables: Vec<ListenSyllable>,
}

/// A listening tier (corresponds to a lesson)
pub struct ListenTier {
    pub tier: u8,
    pub lesson_id: String,
    pub name: String,
    pub vowels: Vec<String>,
    pub vowel_romanizations: Vec<String>,
    pub rows: Vec<ListenRow>,
    pub total_syllables: usize,
}

/// Build a listening tier from manifest using shared utilities
fn build_tier_from_manifest(tier: u8, lesson_id: &str, name: &str) -> Option<ListenTier> {
    let manifest = load_manifest(lesson_id)?;
    let available_syllables = get_available_syllables(lesson_id);

    if available_syllables.is_empty() {
        return None;
    }

    // Use shared vowel romanization function
    let vowel_romanizations: Vec<String> = manifest
        .vowels_order
        .iter()
        .map(|v| vowel_romanization(v).to_string())
        .collect();

    let mut rows = Vec::new();
    let mut total_syllables = 0;

    // Lesson 3 has vowel rows (no consonants_order), lessons 1/2 have consonant rows
    let is_matrix = !manifest.consonants_order.is_empty();

    if is_matrix {
        // Lesson 1/2: Iterate over consonant rows
        for c in &manifest.consonants_order {
            let syllable_infos = get_row_syllables(&manifest, c);

            // Filter to only syllables with audio and convert to ListenSyllable
            let syllables: Vec<ListenSyllable> = syllable_infos
                .into_iter()
                .filter(|s| available_syllables.contains(&s.romanization))
                .map(|s| ListenSyllable {
                    character: s.character,
                    romanization: s.romanization.clone(),
                    audio_path: format!(
                        "/audio/scraped/htsk/{}/syllables/{}.mp3",
                        lesson_id, s.romanization
                    ),
                })
                .collect();

            if !syllables.is_empty() {
                total_syllables += syllables.len();
                rows.push(ListenRow {
                    consonant: c.clone(),
                    romanization: get_row_romanization(&manifest, c),
                    syllables,
                });
            }
        }
    } else {
        // Lesson 3: Iterate over vowel rows (diphthongs/combined vowels)
        for v in &manifest.vowels_order {
            let syllable_infos = get_row_syllables(&manifest, v);

            // Filter to only syllables with audio and convert to ListenSyllable
            let syllables: Vec<ListenSyllable> = syllable_infos
                .into_iter()
                .filter(|s| available_syllables.contains(&s.romanization))
                .map(|s| ListenSyllable {
                    character: s.character,
                    romanization: s.romanization.clone(),
                    audio_path: format!(
                        "/audio/scraped/htsk/{}/syllables/{}.mp3",
                        lesson_id, s.romanization
                    ),
                })
                .collect();

            if !syllables.is_empty() {
                total_syllables += syllables.len();
                rows.push(ListenRow {
                    consonant: v.clone(), // Using vowel as the "row" identifier
                    romanization: vowel_romanization(v).to_string(),
                    syllables,
                });
            }
        }
    }

    if rows.is_empty() {
        return None;
    }

    Some(ListenTier {
        tier,
        lesson_id: lesson_id.to_string(),
        name: name.to_string(),
        vowels: manifest.vowels_order,
        vowel_romanizations,
        rows,
        total_syllables,
    })
}

/// Get all syllables from a tier as a flat list
fn get_all_syllables(tier: &ListenTier) -> Vec<(String, String)> {
    tier.rows
        .iter()
        .flat_map(|row| {
            row.syllables.iter().map(|s| (s.character.clone(), s.audio_path.clone()))
        })
        .collect()
}

/// Pick a random syllable from a tier
fn pick_random_syllable(tier: &ListenTier) -> Option<(String, String)> {
    let syllables = get_all_syllables(tier);
    syllables.choose(&mut rand::rng()).cloned()
}

/// Get all syllables as choices (for hard mode)
fn get_all_choices(tier: &ListenTier) -> Vec<ListenChoice> {
    let choices: Vec<ListenChoice> = tier.rows
        .iter()
        .flat_map(|row| row.syllables.clone())
        .enumerate()
        .map(|(i, s)| ListenChoice {
            character: s.character,
            romanization: s.romanization,
            number: (i + 1) as u8,
        })
        .collect();

    // Sort by consonant row order (already in order from tier.rows)
    choices
}

/// Generate 4 choices: 1 correct answer + 3 random distractors
fn generate_choices(tier: &ListenTier, correct_syllable: &str) -> Vec<ListenChoice> {
    use rand::seq::SliceRandom;

    let all_syllables: Vec<ListenSyllable> = tier.rows
        .iter()
        .flat_map(|row| row.syllables.clone())
        .collect();

    // Find the correct syllable's romanization
    let correct_rom = all_syllables
        .iter()
        .find(|s| s.character == correct_syllable)
        .map(|s| s.romanization.clone())
        .unwrap_or_default();

    // Get distractors (syllables that aren't the correct one)
    let mut distractors: Vec<&ListenSyllable> = all_syllables
        .iter()
        .filter(|s| s.character != correct_syllable)
        .collect();

    // Shuffle and take 3
    distractors.shuffle(&mut rand::rng());
    let distractors: Vec<ListenChoice> = distractors
        .into_iter()
        .take(3)
        .map(|s| ListenChoice {
            character: s.character.clone(),
            romanization: s.romanization.clone(),
            number: 0, // Will be assigned after shuffling
        })
        .collect();

    // Create the correct choice
    let correct_choice = ListenChoice {
        character: correct_syllable.to_string(),
        romanization: correct_rom,
        number: 0,
    };

    // Combine and shuffle all choices
    let mut choices: Vec<ListenChoice> = vec![correct_choice];
    choices.extend(distractors);
    choices.shuffle(&mut rand::rng());

    // Assign numbers 1-4
    for (i, choice) in choices.iter_mut().enumerate() {
        choice.number = (i + 1) as u8;
    }

    choices
}

// ============ Templates ============

#[derive(Template)]
#[template(path = "listen/index.html")]
pub struct ListenIndexTemplate {
    pub tier1_available: bool,
    pub tier1_count: usize,
    pub tier2_available: bool,
    pub tier2_count: usize,
    pub tier3_available: bool,
    pub tier3_count: usize,
}

#[derive(Template)]
#[template(path = "listen/practice.html")]
pub struct ListenPracticeTemplate {
    pub tier: u8,
    pub tier_name: String,
    pub choices: Vec<ListenChoice>,
    pub current_syllable: String,
    pub current_audio: String,
    pub correct: u32,
    pub total: u32,
    pub show_feedback: bool,
    pub was_correct: bool,
    pub correct_answer: String,
    pub user_answer: String,
    pub hard_mode: bool,
    pub all_syllables: Vec<ListenChoice>, // Full matrix for hard mode
}

#[derive(Template)]
#[template(path = "listen/partial_answer.html")]
pub struct ListenAnswerPartialTemplate {
    pub tier: u8,
    pub choices: Vec<ListenChoice>,
    pub current_syllable: String,
    pub current_audio: String,
    pub correct: u32,
    pub total: u32,
    pub was_correct: bool,
    pub correct_answer: String,
    pub user_answer: String,
    pub hard_mode: bool,
    pub all_syllables: Vec<ListenChoice>,
}

// ============ Query/Form structs ============

#[derive(Deserialize)]
pub struct StartQuery {
    pub tier: u8,
    #[serde(default)]
    pub hard_mode: bool,
}

#[derive(Deserialize)]
pub struct AnswerForm {
    pub tier: u8,
    pub answer: String,
    pub correct_syllable: String,
    pub correct: u32,
    pub total: u32,
    #[serde(default)]
    pub hard_mode: bool,
}

#[derive(Deserialize)]
pub struct SkipQuery {
    pub tier: u8,
    pub correct: u32,
    pub total: u32,
    #[serde(default)]
    pub hard_mode: bool,
}

// ============ Handlers ============

/// GET /listen - Tier selection page
pub async fn listen_index(_auth: AuthContext) -> impl IntoResponse {
    let tier1 = if has_lesson1() {
        config::get_listen_tier_info(1)
            .and_then(|(lesson_id, name)| build_tier_from_manifest(1, lesson_id, name))
    } else {
        None
    };

    let tier2 = if has_lesson2() {
        config::get_listen_tier_info(2)
            .and_then(|(lesson_id, name)| build_tier_from_manifest(2, lesson_id, name))
    } else {
        None
    };

    let tier3 = if has_lesson3() {
        config::get_listen_tier_info(3)
            .and_then(|(lesson_id, name)| build_tier_from_manifest(3, lesson_id, name))
    } else {
        None
    };

    let template = ListenIndexTemplate {
        tier1_available: tier1.is_some(),
        tier1_count: tier1.as_ref().map(|t| t.total_syllables).unwrap_or(0),
        tier2_available: tier2.is_some(),
        tier2_count: tier2.as_ref().map(|t| t.total_syllables).unwrap_or(0),
        tier3_available: tier3.is_some(),
        tier3_count: tier3.as_ref().map(|t| t.total_syllables).unwrap_or(0),
    };

    Html(template.render().unwrap_or_default())
}

/// GET /listen/start?tier=1 - Start practice for a tier
pub async fn listen_start(_auth: AuthContext, Query(query): Query<StartQuery>) -> impl IntoResponse {
    let (lesson_id, tier_name) = match config::get_listen_tier_info(query.tier) {
        Some((lid, name)) => (lid, name),
        None => return Html("Invalid tier".to_string()),
    };

    let tier = match build_tier_from_manifest(query.tier, lesson_id, tier_name) {
        Some(t) => t,
        None => return Html("Tier not available".to_string()),
    };

    let (current_syllable, current_audio) = match pick_random_syllable(&tier) {
        Some((s, a)) => (s, a),
        None => return Html("No syllables available".to_string()),
    };

    let choices = generate_choices(&tier, &current_syllable);
    let all_syllables = get_all_choices(&tier);

    let template = ListenPracticeTemplate {
        tier: query.tier,
        tier_name: tier_name.to_string(),
        choices,
        current_syllable,
        current_audio,
        correct: 0,
        total: 0,
        show_feedback: false,
        was_correct: false,
        correct_answer: String::new(),
        user_answer: String::new(),
        hard_mode: query.hard_mode,
        all_syllables,
    };

    Html(template.render().unwrap_or_default())
}

/// POST /listen/answer - Submit answer and get next syllable (legacy full page)
pub async fn listen_answer(_auth: AuthContext, Form(form): Form<AnswerForm>) -> impl IntoResponse {
    let (lesson_id, tier_name) = match config::get_listen_tier_info(form.tier) {
        Some((lid, name)) => (lid, name),
        None => return Html("Invalid tier".to_string()),
    };

    let tier = match build_tier_from_manifest(form.tier, lesson_id, tier_name) {
        Some(t) => t,
        None => return Html("Tier not available".to_string()),
    };

    let was_correct = form.answer == form.correct_syllable;
    let new_correct = form.correct + if was_correct { 1 } else { 0 };
    let new_total = form.total + 1;

    // Pick next syllable
    let (next_syllable, next_audio) = match pick_random_syllable(&tier) {
        Some((s, a)) => (s, a),
        None => return Html("No syllables available".to_string()),
    };

    let choices = generate_choices(&tier, &next_syllable);
    let all_syllables = get_all_choices(&tier);

    let template = ListenPracticeTemplate {
        tier: form.tier,
        tier_name: tier_name.to_string(),
        choices,
        current_syllable: next_syllable,
        current_audio: next_audio,
        correct: new_correct,
        total: new_total,
        show_feedback: true,
        was_correct,
        correct_answer: form.correct_syllable,
        user_answer: form.answer,
        hard_mode: form.hard_mode,
        all_syllables,
    };

    Html(template.render().unwrap_or_default())
}

/// POST /listen/answer-htmx - Submit answer via HTMX (partial update)
pub async fn listen_answer_htmx(_auth: AuthContext, Form(form): Form<AnswerForm>) -> impl IntoResponse {
    let lesson_id = match config::get_listen_tier_info(form.tier) {
        Some((lid, _)) => lid,
        None => return Html("Invalid tier".to_string()),
    };

    let tier = match build_tier_from_manifest(form.tier, lesson_id, "") {
        Some(t) => t,
        None => return Html("Tier not available".to_string()),
    };

    let was_correct = form.answer == form.correct_syllable;
    let new_correct = form.correct + if was_correct { 1 } else { 0 };
    let new_total = form.total + 1;

    // Pick next syllable
    let (next_syllable, next_audio) = match pick_random_syllable(&tier) {
        Some((s, a)) => (s, a),
        None => return Html("No syllables available".to_string()),
    };

    let choices = generate_choices(&tier, &next_syllable);
    let all_syllables = get_all_choices(&tier);

    let template = ListenAnswerPartialTemplate {
        tier: form.tier,
        choices,
        current_syllable: next_syllable,
        current_audio: next_audio,
        correct: new_correct,
        total: new_total,
        was_correct,
        correct_answer: form.correct_syllable,
        user_answer: form.answer,
        hard_mode: form.hard_mode,
        all_syllables,
    };

    Html(template.render().unwrap_or_default())
}

/// GET /listen/skip - Skip current syllable
pub async fn listen_skip(_auth: AuthContext, Query(query): Query<SkipQuery>) -> impl IntoResponse {
    let (lesson_id, tier_name) = match config::get_listen_tier_info(query.tier) {
        Some((lid, name)) => (lid, name),
        None => return Html("Invalid tier".to_string()),
    };

    let tier = match build_tier_from_manifest(query.tier, lesson_id, tier_name) {
        Some(t) => t,
        None => return Html("Tier not available".to_string()),
    };

    let (next_syllable, next_audio) = match pick_random_syllable(&tier) {
        Some((s, a)) => (s, a),
        None => return Html("No syllables available".to_string()),
    };

    let choices = generate_choices(&tier, &next_syllable);
    let all_syllables = get_all_choices(&tier);

    let template = ListenPracticeTemplate {
        tier: query.tier,
        tier_name: tier_name.to_string(),
        choices,
        current_syllable: next_syllable,
        current_audio: next_audio,
        correct: query.correct,
        total: query.total,
        show_feedback: false,
        was_correct: false,
        correct_answer: String::new(),
        user_answer: String::new(),
        hard_mode: query.hard_mode,
        all_syllables,
    };

    Html(template.render().unwrap_or_default())
}
