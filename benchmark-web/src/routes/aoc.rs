//! Advent of Code 2015 routes.
//!
//! A dedicated, category-isolated set of endpoints for the AoC benchmark:
//! the run form, day listing, answer validation, and AoC scheduling.

use super::AppState;
use super::queue::{deserialize_vec_u32, FlexibleForm};
use crate::services::aoc_validator::{validate, ValidateRequest};
use axum::routing::{get, post};
use axum::Router;
use axum::Json;
use axum::Extension;
use serde::Deserialize;
use std::collections::HashMap;

// =============================================================================
// Handlers
// =============================================================================

/// AoC run form page.
pub async fn run_aoc_form(
    Extension(state): Extension<AppState>,
    Extension(templates): Extension<super::TemplateEngine>,
) -> impl axum::response::IntoResponse {
    let models = state.service.fetch_models().await.unwrap_or_default();
    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Run AoC 2015 Benchmark");
    ctx.insert("models", &models);
    ctx.insert("days", &(1..=25).collect::<Vec<u32>>());
    axum::response::Html(templates.render("run-aoc.tera", &ctx))
}

/// List the AoC puzzle days.
pub async fn get_aoc_days() -> Json<Vec<u32>> {
    Json(benchmark_aoc2015::days())
}

/// Validate a submitted AoC answer.
pub async fn validate_answer(
    Json(request): Json<ValidateRequest>,
) -> Json<serde_json::Value> {
    match validate(&request) {
        Some(resp) => Json(serde_json::to_value(&resp).unwrap()),
        None => Json(serde_json::json!({
            "error": "invalid request: year must be 2015 and day in 1..=25",
        })),
    }
}

/// Schedule an AoC benchmark run.
///
/// One queue item per selected day (sequential execution). The `user` seed is
/// carried on each item so the executor can regenerate the input and build the
/// validator URL.
pub async fn schedule_aoc(
    Extension(state): Extension<AppState>,
    FlexibleForm(request): FlexibleForm<ScheduleAocRequest>,
) -> Json<serde_json::Value> {
    let model = if request.agent == "reference" {
        "reference".to_string()
    } else {
        request.model.clone()
    };

    let days: Vec<u32> = if request.days.is_empty() {
        benchmark_aoc2015::days()
    } else {
        request.days.clone()
    };

    let items = state.service.schedule_aoc(
        request.agent.clone(),
        days,
        model,
        request.thinking_level.clone(),
        request.user.clone().unwrap_or_else(|| "benchmark".to_string()),
        request.retry,
    );

    let items_map: Vec<HashMap<String, String>> =
        items.iter().map(|i| aoc_item_to_map(i)).collect();
    Json(serde_json::json!({
        "status": "scheduled",
        "count": items.len(),
        "items": items_map,
    }))
}

/// Map an AoC queue item to a simple serializable shape.
fn aoc_item_to_map(item: &crate::models::queue_item::BenchmarkQueueItem) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("id".to_string(), item.id.clone());
    m.insert("agent".to_string(), item.agent_name.clone());
    m.insert("model".to_string(), item.model.clone());
    m.insert("exercise".to_string(), item.exercise.clone());
    m.insert("status".to_string(), item.status.to_string());
    m.insert("category".to_string(), item.category.to_string());
    m
}

// =============================================================================
// Request types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct ScheduleAocRequest {
    pub agent: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub thinking_level: Option<String>,
    /// Selected days (1..=25). Empty means all days. Handles repeated form
    /// fields, single values, and JSON array strings (via [`FlexibleForm`]).
    #[serde(default, deserialize_with = "deserialize_vec_u32")]
    pub days: Vec<u32>,
    /// Seed user for input generation. Defaults to `benchmark`.
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub retry: bool,
}

// =============================================================================
// Router
// =============================================================================

/// Register AoC routes.
pub fn register(app: Router<()>) -> Router<()> {
    app.route("/run-aoc", get(run_aoc_form))
        .route("/api/aoc/days", get(get_aoc_days))
        .route("/api/aoc/validate", post(validate_answer))
        .route("/api/aoc/queue/schedule", post(schedule_aoc))
}
