//! Results routes - mirrors Java ResultsController.java
//! REST API for results browsing and statistics.

use super::{AppState, TemplateEngine};
use axum::extract::{Path, Query};
use axum::routing::get;
use axum::Router;
use axum::Json;
use axum::Extension;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Request/Response types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub language: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub exercise: Option<String>,
    #[serde(alias = "quick", default)]
    pub quick_only: bool,
}

impl StatsQuery {
    /// Convert empty string query params to None ("all" selection).
    pub fn cleaned(&self) -> StatsQuery {
        StatsQuery {
            language: self.language.as_ref().filter(|s| !s.is_empty()).cloned(),
            agent: self.agent.as_ref().filter(|s| !s.is_empty()).cloned(),
            model: self.model.as_ref().filter(|s| !s.is_empty()).cloned(),
            exercise: self.exercise.as_ref().filter(|s| !s.is_empty()).cloned(),
            quick_only: self.quick_only,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TableFragmentQuery {
    pub language: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub exercise: Option<String>,
    #[serde(default)]
    pub quick_only: bool,
}

#[derive(Debug, Serialize)]
pub struct ResultsTable {
    pub results: Vec<HashMap<String, String>>,
    pub total: usize,
}

// =============================================================================
// Helpers
// =============================================================================

fn result_to_map(r: &crate::services::result_service::IndividualResult) -> HashMap<String, String> {
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
    map.insert(
        "timestamp_epoch".to_string(),
        r.timestamp_epoch.map(|e| e.to_string()).unwrap_or_default(),
    );
    map.insert("has_trace_file".to_string(), r.has_trace_file.to_string());
    map
}

// =============================================================================
// Handlers
// =============================================================================

/// Results page.
pub async fn results_page(
    Extension(state): Extension<AppState>,
    Extension(templates): Extension<TemplateEngine>,
    Query(params): Query<StatsQuery>,
) -> impl axum::response::IntoResponse {
    let q = params.cleaned();
    let stats = state.service.get_statistics(
        q.language.as_deref(),
        q.agent.as_deref(),
        q.model.as_deref(),
        q.exercise.as_deref(),
        q.quick_only,
    );
    let results = state.service.list_individual_results(
        q.language.as_deref(), q.agent.as_deref(),
        q.model.as_deref(), q.exercise.as_deref(),
        q.quick_only,
    );
    let models = state.service.get_models();
    let languages = state.service.get_languages();
    let exercises = state.service.get_exercises(q.language.as_deref());

    let q = params.cleaned();
    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Results");
    ctx.insert("stats", &stats);
    ctx.insert("individual_results", &results);
    ctx.insert("models", &models);
    ctx.insert("languages", &languages);
    ctx.insert("exercises", &exercises);
    ctx.insert("filter_language", &q.language.as_deref().unwrap_or(""));
    ctx.insert("filter_agent", &q.agent.as_deref().unwrap_or(""));
    ctx.insert("filter_model", &q.model.as_deref().unwrap_or(""));
    ctx.insert("filter_exercise", &q.exercise.as_deref().unwrap_or(""));
    ctx.insert("filter_quick", &q.quick_only);

    axum::response::Html(templates.render("results.tera", &ctx))
}

/// API endpoint for the results list.
pub async fn get_results_api(
    Extension(state): Extension<AppState>,
    Query(params): Query<StatsQuery>,
) -> Json<ResultsTable> {
    let q = params.cleaned();
    let results = state.service.list_individual_results(
        q.language.as_deref(), q.agent.as_deref(),
        q.model.as_deref(), q.exercise.as_deref(),
        q.quick_only,
    );
    let result_maps: Vec<HashMap<String, String>> = results.iter().map(result_to_map).collect();
    Json(ResultsTable { results: result_maps, total: results.len() })
}





/// Get results by language/exercise: /results/{lang}/{ex}
pub async fn get_results_by_lang_ex(
    Extension(state): Extension<AppState>,
    Path((lang, ex)): Path<(String, String)>,
) -> Json<Vec<HashMap<String, String>>> {
    let results = state.service.list_individual_results(Some(&lang), None, None, Some(&ex), false);
    let result_maps: Vec<HashMap<String, String>> = results.iter().map(|r| {
        let mut map = HashMap::new();
        map.insert("filename".to_string(), r.filename.clone());
        map.insert("detail_url".to_string(), r.detail_url.clone());
        map.insert("agent".to_string(), r.agent.clone());
        map.insert("model".to_string(), r.model.clone());
        map.insert("success".to_string(), r.success.to_string());
        map.insert("timestamp".to_string(), r.timestamp.clone().unwrap_or_default());
        map
    }).collect();
    Json(result_maps)
}

/// API endpoint: /results/api/{agent}/{lang}/{ex}
pub async fn get_results_api_agent_lang_ex(
    Extension(state): Extension<AppState>,
    Path((_agent, lang, ex)): Path<(String, String, String)>,
) -> Json<Vec<HashMap<String, String>>> {
    let results = state.service.list_individual_results(Some(&lang), Some(&_agent), None, Some(&ex), false);
    let result_maps: Vec<HashMap<String, String>> = results.iter().map(|r| {
        let mut map = HashMap::new();
        map.insert("filename".to_string(), r.filename.clone());
        map.insert("detail_url".to_string(), r.detail_url.clone());
        map.insert("model".to_string(), r.model.clone());
        map.insert("success".to_string(), r.success.to_string());
        map.insert("timestamp".to_string(), r.timestamp.clone().unwrap_or_default());
        map
    }).collect();
    Json(result_maps)
}

/// Get aggregate statistics: /results/api/stats
pub async fn get_stats(
    Extension(state): Extension<AppState>,
    Query(params): Query<StatsQuery>,
) -> Json<crate::services::result_service::Statistics> {
    let q = params.cleaned();
    let stats = state.service.get_statistics(
        q.language.as_deref(),
        q.agent.as_deref(),
        q.model.as_deref(),
        q.exercise.as_deref(),
        q.quick_only,
    );
    Json(stats)
}

/// Results table fragment (for HTMX): /results/table-fragment
pub async fn table_fragment(
    Extension(state): Extension<AppState>,
    Query(params): Query<TableFragmentQuery>,
) -> Json<ResultsTable> {
    let results = state.service.list_individual_results(
        params.language.as_deref(), params.agent.as_deref(),
        params.model.as_deref(), params.exercise.as_deref(),
        params.quick_only,
    );
    let result_maps: Vec<HashMap<String, String>> = results.iter().map(result_to_map).collect();
    Json(ResultsTable { results: result_maps, total: results.len() })
}

// =============================================================================
// Router
// =============================================================================

/// Register results routes.
pub fn register(app: Router<()>) -> Router<()> {
    app.route("/results", get(results_page))
        .route("/results/api/results", get(get_results_api))
        // More specific routes must come before parameterized ones
        .route("/results/{agent}/{dir}/{lang}/{ex}/trace", get(super::result::result_detail_trace))
        .route("/results/{agent}/{dir}/{lang}/{ex}", get(super::result::result_detail_page))
        .route("/results/{lang}/{ex}", get(get_results_by_lang_ex))
        .route("/results/api/{agent}/{lang}/{ex}", get(get_results_api_agent_lang_ex))
        .route("/results/api/stats", get(get_stats))
        .route("/results/table-fragment", get(table_fragment))
}
