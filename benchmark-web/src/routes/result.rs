//! Result routes - mirrors Java ResultController.java
//! REST API for results management.
//!
//! # URL Design Philosophy
//!
//! This module exposes a **clean RESTful API** where `agent` and `model` are separate path segments:
//!
//! URL pattern: `/results/{agent}/{model}/{language}/{exercise}`
//!
//! Example: `/results/pi/gemma-4-26b/java/custom-set`
//!
//! ## Internal vs. External Representation
//!
//! - **External (URLs)**: Agent and model are always separate path segments
//! - **Internal (Filesystem/Cache)**: Results are stored in `{agent}-{model}` directories
//!
//! This module is responsible for translating between the two representations:
//! - URLs receive separate `agent` and `model` parameters
//! - Internal cache lookups reconstruct the directory name as `{agent}-{model}`
//! - API responses expose `agent` and `model` as separate fields, never the directory name
//!
//! ## Important: Never Expose Internal Structure
//!
//! ❌ **Wrong**: `/results/pi/pi-gemma-4-26b/java/custom-set` (exposes internal directory)
//! ✅ **Correct**: `/results/pi/gemma-4-26b/java/custom-set` (clean RESTful URL)
//!
//! The directory naming convention (`{agent}-{model}`) is an **implementation detail** that should
//! never leak into the public API surface.

use super::{AppState, TemplateEngine};
use axum::extract::{Path, Query};

use axum::routing::{get, post};
use axum::Router;
use axum::Json;
use axum::Extension;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::services::result_service::LoadingStatus;

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

/// Get loading status of the result cache.
pub async fn get_loading_status(Extension(state): Extension<AppState>) -> Json<LoadingStatus> {
    Json(state.service.get_loading_status())
}

/// Get recent results fragment (for HTMX).
pub async fn recent_results_fragment(
    Extension(state): Extension<AppState>,
    Query(params): Query<QueryParam>,
) -> Json<RecentResultsResponse> {
    // If results are still loading, return empty results
    if !state.service.get_loading_status().loaded {
        return Json(RecentResultsResponse { results: vec![] });
    }

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
    // If results are still loading, return empty results
    if !state.service.get_loading_status().loaded {
        return Json(IndividualResultsResponse { results: vec![], total: 0 });
    }

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
///
/// **URL Design Note:**
/// The RESTful URL path keeps `agent` and `model` as separate path segments.
/// This is the public API surface and should never expose the internal directory structure.
///
/// **Internal Storage:**
/// Result files are stored on disk in directories named `{agent}-{model}` (e.g., `pi-gemma-4-26b`).
/// This directory naming is an implementation detail for organizing result files and caching.
/// The URL layer must always translate between the clean RESTful path and the internal directory format.
///
/// **Why separate?**
/// - Clean, predictable URLs: `/results/pi/gemma-4-26b/java/custom-set`
/// - Proper filtering: Agent and model are independent dimensions
/// - Future-proof: Adding new models doesn't change URL structure
/// - Cache key compatibility: Internal cache uses `{directory}/{lang}/{exercise}` format
///
/// **Never do this:**
/// ❌ `/results/{agent}/{agent}-{model}/{lang}/{ex}` (exposes internal structure)
/// ❌ Filter by directory name in API responses (leaks implementation detail)
///
/// **Always do this:**
/// ✅ Keep agent and model separate in URLs
/// ✅ Reconstruct directory name only when accessing the filesystem/cache internally
/// ✅ Use `agent` and `model` fields from result objects, not directory parsing
pub async fn result_detail_page(
    Extension(state): Extension<AppState>,
    Extension(templates): Extension<TemplateEngine>,
    Path((agent, model, lang, ex)): Path<(String, String, String, String)>,
) -> axum::response::Html<String> {
    // Internal implementation detail: reconstruct directory name for cache lookup
    // This is NOT exposed in the URL - the URL keeps agent and model separate
    let dir = format!("{}-{}", agent, model);
    tracing::debug!("RESULT DETAIL: agent={}, model={}, internal_dir={}, lang={}, ex={}", agent, model, dir, lang, ex);
    
    // Get the full result details (filter by model to ensure we get the right one)
    let results = state.service.list_individual_results(
        Some(&lang),
        Some(&agent),
        Some(&model),
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
        // Token statistics
        ctx.insert("input_tokens", &r.input_tokens);
        ctx.insert("output_tokens", &r.output_tokens);
        ctx.insert("cached_input_tokens", &r.cached_input_tokens);
        ctx.insert("uncached_input_tokens", &r.uncached_input_tokens);
        ctx.insert("total_tokens", &r.total_tokens);
        ctx.insert("tokens_per_sec", &r.tokens_per_sec);
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
    // Internal: reconstruct directory name for cache key lookup
    // URL keeps agent and model separate; directory is an implementation detail
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
        .route("/api/results/loading-status", get(get_loading_status))
        .route("/recent-results-fragment", get(recent_results_fragment))
        .route("/api/individual-results", get(get_individual_results))
        .route("/exercise-detail", get(exercise_detail))
}
