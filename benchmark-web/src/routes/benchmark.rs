//! Benchmark routes - mirrors Java BenchmarkController.java
//! Handles starting, monitoring, and canceling benchmark runs.

use super::{AppState, TemplateEngine};
use axum::extract::{Path, Query};
use axum::response::{Html, IntoResponse, Sse};
use axum::response::sse::{Event, KeepAlive};
use axum::routing::{get, post};
use axum::Router;
use axum::Json;
use axum::Extension;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Request/Response types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct QuickParam {
    pub quick: Option<bool>,
    pub complete: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub id: String,
    pub status: String,
    pub agent: String,
    pub language: String,
    pub exercise: Option<String>,
    pub progress: f64,
    pub completed_exercises: u32,
    pub total_exercises: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CancelResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ActiveRunsResponse {
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct ActiveSessionsResponse {
    pub sessions: Vec<HashMap<String, String>>,
}

// =============================================================================
// Handlers
// =============================================================================

/// Dashboard page.
pub async fn dashboard(
    Extension(state): Extension<AppState>,
    Extension(templates): Extension<TemplateEngine>,
    Query(params): Query<QuickParam>,
) -> impl IntoResponse {
    let quick_only = params.quick.is_some_and(|q| q);
    let stats = state.service.get_statistics(None, None, None, None, quick_only);

    let active_sessions = state.service.get_active_sessions();
    // Show all queue items (pending, running, completed, failed, cancelled)
    let queue_items: Vec<_> = state.service.get_queue_items();

    let running_count = queue_items.iter().filter(|i| i.status.to_string() == "RUNNING").count();
    let pending_count = queue_items.iter().filter(|i| i.status.to_string() == "PENDING").count();
    let completed_count = queue_items.iter().filter(|i| i.status.to_string() == "COMPLETED").count();
    let failed_count = queue_items.iter().filter(|i| i.status.to_string() == "FAILED").count();
    let cancelled_count = queue_items.iter().filter(|i| i.status.to_string() == "CANCELLED").count();
    let total = queue_items.len();

    let running_width = if total > 0 { running_count as f64 / total as f64 * 100.0 } else { 0.0 };
    let pending_width = if total > 0 { pending_count as f64 / total as f64 * 100.0 } else { 0.0 };
    let completed_width = if total > 0 { completed_count as f64 / total as f64 * 100.0 } else { 0.0 };
    let failed_width = if total > 0 { failed_count as f64 / total as f64 * 100.0 } else { 0.0 };
    let cancelled_width = if total > 0 { cancelled_count as f64 / total as f64 * 100.0 } else { 0.0 };

    // Debug: Log the data being passed to template
    tracing::debug!("Dashboard context: {} active sessions, {} queue items", active_sessions.len(), queue_items.len());
    if !queue_items.is_empty() {
        let first_item = &queue_items[0];
        tracing::debug!("First queue item: id={}, status={:?}, agent={}, language={}, exercise={}", 
            first_item.id, first_item.status, first_item.agent_name, first_item.language, first_item.exercise);
    }
    if !active_sessions.is_empty() {
        let first_session = &active_sessions[0];
        tracing::debug!("First active session: id={}, status={:?}, agent={}, language={}", 
            first_session.id, first_session.status, first_session.agent_name, first_session.language());
    }

    // Convert queue items to a simpler format for the template
    let queue_items_simple: Vec<_> = queue_items.iter().map(|item| {
        serde_json::json!({
            "id": item.id,
            "status": item.status.to_string(),
            "agent_name": item.agent_name,
            "language": item.language,
            "exercise": item.exercise,
            "model": item.model,
            "retry": item.retry
        })
    }).collect();

    // Convert active sessions to a simpler format
    let active_sessions_simple: Vec<_> = active_sessions.iter().map(|session| {
        serde_json::json!({
            "id": session.id,
            "status": session.status.to_string(),
            "agent_name": session.agent_name,
            "language": session.language(),
            "exercise_name": session.exercise_name,
            "completed_exercises": session.completed_exercises,
            "total_exercises": session.total_exercises,
            "progress": session.progress,
            "session_id": session.id,
            "started_at": session.started_at.map(|d| d.to_string()).unwrap_or_default()
        })
    }).collect();

    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Dashboard");
    ctx.insert("stats", &stats);
    ctx.insert("active_runs", &active_sessions.len());
    ctx.insert("active_sessions", &active_sessions_simple);
    ctx.insert("queue_items", &queue_items_simple);
    ctx.insert("running_count", &running_count);
    ctx.insert("pending_count", &pending_count);
    ctx.insert("completed_count", &completed_count);
    ctx.insert("failed_count", &failed_count);
    ctx.insert("cancelled_count", &cancelled_count);
    ctx.insert("running_width", &format!("{:.1}", running_width));
    ctx.insert("pending_width", &format!("{:.1}", pending_width));
    ctx.insert("completed_width", &format!("{:.1}", completed_width));
    ctx.insert("failed_width", &format!("{:.1}", failed_width));
    ctx.insert("cancelled_width", &format!("{:.1}", cancelled_width));
    // Pass quick_bench and complete_bench to template so checkboxes reflect URL params
    let complete_only = params.complete.is_some_and(|c| c);
    ctx.insert("quick_bench", &quick_only);
    ctx.insert("complete_bench", &complete_only);

    match templates.tera.render("dashboard.tera", &ctx) {
        Ok(html) => Html(html),
        Err(e) => {
            tracing::error!("Dashboard template error: {}", e);
            // Return a generic error page — do not leak Tera error details
            let error_html = "<!DOCTYPE html><html><head><title>Error</title></head>\
                 <body><h1>Template Rendering Error</h1>\
                 <p>An internal error occurred. Please try again later.</p>\
                 <p><a href='/'>Retry</a></p>\
                 </body></html>";
            Html(error_html.to_string())
        }
    }
}

/// Run benchmark form page.
pub async fn run_form(
    Extension(state): Extension<AppState>,
    Extension(templates): Extension<TemplateEngine>,
) -> impl IntoResponse {
    // Fetch models from inference endpoint, fall back to cached if unavailable
    let models = state.service.fetch_models().await.unwrap_or_default();
    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Run Benchmark");
    ctx.insert("models", &models);
    Html(templates.render("run.tera", &ctx))
}

/// View a benchmark session.
pub async fn view_benchmark(
    Extension(state): Extension<AppState>,
    Extension(templates): Extension<TemplateEngine>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.service.get_session(&id) {
        Some(session) => {
            let mut ctx = tera::Context::new();
            ctx.insert("title", &"Benchmark Session");
            ctx.insert("session_id", &session.id);
            ctx.insert("status", &session.status.to_string());
            ctx.insert("progress", &session.progress);
            ctx.insert("completed", &session.completed_exercises);
            ctx.insert("total", &session.total_exercises);
            let output_lines = session.get_accumulated_output();
            ctx.insert("output", &if output_lines.is_empty() { Vec::<String>::new() } else { vec![output_lines] });
            ctx.insert("agent", &session.agent_name);
            ctx.insert("language", &session.language());
            ctx.insert("exercise", session.exercise_name.as_deref().unwrap_or("All"));
            ctx.insert("error_message", session.error_message.as_deref().unwrap_or(""));
            ctx.insert("started_at", &session.started_at.map(|d| d.to_string()).unwrap_or_default());
            axum::response::Html(templates.render("view_benchmark.tera", &ctx))
        }
        None => axum::response::Html("<h1>Session not found</h1>".to_string()),
    }
}

/// Get status of a benchmark run.
pub async fn get_status(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.service.get_session(&id) {
        Some(session) => {
            let response = StatusResponse {
                id: session.id.clone(),
                status: session.status.to_string(),
                agent: session.agent_name.clone(),
                language: session.language(),
                exercise: session.exercise_name.clone(),
                progress: session.progress,
                completed_exercises: session.completed_exercises,
                total_exercises: session.total_exercises,
                error_message: session.error_message.clone(),
            };
            axum::response::Json(serde_json::to_value(&response).unwrap())
        }
        None => {
            let mut response = HashMap::new();
            response.insert("error".to_string(), "Session not found".to_string());
            axum::response::Json(serde_json::to_value(response).unwrap())
        }
    }
}

/// SSE endpoint for live output streaming.
pub async fn stream_output(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    tracing::info!("SSE stream requested for session {}", id);
    let session = match state.service.get_session(&id) {
        Some(s) => s,
        None => {
            return Sse::new(async_stream::stream! {
                yield Ok::<_, anyhow::Error>(Event::default().event("error").json_data(&serde_json::json!({"message": "Session not found"})).unwrap_or_else(|_| Event::default().data("{\"message\":\"Session not found\"}")));
            }).into_response();
        }
    };

    // Each call to setup_sse() gets a fresh broadcast subscriber.
    // Broadcast channels support multiple consumers — no "already taken" issue.
    tracing::info!("Taking session receiver for {}", id);
    let mut rx = state.service.take_session_receiver(&id).expect("session exists");
    tracing::info!("Session receiver acquired, creating stream for {}", id);
    let session_id = session.id.clone();
    let status = session.status.to_string();
    let shutdown_flag = state.shutdown_flag.clone();

    let event_stream = async_stream::stream! {
macro_rules! sse_event {
            ($event:expr, $data:expr) => {
                Ok::<_, anyhow::Error>(Event::default()
                    .event($event)
                    .json_data($data)
                    .unwrap_or_else(|e| {
                        tracing::error!("SSE json_data failed: {}", e);
                        Event::default().data("{\"error\":\"serialization failed\"}")
                    }))
            };
        }

        // Send initial session info once
        yield sse_event!("session", &serde_json::json!({
            "id": session_id,
            "status": status
        }));

        // No need to send accumulated output via SSE — it's already rendered
        // in the server-side HTML template. Just start streaming live updates
        // from the current position in the broadcast channel.

        // Stream live output from broadcast channel — blocks until sender is dropped
        // or shutdown signal is received.
        loop {
            // Use a short timeout to periodically check the shutdown flag.
            // This avoids blocking forever on recv() when broadcast senders
            // are still alive (held by SSE handler session clones).
            match tokio::time::timeout(
                std::time::Duration::from_millis(100),
                rx.recv(),
            ).await {
                Ok(Ok(message)) => {
                    yield sse_event!("message", &serde_json::json!({
                        "type": "output",
                        "data": message
                    }));
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(n))) => {
                    tracing::warn!("SSE subscriber lagged behind by {} messages, skipping", n);
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                Err(_) => {
                    // Timeout — check shutdown flag
                    if shutdown_flag.load(std::sync::atomic::Ordering::SeqCst) {
                        tracing::info!("SSE stream for session {} exiting due to shutdown", session_id);
                        break;
                    }
                }
            }
        }

        // Session complete signal
        yield sse_event!("complete", &serde_json::json!({
            "id": session_id,
            "status": status
        }));
    };

    Sse::new(event_stream)
        .keep_alive(KeepAlive::default().interval(std::time::Duration::from_secs(4)))
        .into_response()
}

/// Cancel a running benchmark.
pub async fn cancel_benchmark(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Json<CancelResponse> {
    let cancelled = state.service.cancel_session(&id);
    let response = if cancelled {
        CancelResponse {
            status: "cancelled".to_string(),
            message: "Benchmark run cancelled".to_string(),
        }
    } else {
        CancelResponse {
            status: "error".to_string(),
            message: "Could not cancel - session not found or not running".to_string(),
        }
    };
    Json(response)
}

/// API endpoint to get active runs count.
pub async fn get_active_runs(Extension(state): Extension<AppState>) -> Json<ActiveRunsResponse> {
    let count = state.service.get_active_session_count();
    Json(ActiveRunsResponse { count })
}

/// API endpoint to get active benchmark sessions.
pub async fn get_active_sessions(
    Extension(state): Extension<AppState>,
) -> Json<ActiveSessionsResponse> {
    let sessions = state.service.get_active_sessions();
    let session_maps: Vec<HashMap<String, String>> = sessions
        .iter()
        .map(|s| {
            let mut map = HashMap::new();
            map.insert("id".to_string(), s.id.clone());
            map.insert("status".to_string(), s.status.to_string());
            map.insert("agent".to_string(), s.agent_name.clone());
            map.insert("language".to_string(), s.language());
            map.insert("exercise".to_string(), s.exercise_name.clone().unwrap_or_default());
            map.insert("progress".to_string(), s.progress_display());
            map.insert("started_at".to_string(), s.started_at.map(|d| d.to_string()).unwrap_or_default());
            map
        })
        .collect();
    Json(ActiveSessionsResponse { sessions: session_maps })
}

/// API endpoint for completeness info: which agent-model combos have all exercises.
pub async fn get_completeness(
    Extension(state): Extension<AppState>,
    Query(params): Query<QuickParam>,
) -> Json<serde_json::Value> {
    let quick_only = params.quick.is_some_and(|q| q);
    let info = state.service.get_completeness_info(quick_only);
    Json(serde_json::to_value(&info.first().unwrap_or(&crate::services::result_service::CompletenessInfo {
        total_exercises: 0,
        complete_keys: vec![],
    })).unwrap_or_default())
}

/// API endpoint to fetch available models from the inference endpoint.
pub async fn fetch_models_endpoint(Extension(state): Extension<AppState>) -> Json<Vec<String>> {
    match state.service.fetch_models().await {
        Ok(models) => {
            tracing::info!("Fetched {} models from inference endpoint", models.len());
            Json(models)
        }
        Err(e) => {
            tracing::warn!("Failed to fetch models from inference endpoint: {}", e);
            // Fallback to cached models from result files
            let cached = state.service.get_models();
            tracing::info!("Falling back to {} cached models", cached.len());
            Json(cached)
        }
    }
}

/// Test page.
pub async fn test_page(
    Extension(templates): Extension<TemplateEngine>,
) -> impl IntoResponse {
    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Test");
    ctx.insert("now", &chrono::Utc::now().to_string());
    axum::response::Html(templates.render("test.tera", &ctx))
}

// =============================================================================
// Router
// =============================================================================

/// Register benchmark routes.
pub fn register(app: Router<()>) -> Router<()> {
    app.route("/", get(dashboard))
        .route("/run", get(run_form))
        .route("/benchmark/{id}", get(view_benchmark))
        .route("/test", get(test_page))
        .route("/api/benchmark/{id}/status", get(get_status))
        .route("/api/benchmark/{id}/stream", get(stream_output))
        .route("/api/benchmark/{id}/cancel", post(cancel_benchmark))
        .route("/api/active-runs", get(get_active_runs))
        .route("/api/active-sessions", get(get_active_sessions))
        .route("/api/models", get(fetch_models_endpoint))
        .route("/api/dashboard/completeness", get(get_completeness))
}
