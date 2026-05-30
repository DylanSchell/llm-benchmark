//! Scoring routes - visualization and ranking of benchmark results.

use super::{AppState, TemplateEngine};
use axum::extract::Query;
use axum::routing::get;
use axum::Router;
use axum::Json;
use axum::Extension;
use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize)]
pub struct ScoreFilterQuery {
    pub language: Option<String>,
    pub agent: Option<String>,
    pub quick: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ScoredResultsResponse {
    pub results: Vec<ScoredResultView>,
    pub total: usize,
    pub filters: ScoreFilters,
}

#[derive(Debug, Serialize)]
pub struct ScoredResultView {
    pub agent: String,
    pub model: String,
    pub language: String,
    pub exercise: String,
    pub success: bool,
    pub success_rate: f64,
    pub speed_score: f64,
    pub token_score: f64,
    pub composite_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_sec: Option<f64>,
    pub detail_url: String,
}

#[derive(Debug, Serialize)]
pub struct ModelScoresResponse {
    pub scores: Vec<ModelScoreView>,
    pub filters: ScoreFilters,
}

#[derive(Debug, Serialize)]
pub struct ModelScoreView {
    pub name: String,
    pub avg_composite_score: f64,
    pub avg_success_rate: f64,
    pub avg_speed_score: f64,
    pub avg_token_score: f64,
    pub total_tokens: u64,
    pub total_runs: u32,
}

#[derive(Debug, Serialize)]
pub struct ScoreFilters {
    pub language: Option<String>,
    pub agent: Option<String>,
    pub quick_only: bool,
}

/// Scoring dashboard page: /scoring
pub async fn scoring_dashboard(
    Extension(state): Extension<AppState>,
    Extension(templates): Extension<TemplateEngine>,
    Query(params): Query<ScoreFilterQuery>,
) -> axum::response::Html<String> {
    let scored_results = state.service.calculate_scores(
        params.language.as_deref(),
        params.agent.as_deref(),
        None,
        None,
        params.quick.unwrap_or(false),
    );

    // Sort by composite score descending
    let mut sorted_results = scored_results.clone();
    sorted_results.sort_by(|a, b| {
        b.composite_score.partial_cmp(&a.composite_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let model_scores = state.service.get_model_scores(
        params.language.as_deref(),
        params.agent.as_deref(),
        params.quick.unwrap_or(false),
    );

    // Sort models by composite score
    let mut sorted_models = model_scores.clone();
    sorted_models.sort_by(|a, b| {
        b.avg_composite_score.partial_cmp(&a.avg_composite_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Scoring Dashboard");
    ctx.insert("results", &sorted_results);
    ctx.insert("model_scores", &sorted_models);
    ctx.insert("total_results", &sorted_results.len());
    ctx.insert("filter_language", &params.language);
    ctx.insert("filter_agent", &params.agent);
    ctx.insert("filter_quick", &params.quick.unwrap_or(false));

    axum::response::Html(templates.render("scoring.tera", &ctx))
}

/// Get scored results as JSON API
pub async fn get_scored_results(
    Extension(state): Extension<AppState>,
    Query(params): Query<ScoreFilterQuery>,
) -> Json<ScoredResultsResponse> {
    let results = state.service.calculate_scores(
        params.language.as_deref(),
        params.agent.as_deref(),
        None,
        None,
        params.quick.unwrap_or(false),
    );

    // Sort by composite score descending
    let mut sorted = results.clone();
    sorted.sort_by(|a, b| {
        b.composite_score.partial_cmp(&a.composite_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total = sorted.len();

    Json(ScoredResultsResponse {
        results: sorted.into_iter().map(|r| ScoredResultView {
            agent: r.agent,
            model: r.model,
            language: r.language,
            exercise: r.exercise,
            success: r.success,
            success_rate: r.success_rate,
            speed_score: r.speed_score,
            token_score: r.token_score,
            composite_score: r.composite_score,
            duration: r.duration,
            output_tokens: r.output_tokens,
            tokens_per_sec: r.tokens_per_sec,
            detail_url: r.detail_url,
        }).collect(),
        total,
        filters: ScoreFilters {
            language: params.language,
            agent: params.agent,
            quick_only: params.quick.unwrap_or(false),
        },
    })
}

/// Get model scores as JSON API
pub async fn get_model_scores(
    Extension(state): Extension<AppState>,
    Query(params): Query<ScoreFilterQuery>,
) -> Json<ModelScoresResponse> {
    let scores = state.service.get_model_scores(
        params.language.as_deref(),
        params.agent.as_deref(),
        params.quick.unwrap_or(false),
    );

    // Sort by composite score descending
    let mut sorted = scores.clone();
    sorted.sort_by(|a, b| {
        b.avg_composite_score.partial_cmp(&a.avg_composite_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Json(ModelScoresResponse {
        scores: sorted.into_iter().map(|s| ModelScoreView {
            name: s.name,
            avg_composite_score: s.avg_composite_score,
            avg_success_rate: s.avg_success_rate,
            avg_speed_score: s.avg_speed_score,
            avg_token_score: s.avg_token_score,
            total_tokens: s.total_tokens,
            total_runs: s.total_runs,
        }).collect(),
        filters: ScoreFilters {
            language: params.language,
            agent: params.agent,
            quick_only: params.quick.unwrap_or(false),
        },
    })
}

/// Register scoring routes.
pub fn register(app: Router<()>) -> Router<()> {
    app.route("/scoring", get(scoring_dashboard))
        .route("/api/scored-results", get(get_scored_results))
        .route("/api/model-scores", get(get_model_scores))
}
