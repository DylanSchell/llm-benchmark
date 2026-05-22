//! Result routes - mirrors Java ResultController.java
//! REST API for results management.

use super::{AppState, TemplateEngine};
use axum::extract::Query;

use axum::routing::{get, post};
use axum::Router;
use axum::Json;
use axum::Extension;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Request/Response types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct FilterQuery {
    pub language: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub exercise: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QueryParam {
    pub agent: Option<String>,
    pub language: Option<String>,
    pub model: Option<String>,
    pub exercise: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub message: String,
    pub loaded: usize,
}

#[derive(Debug, Serialize)]
pub struct IndividualResultsResponse {
    pub results: Vec<HashMap<String, String>>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct RecentResultsResponse {
    pub results: Vec<HashMap<String, String>>,
}

// =============================================================================
// Handlers
// =============================================================================

/// Refresh the result cache.
pub async fn refresh(Extension(state): Extension<AppState>) -> Json<RefreshResponse> {
    state.service.refresh_result_cache();
    Json(RefreshResponse { message: "Result cache refreshed".to_string(), loaded: 0 })
}

/// Get recent results fragment (for HTMX).
pub async fn recent_results_fragment(
    Extension(state): Extension<AppState>,
    Query(params): Query<QueryParam>,
) -> Json<RecentResultsResponse> {
    let results = state.service.list_individual_results(
        params.language.as_deref(),
        params.agent.as_deref(),
        params.model.as_deref(),
        params.exercise.as_deref(),
        false,
    );
    let result_maps: Vec<HashMap<String, String>> = results.iter().map(|r| {
        let mut map = HashMap::new();
        map.insert("filename".to_string(), r.filename.clone());
        map.insert("detail_url".to_string(), r.detail_url.clone());
        map.insert("trace_url".to_string(), r.trace_url.clone());
        map.insert("agent".to_string(), r.agent.clone());
        map.insert("language".to_string(), r.language.clone());
        map.insert("model".to_string(), r.model.clone());
        map.insert("exercise".to_string(), r.exercise.clone());
        map.insert("success".to_string(), r.success.to_string());
        map.insert("timestamp".to_string(), r.timestamp.clone().unwrap_or_default());
        map
    }).collect();
    Json(RecentResultsResponse { results: result_maps })
}

/// Get individual results.
pub async fn get_individual_results(
    Extension(state): Extension<AppState>,
    Query(params): Query<FilterQuery>,
) -> Json<IndividualResultsResponse> {
    let results = state.service.list_individual_results(
        params.language.as_deref(),
        params.agent.as_deref(),
        params.model.as_deref(),
        params.exercise.as_deref(),
        false,
    );
    let result_maps: Vec<HashMap<String, String>> = results.iter().map(|r| {
        let mut map = HashMap::new();
        map.insert("filename".to_string(), r.filename.clone());
        map.insert("detail_url".to_string(), r.detail_url.clone());
        map.insert("trace_url".to_string(), r.trace_url.clone());
        map.insert("agent".to_string(), r.agent.clone());
        map.insert("language".to_string(), r.language.clone());
        map.insert("model".to_string(), r.model.clone());
        map.insert("exercise".to_string(), r.exercise.clone());
        map.insert("success".to_string(), r.success.to_string());
        map.insert("timestamp".to_string(), r.timestamp.clone().unwrap_or_default());
        map.insert("has_trace_file".to_string(), r.has_trace_file.to_string());
        map
    }).collect();
    let total = result_maps.len();
    Json(IndividualResultsResponse { results: result_maps, total })
}

// =============================================================================
// Router
// =============================================================================

/// Exercise detail page.
pub async fn exercise_detail(
    Extension(state): Extension<AppState>,
    Extension(templates): Extension<TemplateEngine>,
    Query(params): Query<QueryParam>,
) -> impl IntoResponse {
    let results = state.service.list_individual_results(
        params.language.as_deref(),
        params.agent.as_deref(),
        params.model.as_deref(),
        params.exercise.as_deref(),
        false,
    );
    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Exercise Detail");
    ctx.insert("exercise_name", params.exercise.as_deref().unwrap_or(""));
    ctx.insert("language", &params.language);
    ctx.insert("agent", &params.agent);
    ctx.insert("model", &params.model);
    if let Some(r) = results.first() {
        ctx.insert("total_exercises", &1);
        ctx.insert("successful", &(if r.success { 1 } else { 0 }));
        ctx.insert("failed", &(if r.success { 0 } else { 1 }));
        ctx.insert("success_rate", if r.success { "100.0%" } else { "0.0%" });
        ctx.insert("output", &r.filename);
    } else {
        ctx.insert("total_exercises", &0);
        ctx.insert("successful", &0);
        ctx.insert("failed", &0);
        ctx.insert("success_rate", &"0.0%");
    }
    axum::response::Html(templates.render("exercise-detail.tera", &ctx))
}

/// Register result routes.
pub fn register(app: Router<()>) -> Router<()> {
    app.route("/api/results/refresh", post(refresh))
        .route("/recent-results-fragment", get(recent_results_fragment))
        .route("/api/individual-results", get(get_individual_results))
        .route("/exercise-detail", get(exercise_detail))
}
