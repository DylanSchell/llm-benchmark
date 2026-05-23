//! Result routes - mirrors Java ResultController.java
//! REST API for results management.

use super::{AppState, TemplateEngine};
use axum::extract::{Path, Query};

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

/// Result detail page: /results/{agent}/{model}/{lang}/{ex}
pub async fn result_detail_page(
    Extension(state): Extension<AppState>,
    Extension(templates): Extension<TemplateEngine>,
    Path((agent, model, lang, ex)): Path<(String, String, String, String)>,
) -> axum::response::Html<String> {
    // Reconstruct directory name as {agent}-{model} to find the file
    let dir = format!("{}-{}", agent, model);
    tracing::info!("RESULT DETAIL: agent={}, model={}, dir={}, lang={}, ex={}", agent, model, dir, lang, ex);
    
    // Get the full result details
    let results = state.service.list_individual_results(
        Some(&lang),
        Some(&agent),
        None,
        Some(&ex),
        false,
    );
    tracing::info!("Found {} results", results.len());
    
    let mut ctx = tera::Context::new();
    
    if let Some(r) = results.first() {
        // Verify the model matches what's in the URL
        if r.model != model {
            return axum::response::Html("<h1>Result Not Found</h1><p>The requested result does not exist.</p>".to_string());
        }
        
        ctx.insert("title", &format!("Result: {} - {} - {}", agent, r.model, ex));
        ctx.insert("agent", &r.agent);
        ctx.insert("model", &r.model);
        ctx.insert("directory", &dir);
        ctx.insert("language", &lang);
        ctx.insert("exercise", &ex);
        ctx.insert("success", &r.success);
        ctx.insert("timestamp", &r.timestamp);
        ctx.insert("duration", &r.duration);
        ctx.insert("has_trace", &r.has_trace_file);
        ctx.insert("trace_url", &r.trace_url);
    } else {
        return axum::response::Html("<h1>Result Not Found</h1><p>The requested result does not exist.</p>".to_string());
    }
    
    axum::response::Html(templates.render("result-detail.tera", &ctx))
}

/// Get trace for a result: /results/{agent}/{model}/{lang}/{ex}/trace
pub async fn result_detail_trace(
    Extension(state): Extension<AppState>,
    Path((agent, model, lang, ex)): Path<(String, String, String, String)>,
) -> impl IntoResponse {
    // Reconstruct directory name as {agent}-{model}
    let dir = format!("{}-{}", agent, model);
    let key = format!("{}/{}/{}", dir, lang, ex);
    match state.service.get_trace_content(&key) {
        Ok(Some(content)) => (
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            content,
        ),
        _ => {
            let body = "<html><body><h1>Trace Not Found</h1><p>No trace file available for this result.</p></body></html>";
            (
                axum::http::StatusCode::NOT_FOUND,
                [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body.to_string(),
            )
        }
    }
}

/// Exercise detail page (legacy).
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
