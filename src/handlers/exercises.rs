//! Exercise handlers for interactive grammar practice.

use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;

use super::NavContext;
use crate::auth::AuthContext;
use crate::content::{load_exercises_from_pack, Exercise};
use crate::filters;
use crate::services::pack_manager;
use crate::state::AppState;
use crate::validation::validate_cloze;

/// Summary of a pack with exercises for the index page.
pub struct ExercisePackSummary {
    pub pack_id: String,
    pub pack_name: String,
    pub description: Option<String>,
    pub lesson_count: usize,
    pub exercise_count: usize,
}

/// Summary of a lesson for the pack overview.
pub struct ExerciseLessonSummary {
    pub number: u8,
    pub title: Option<String>,
    pub exercise_count: usize,
}

/// Template for exercise index (list packs with exercises).
#[derive(Template)]
#[template(path = "exercises/index.html")]
pub struct ExerciseIndexTemplate {
    pub nav: NavContext,
    pub packs: Vec<ExercisePackSummary>,
}

/// Template for pack exercise overview (list lessons).
#[derive(Template)]
#[template(path = "exercises/pack.html")]
pub struct ExercisePackTemplate {
    pub nav: NavContext,
    pub pack_id: String,
    pub pack_name: String,
    pub lessons: Vec<ExerciseLessonSummary>,
}

/// Template for exercise session (the actual exercise UI).
#[derive(Template)]
#[template(path = "exercises/session.html")]
pub struct ExerciseSessionTemplate {
    pub nav: NavContext,
    pub pack_id: String,
    pub pack_name: String,
    pub lesson: u8,
    pub exercise_index: usize,
    pub exercise_count: usize,
    pub exercise: Exercise,
    pub show_english: bool,
}

/// HTMX partial for cloze exercise component.
#[derive(Template)]
#[template(path = "exercises/cloze.html")]
pub struct ClozePartialTemplate {
    pub exercise: Exercise,
    pub exercise_index: usize,
    pub exercise_count: usize,
    pub pack_id: String,
    pub lesson: u8,
}

/// HTMX partial for cloze answer feedback.
#[derive(Template)]
#[template(path = "exercises/cloze_feedback.html")]
pub struct ClozeFeedbackTemplate {
    pub correct: bool,
    pub feedback: Option<String>,
    pub expected: String,
    pub user_answer: String,
    pub english: Option<String>,
    pub pack_id: String,
    pub lesson: u8,
    pub exercise_index: usize,
    pub exercise_count: usize,
}

/// List all packs with exercises.
pub async fn exercise_index(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Html<String> {
    let app_conn = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => return Html(super::DB_ERROR_HTML.to_string()),
    };

    let packs = get_exercise_packs(&app_conn, auth.user_id);

    let template = ExerciseIndexTemplate {
        nav: NavContext::from_auth(&auth),
        packs,
    };

    Html(template.render().unwrap_or_default())
}

/// Get accessible packs with exercises for a user.
fn get_exercise_packs(
    app_conn: &rusqlite::Connection,
    user_id: i64,
) -> Vec<ExercisePackSummary> {
    let accessible_packs = pack_manager::get_accessible_packs(app_conn, user_id, None);

    accessible_packs
        .into_iter()
        .filter_map(|pack| {
            // Only include packs that have exercise config
            let ex_config = pack.manifest.exercises.as_ref()?;

            // Try to load exercises to get counts
            let data = load_exercises_from_pack(&pack.path, &ex_config.directory).ok()?;

            // Skip if no exercises
            if data.lessons.is_empty() {
                return None;
            }

            let exercise_count: usize = data.lessons.iter().map(|l| l.exercises.len()).sum();

            Some(ExercisePackSummary {
                pack_id: pack.manifest.id,
                pack_name: pack.manifest.name,
                description: pack.manifest.description,
                lesson_count: data.lessons.len(),
                exercise_count,
            })
        })
        .collect()
}

