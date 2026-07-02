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
    #[serde(default)]
    pub metric: Option<String>,  // which metric to compare: "speed" (default), "tokens", "turns", "toolcalls"
}

/// A single row in the comparison table — one exercise, two values for the selected metric.
#[derive(Debug, Serialize)]
pub struct CompareRow {
    pub language: String,
    pub exercise: String,
    /// Display value for A (depends on selected metric)
    pub a_display: String,
    /// Sort value for A
    pub sort_a: f64,
    /// Display value for B
    pub b_display: String,
    /// Sort value for B
    pub sort_b: f64,
    pub a_success: bool,
    pub b_success: bool,
    /// Which side is better: "a", "b", "tie"
    pub faster: String,
    /// Pre-formatted ratio string like "0.75x"
    pub ratio_fmt: String,
    /// Sortable ratio placeholder (999 for missing)
    pub sort_ratio: f64,
    /// Which side the ratio favors: "a", "b", or "tie"
    pub ratio_favor: String,
    /// Column label for the metric being compared
    pub metric_label: String,
    /// Display format: "duration" or "count"
    pub metric_fmt: String,
    // Legacy duration-specific fields (preserved for backward compatibility)
    pub a_duration: Option<String>,
    pub a_sort_duration: Option<f64>,
    pub b_duration: Option<String>,
    pub b_sort_duration: Option<f64>,
    pub a_tps: Option<f64>,
    pub tps_a_fmt: String,
    pub b_tps: Option<f64>,
    pub tps_b_fmt: String,
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
    let metric = params.metric.clone().unwrap_or_else(|| "speed".to_string());

    let (metric_label, metric_fmt) = match metric.as_str() {
        "tokens" => ("Total Tokens", "count"),
        "turns" => ("Turns", "count"),
        "toolcalls" => ("Tool Calls", "count"),
        _ => ("Duration", "duration"),
    };

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

        /// Extract (display_value, sort_value) for the selected metric.
        fn metric_values(
            r: &crate::services::result_service::IndividualResult,
            metric: &str,
        ) -> (String, Option<f64>) {
            match metric {
                "tokens" => {
                    let v = r.input_tokens + r.output_tokens;
                    let sort = if v > 0 { Some(v as f64) } else { None };
                    let disp = if v > 0 { format_tokens_compact(v) } else { String::new() };
                    (disp, sort)
                }
                "turns" => {
                    let v = r.turn_count;
                    let sort = if v > 0 { Some(v as f64) } else { None };
                    let disp = if v > 0 { v.to_string() } else { String::new() };
                    (disp, sort)
                }
                "toolcalls" => {
                    let v = r.tool_call_count;
                    let sort = if v > 0 { Some(v as f64) } else { None };
                    let disp = if v > 0 { v.to_string() } else { String::new() };
                    (disp, sort)
                }
                _ => {
                    // speed (default) — use duration
                    (r.duration.clone().unwrap_or_default(), r.sort_duration)
                }
            }
        }

        /// Format a raw u64 token count compactly (e.g. "12.3K", "1.2M").
        fn format_tokens_compact(n: u64) -> String {
            if n >= 1_000_000 {
                format!("{:.1}M", n as f64 / 1_000_000.0)
            } else if n >= 1_000 {
                format!("{:.1}K", n as f64 / 1_000.0)
            } else {
                n.to_string()
            }
        }

