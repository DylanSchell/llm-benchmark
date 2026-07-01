//! Compare routes - side-by-side comparison of two benchmark runs (models).
//! Users select two models and see exercise execution times compared in a table.

use super::{AppState, TemplateEngine};
use axum::extract::Query;
use axum::routing::get;
use axum::Router;
use axum::Extension;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Request/Response types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CompareQuery {
    pub a: Option<String>,  // model key for run A (format: "agent - model")
    pub b: Option<String>,  // model key for run B (format: "agent - model")
}

/// A single row in the comparison table — one exercise, two durations.
#[derive(Debug, Serialize)]
pub struct CompareRow {
    pub language: String,
    pub exercise: String,
    pub a_duration: Option<String>,
    pub a_sort: Option<f64>,
    pub sort_a: f64,
    pub a_success: bool,
    pub a_tps: Option<f64>,
    pub tps_a_fmt: String,
    pub b_duration: Option<String>,
    pub b_sort: Option<f64>,
    pub sort_b: f64,
    pub b_success: bool,
    pub b_tps: Option<f64>,
    pub tps_b_fmt: String,
    /// Which side is faster: "a", "b", "tie"
    pub faster: String,
    /// Pre-formatted ratio string like "0.75x"
    pub ratio_fmt: String,
    /// Sortable ratio placeholder (999 for missing)
    pub sort_ratio: f64,
    /// Which side the ratio favors: "a", "b", or "tie"
    pub ratio_favor: String,
}

// =============================================================================
// Handlers
// =============================================================================

/// Comparison page.
pub async fn compare_page(
    Extension(state): Extension<AppState>,
    Extension(templates): Extension<TemplateEngine>,
    Query(params): Query<CompareQuery>,
) -> impl axum::response::IntoResponse {
    let _models = state.service.get_models();
    // Build full "agent - model" keys from individual results
    let all_results = state.service.list_individual_results(None, None, None, None, false);
    let mut model_keys: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for r in &all_results {
        let key = format!("{} - {}", r.agent, r.model);
        if seen.insert(key.clone()) {
            model_keys.push(key);
        }
    }
    model_keys.sort();

    let a_key = params.a.clone().unwrap_or_default();
    let b_key = params.b.clone().unwrap_or_default();

    let mut rows: Vec<CompareRow> = Vec::new();

    if !a_key.is_empty() && !b_key.is_empty() {
        // Parse agent/model from keys
        let (a_agent, a_model) = split_model_key(&a_key);
        let (b_agent, b_model) = split_model_key(&b_key);

        // Get all exercises for run A
        let a_results = state.service.list_individual_results(
            None,
            Some(&a_agent),
            Some(&a_model),
            None,
            false,
        );

        // Get all exercises for run B, index by (language, exercise)
        let b_results = state.service.list_individual_results(
            None,
            Some(&b_agent),
            Some(&b_model),
            None,
            false,
        );
        let b_map: HashMap<(String, String), &crate::services::result_service::IndividualResult> = b_results
            .iter()
            .map(|r| ((r.language.clone(), r.exercise.clone()), r))
            .collect();

        // Build rows from A results, merging B data
        for a in &a_results {
            let b = b_map.get(&(a.language.clone(), a.exercise.clone()));

            let a_dur = a.sort_duration;
            let b_dur = b.and_then(|r| r.sort_duration);

            let a_tps = a.tokens_per_sec;
            let b_tps = b.and_then(|r| r.tokens_per_sec);

            let faster = match (a_dur, b_dur) {
                (Some(ad), Some(bd)) if ad < bd => "a".to_string(),
                (Some(ad), Some(bd)) if ad > bd => "b".to_string(),
                (Some(_), Some(_)) => "tie".to_string(),
                (Some(_), None) => "a".to_string(),
                (None, Some(_)) => "b".to_string(),
                (None, None) => "tie".to_string(),
            };

            let (ratio_fmt, sort_ratio, ratio_favor) = match (a_dur, b_dur) {
                (Some(ad), Some(bd)) if bd > 0.0 => {
                    let r = ad / bd;
                    let favor = if r < 1.0 { "a" } else if r > 1.0 { "b" } else { "tie" };
                    (format!("{:.2}x", r), r, favor.to_string())
                }
                _ => (String::new(), 999.0, String::new()),
            };

            let tps_a_fmt = a_tps.map(|v| format!("{:.1}t/s", v)).unwrap_or_default();
            let tps_b_fmt = b_tps.map(|v| format!("{:.1}t/s", v)).unwrap_or_default();

            rows.push(CompareRow {
                language: a.language.clone(),
                exercise: a.exercise.clone(),
                a_duration: a.duration.clone(),
                a_sort: a_dur,
                sort_a: a_dur.unwrap_or(0.0),
                a_success: a.success,
                a_tps,
                tps_a_fmt,
                b_duration: b.map(|r| r.duration.clone()).flatten(),
                b_sort: b_dur,
                sort_b: b_dur.unwrap_or(0.0),
                b_success: b.map(|r| r.success).unwrap_or(false),
                b_tps,
                tps_b_fmt,
                faster,
                ratio_fmt,
                sort_ratio,
                ratio_favor,
            });
        }

        // Also add exercises that only B has (but A doesn't)
        let a_keys: std::collections::HashSet<(String, String)> = a_results
            .iter()
            .map(|r| (r.language.clone(), r.exercise.clone()))
            .collect();

        for b in &b_results {
            if !a_keys.contains(&(b.language.clone(), b.exercise.clone())) {
                let b_dur = b.sort_duration;
                let b_tps = b.tokens_per_sec;
                let tps_b_fmt = b_tps.map(|v| format!("{:.1}t/s", v)).unwrap_or_default();

                rows.push(CompareRow {
                    language: b.language.clone(),
                    exercise: b.exercise.clone(),
                    a_duration: None,
                    a_sort: None,
                    sort_a: 0.0,
                    a_success: false,
                    a_tps: None,
                    tps_a_fmt: String::new(),
                    b_duration: b.duration.clone(),
                    b_sort: b_dur,
                    sort_b: b_dur.unwrap_or(0.0),
                    b_success: b.success,
                    b_tps,
                    tps_b_fmt,
                    faster: "b".to_string(),
                    ratio_fmt: String::new(),
                    sort_ratio: 999.0,
                    ratio_favor: String::new(),
                });
            }
        }

        // Sort by language, then exercise
        rows.sort_by(|a, b| {
            a.language.cmp(&b.language).then_with(|| a.exercise.cmp(&b.exercise))
        });
    }

    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Compare Models");
    ctx.insert("models", &model_keys);
    ctx.insert("a", &a_key);
    ctx.insert("b", &b_key);
    ctx.insert("rows", &rows);
    // Row count for display
    let row_count = rows.len();
    ctx.insert("row_count", &row_count);

    axum::response::Html(templates.render("compare.tera", &ctx))
}

/// Split "agent - model" into (agent, model).
fn split_model_key(key: &str) -> (String, String) {
    if let Some((agent, model)) = key.split_once(" - ") {
        (agent.to_string(), model.to_string())
    } else {
        (key.to_string(), key.to_string())
    }
}

// =============================================================================
// Router
// =============================================================================

/// Register compare routes.
pub fn register(app: Router<()>) -> Router<()> {
    app.route("/compare", get(compare_page))
}