/// Show lessons in a pack's exercises.
pub async fn exercise_pack(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(pack_id): Path<String>,
) -> Response {
    let app_conn = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => return Html(super::DB_ERROR_HTML.to_string()).into_response(),
    };

    // Find pack
    let accessible_packs = pack_manager::get_accessible_packs(&app_conn, auth.user_id, None);
    let pack = match accessible_packs.iter().find(|p| p.manifest.id == pack_id) {
        Some(p) => p,
        None => return Redirect::to("/exercises").into_response(),
    };

    // Check for exercise config
    let ex_config = match pack.manifest.exercises.as_ref() {
        Some(c) => c,
        None => return Redirect::to("/exercises").into_response(),
    };

    // Load exercises
    let data = match load_exercises_from_pack(&pack.path, &ex_config.directory) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Failed to load exercises from pack {}: {}", pack_id, e);
            return Html("<h1>Error loading exercises</h1>".to_string()).into_response();
        }
    };

    let lessons: Vec<ExerciseLessonSummary> = data
        .lessons
        .iter()
        .map(|l| ExerciseLessonSummary {
            number: l.lesson,
            title: l.title.clone(),
            exercise_count: l.exercises.len(),
        })
        .collect();

    let template = ExercisePackTemplate {
        nav: NavContext::from_auth(&auth),
        pack_id: pack.manifest.id.clone(),
        pack_name: pack.manifest.name.clone(),
        lessons,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

/// Start an exercise session for a lesson.
pub async fn exercise_session(
    State(state): State<AppState>,
    auth: AuthContext,
    Path((pack_id, lesson_num)): Path<(String, u8)>,
) -> Response {
    let app_conn = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => return Html(super::DB_ERROR_HTML.to_string()).into_response(),
    };

    // Find pack
    let accessible_packs = pack_manager::get_accessible_packs(&app_conn, auth.user_id, None);
    let pack = match accessible_packs.iter().find(|p| p.manifest.id == pack_id) {
        Some(p) => p,
        None => return Redirect::to("/exercises").into_response(),
    };

    // Check for exercise config
    let ex_config = match pack.manifest.exercises.as_ref() {
        Some(c) => c,
        None => return Redirect::to("/exercises").into_response(),
    };

    // Load exercises
    let data = match load_exercises_from_pack(&pack.path, &ex_config.directory) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Failed to load exercises from pack {}: {}", pack_id, e);
            return Html("<h1>Error loading exercises</h1>".to_string()).into_response();
        }
    };

    // Find the lesson
    let lesson = match data.lessons.iter().find(|l| l.lesson == lesson_num) {
        Some(l) => l,
        None => return Redirect::to(&format!("/exercises/pack/{}", pack_id)).into_response(),
    };

    // Get first exercise
    let exercise = match lesson.exercises.first() {
        Some(e) => e.clone(),
        None => return Redirect::to(&format!("/exercises/pack/{}", pack_id)).into_response(),
    };

    let template = ExerciseSessionTemplate {
        nav: NavContext::from_auth(&auth),
        pack_id: pack.manifest.id.clone(),
        pack_name: pack.manifest.name.clone(),
        lesson: lesson_num,
        exercise_index: 0,
        exercise_count: lesson.exercises.len(),
        exercise,
        show_english: false,
    };

    Html(template.render().unwrap_or_default()).into_response()
}

/// Form data for checking a cloze answer.
#[derive(Deserialize)]
pub struct CheckClozeForm {
    pub pack_id: String,
    pub lesson: u8,
    pub exercise_index: usize,
    pub blank_position: u8,
    pub answer: String,
}

/// HTMX handler to check a cloze answer.
pub async fn check_cloze(
    State(state): State<AppState>,
    auth: AuthContext,
    Form(form): Form<CheckClozeForm>,
) -> Response {
    let app_conn = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => return Html("<div class=\"error\">Database error</div>".to_string()).into_response(),
    };

    // Find pack and load exercises
    let accessible_packs = pack_manager::get_accessible_packs(&app_conn, auth.user_id, None);
    let pack = match accessible_packs.iter().find(|p| p.manifest.id == form.pack_id) {
        Some(p) => p,
        None => return Html("<div class=\"error\">Pack not found</div>".to_string()).into_response(),
    };

    let ex_config = match pack.manifest.exercises.as_ref() {
        Some(c) => c,
        None => return Html("<div class=\"error\">No exercises</div>".to_string()).into_response(),
    };

    let data = match load_exercises_from_pack(&pack.path, &ex_config.directory) {
        Ok(d) => d,
        Err(_) => return Html("<div class=\"error\">Load error</div>".to_string()).into_response(),
    };

    // Find the lesson and exercise
    let lesson = match data.lessons.iter().find(|l| l.lesson == form.lesson) {
        Some(l) => l,
        None => return Html("<div class=\"error\">Lesson not found</div>".to_string()).into_response(),
    };

    let exercise = match lesson.exercises.get(form.exercise_index) {
        Some(e) => e,
        None => return Html("<div class=\"error\">Exercise not found</div>".to_string()).into_response(),
    };

    // Find the blank being answered
    let blank = match exercise.blanks.iter().find(|b| b.position == form.blank_position) {
        Some(b) => b,
        None => return Html("<div class=\"error\">Blank not found</div>".to_string()).into_response(),
    };

    // Validate the answer
    let result = validate_cloze(&form.answer, &blank.answer);

    let template = ClozeFeedbackTemplate {
        correct: result.is_correct(),
        feedback: result.feedback().map(|s| s.to_string()),
        expected: blank.answer.clone(),
        user_answer: form.answer,
        english: exercise.english.clone(),
        pack_id: form.pack_id,
        lesson: form.lesson,
        exercise_index: form.exercise_index,
        exercise_count: lesson.exercises.len(),
    };

    Html(template.render().unwrap_or_default()).into_response()
}