        // Build rows from A results, merging B data
        for a in &a_results {
            let b = b_map.get(&(a.language.clone(), a.exercise.clone()));

            let (a_disp, a_sort) = metric_values(a, &metric);
            let (b_disp, b_sort) = if let Some(b) = b {
                metric_values(b, &metric)
            } else {
                (String::new(), None)
            };

            // Lower is better for all current metrics
            let faster = match (a_sort, b_sort) {
                (Some(av), Some(bv)) if av < bv => "a".to_string(),
                (Some(av), Some(bv)) if av > bv => "b".to_string(),
                (Some(_), Some(_)) => "tie".to_string(),
                (Some(_), None) => "a".to_string(),
                (None, Some(_)) => "b".to_string(),
                (None, None) => "tie".to_string(),
            };

            let (ratio_fmt, sort_ratio, ratio_favor) = match (a_sort, b_sort) {
                (Some(av), Some(bv)) if bv > 0.0 => {
                    let r = av / bv;
                    let favor = if r < 1.0 { "a" } else if r > 1.0 { "b" } else { "tie" };
                    (format!("{:.2}x", r), r, favor.to_string())
                }
                _ => (String::new(), 999.0, String::new()),
            };

            // Legacy duration-specific fields (for template backward compat)
            let a_dur = a.sort_duration;
            let b_dur = b.and_then(|r| r.sort_duration);
            let a_tps = a.tokens_per_sec;
            let b_tps = b.and_then(|r| r.tokens_per_sec);
            let tps_a_fmt = a_tps.map(|v| format!("{:.1}t/s", v)).unwrap_or_default();
            let tps_b_fmt = b_tps.map(|v| format!("{:.1}t/s", v)).unwrap_or_default();

            rows.push(CompareRow {
                language: a.language.clone(),
                exercise: a.exercise.clone(),
                a_display: a_disp,
                sort_a: a_sort.unwrap_or(0.0),
                b_display: b_disp,
                sort_b: b_sort.unwrap_or(0.0),
                a_success: a.success,
                b_success: b.map(|r| r.success).unwrap_or(false),
                faster,
                ratio_fmt,
                sort_ratio,
                ratio_favor,
                metric_label: metric_label.to_string(),
                metric_fmt: metric_fmt.to_string(),
                a_duration: a.duration.clone(),
                a_sort_duration: a_dur,
                b_duration: b.map(|r| r.duration.clone()).flatten(),
                b_sort_duration: b_dur,
                a_tps,
                tps_a_fmt,
                b_tps,
                tps_b_fmt,
            });
        }

        // Also add exercises that only B has (but A doesn't)
        let a_keys: std::collections::HashSet<(String, String)> = a_results
            .iter()
            .map(|r| (r.language.clone(), r.exercise.clone()))
            .collect();

        for b in &b_results {
            if !a_keys.contains(&(b.language.clone(), b.exercise.clone())) {
                let (b_disp, b_sort) = metric_values(b, &metric);
                let b_dur = b.sort_duration;
                let b_tps = b.tokens_per_sec;
                let tps_b_fmt = b_tps.map(|v| format!("{:.1}t/s", v)).unwrap_or_default();

                rows.push(CompareRow {
                    language: b.language.clone(),
                    exercise: b.exercise.clone(),
                    a_display: String::new(),
                    sort_a: 0.0,
                    b_display: b_disp,
                    sort_b: b_sort.unwrap_or(0.0),
                    a_success: false,
                    b_success: b.success,
                    faster: "b".to_string(),
                    ratio_fmt: String::new(),
                    sort_ratio: 999.0,
                    ratio_favor: String::new(),
                    metric_label: metric_label.to_string(),
                    metric_fmt: metric_fmt.to_string(),
                    a_duration: None,
                    a_sort_duration: None,
                    b_duration: b.duration.clone(),
                    b_sort_duration: b_dur,
                    a_tps: None,
                    tps_a_fmt: String::new(),
                    b_tps,
                    tps_b_fmt,
                });
            }
        }

        // Sort by language, then exercise
        rows.sort_by(|a, b| {
            a.language.cmp(&b.language).then_with(|| a.exercise.cmp(&b.exercise))
        });
    }

    // Compute speed comparison counts (done in Rust, not Tera — Tera's {% set %}
    // is loop-scoped, so counter variables in for-loops never actually update).
    let a_faster_count = rows.iter().filter(|r| r.faster == "a").count();
    let b_faster_count = rows.iter().filter(|r| r.faster == "b").count();
    let tie_count = rows.iter().filter(|r| r.faster == "tie").count();

    // Also extract short model names for labels (e.g., "claude-sonnet-5" from "pi - claude-sonnet-5")
    let a_label = split_model_key(&a_key).1;
    let b_label = split_model_key(&b_key).1;

    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Compare Models");
    ctx.insert("models", &model_keys);
    ctx.insert("a", &a_key);
    ctx.insert("a_label", &a_label);
    ctx.insert("b", &b_key);
    ctx.insert("b_label", &b_label);
    ctx.insert("rows", &rows);
    // Row count for display
    let row_count = rows.len();
    ctx.insert("row_count", &row_count);
    // Speed comparison counts
    ctx.insert("a_faster_count", &a_faster_count);
    ctx.insert("b_faster_count", &b_faster_count);
    ctx.insert("tie_count", &tie_count);
    ctx.insert("metric", &metric);
    ctx.insert("metric_label", &metric_label);
    ctx.insert("metric_fmt", &metric_fmt);

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
