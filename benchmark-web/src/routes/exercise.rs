//! Exercise routes - mirrors Java ExerciseController.java
//! Handles listing available languages and exercises.

use super::AppState;
use axum::extract::Path;
use axum::routing::get;
use axum::Router;
use axum::Json;
use axum::Extension;
use std::collections::HashMap;

// =============================================================================
// Handlers
// =============================================================================

/// API endpoint to get available languages and exercises.
pub async fn get_exercises(Extension(state): Extension<AppState>) -> Json<HashMap<String, Vec<String>>> {
    let mut exercises: HashMap<String, Vec<String>> = HashMap::new();
    let runner = state.service.get_exercise_runner();
    for language in runner.get_available_languages() {
        exercises.insert(language.clone(), runner.get_exercises_for_language(&language));
    }
    Json(exercises)
}

/// API endpoint to get available languages only.
pub async fn get_languages(Extension(state): Extension<AppState>) -> Json<Vec<String>> {
    let runner = state.service.get_exercise_runner();
    Json(runner.get_available_languages())
}

/// API endpoint to get exercises for a specific language.
pub async fn get_exercises_for_language(
    Extension(state): Extension<AppState>,
    Path(language): Path<String>,
) -> Json<Vec<String>> {
    let runner = state.service.get_exercise_runner();
    Json(runner.get_exercises_for_language(&language))
}

// =============================================================================
// Router
// =============================================================================

/// Register exercise routes.
pub fn register(app: Router<()>) -> Router<()> {
    app.route("/api/exercises", get(get_exercises))
        .route("/api/languages", get(get_languages))
        .route("/api/exercises/{language}", get(get_exercises_for_language))
}