/// Form data for getting next exercise.
#[derive(Deserialize)]
pub struct NextExerciseForm {
    pub pack_id: String,
    pub lesson: u8,
    pub exercise_index: usize,
}

/// HTMX handler to get the next exercise.
pub async fn next_exercise(
    State(state): State<AppState>,
    auth: AuthContext,
    Form(form): Form<NextExerciseForm>,
) -> Response {
    let app_conn = match state.auth_db.lock() {
        Ok(conn) => conn,
        Err(_) => return Html("<div class=\"error\">Database error</div>".to_string()).into_response(),
    };

    // Find pack and load exercises
    let accessible_packs = pack_manager::get_accessible_packs(&app_conn, auth.user_id, None);
    let pack = match accessible_packs.iter().find(|p| p.manifest.id == form.pack_id) {
        Some(p) => p,
        None => return Html("<div class=\"error\">Pack not found</div>".to_string()).into_response(),
    };

    let ex_config = match pack.manifest.exercises.as_ref() {
        Some(c) => c,
        None => return Html("<div class=\"error\">No exercises</div>".to_string()).into_response(),
    };

    let data = match load_exercises_from_pack(&pack.path, &ex_config.directory) {
        Ok(d) => d,
        Err(_) => return Html("<div class=\"error\">Load error</div>".to_string()).into_response(),
    };

    // Find the lesson
    let lesson = match data.lessons.iter().find(|l| l.lesson == form.lesson) {
        Some(l) => l,
        None => return Html("<div class=\"error\">Lesson not found</div>".to_string()).into_response(),
    };

    let next_index = form.exercise_index + 1;

    // Check if there are more exercises
    if next_index >= lesson.exercises.len() {
        // Return completion message with proper styling and data-testid attributes
        let total = lesson.exercises.len();
        return Html(format!(
            r#"<div id="card-container" data-testid="card-container" class="text-center">
  <div class="mb-4 sm:mb-6 bg-white dark:bg-gray-800 shadow-lg rounded-xl p-6 sm:p-10">
    <div data-testid="lesson-complete" class="py-4">
      <div class="flex items-center justify-center gap-2 text-green-600 dark:text-green-400 mb-4">
        <iconify-icon icon="heroicons:check-badge" width="48" height="48"></iconify-icon>
      </div>
      <h2 class="text-2xl font-bold text-green-600 dark:text-green-400 mb-4">Lesson Complete!</h2>
      <p class="text-gray-600 dark:text-gray-300 mb-6">You've completed all {} exercises in this lesson.</p>
      <a href="/exercises/pack/{}" class="inline-block w-full bg-indigo-500 hover:bg-indigo-600 text-white font-semibold py-3 px-6 rounded-lg transition-colors">
        Back to Lessons
      </a>
    </div>
  </div>
</div>
<!-- OOB swap for progress bar (show 100%) -->
<div id="exercise-progress" hx-swap-oob="true" data-testid="progress-bar" class="mb-4 flex items-center justify-center gap-3 text-xs text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2">
  <span class="flex items-center gap-1">
    <span class="text-green-500 dark:text-green-400">&#9679;</span>
    Complete: {} / {}
  </span>
  <span class="flex-1">
    <div class="w-full bg-gray-200 dark:bg-gray-600 rounded-full h-2">
      <div class="bg-green-500 h-2 rounded-full transition-all duration-300" style="width: 100%"></div>
    </div>
  </span>
</div>"#,
            total,
            form.pack_id,
            total,
            total
        )).into_response();
    }

    let exercise = lesson.exercises[next_index].clone();
    let exercise_count = lesson.exercises.len();

    let template = ClozePartialTemplate {
        exercise,
        exercise_index: next_index,
        exercise_count,
        pack_id: form.pack_id,
        lesson: form.lesson,
    };

    // Append OOB swap for progress bar (HTMX response)
    let progress_pct = (next_index + 1) * 100 / exercise_count;
    let oob_progress = format!(
        r#"<div id="exercise-progress" hx-swap-oob="true" data-testid="progress-bar" class="mb-4 flex items-center justify-center gap-3 text-xs text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2">
  <span class="flex items-center gap-1">
    <span class="text-indigo-500 dark:text-indigo-400">&#9679;</span>
    Progress: {} / {}
  </span>
  <span class="flex-1">
    <div class="w-full bg-gray-200 dark:bg-gray-600 rounded-full h-2">
      <div class="bg-green-500 h-2 rounded-full transition-all duration-300" style="width: {}%"></div>
    </div>
  </span>
</div>"#,
        next_index + 1,
        exercise_count,
        progress_pct
    );

    let html_content = format!("{}{}", template.render().unwrap_or_default(), oob_progress);
    Html(html_content).into_response()
}
