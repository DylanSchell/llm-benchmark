//! Queue routes - mirrors Java QueueController.java
//! REST API for queue management.

use super::AppState;
use axum::extract::{Path, Query, FromRequest, Request};
use axum::routing::{get, post};
use axum::Router;
use axum::Json;
use axum::Extension;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::queue_item::{BenchmarkQueueItem, QueueItemStatus};

// =============================================================================
// Request/Response types
// =============================================================================

// Custom form extractor that handles repeated fields properly.
// Shared across routes (AoC scheduling also relies on it for repeated `days`).
pub struct FlexibleForm<T>(pub T);

impl<T, S> FromRequest<S> for FlexibleForm<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract form data as raw bytes first
        let body = axum::body::to_bytes(req.into_body(), 1_048_576)
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        
        let body_str = String::from_utf8_lossy(&body);
        
        // Parse manually to handle repeated fields
        let mut pairs: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        
        for pair in body_str.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                let decoded_key = urlencoding::decode(key)
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
                    .into_owned();
                let decoded_value = urlencoding::decode(value)
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
                    .into_owned();
                pairs.entry(decoded_key).or_default().push(decoded_value);
            }
        }
        
        // Convert to URL-encoded format that serde_urlencoded can handle
        let mut flattened: Vec<(String, String)> = Vec::new();
        for (key, values) in pairs.into_iter() {
            if values.len() == 1 {
                flattened.push((key, values[0].clone()));
            } else {
                // For repeated fields, create a JSON array string
                let json_array = serde_json::to_string(&values)
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
                flattened.push((key, json_array));
            }
        }
        
        let serialized = serde_urlencoded::to_string(&flattened)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        
        let request: T = serde_urlencoded::from_str(&serialized)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to deserialize form: {}", e)))?;
        
        Ok(FlexibleForm(request))
    }
}

#[derive(Debug, Deserialize)]
pub struct ScheduleRequest {
    pub agent: String,
    #[serde(deserialize_with = "deserialize_vec_string")]
    pub languages: Vec<String>,
    pub model: String,
    /// Pi thinking level: off, minimal, low, medium, high, xhigh
    #[serde(default)]
    pub thinking_level: Option<String>,
    pub exercise: Option<String>,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub retry: bool,
}

/// Deserialize a form field that may appear multiple times into a Vec<String>.
/// Handles both single values, repeated fields from HTML forms, and JSON arrays.
pub(crate) fn deserialize_vec_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_vec(deserializer)
}

/// Deserialize a form field that may appear multiple times into a Vec<u32>.
/// Used by the AoC scheduling form's repeated `days` field.
pub(crate) fn deserialize_vec_u32<'de, D>(deserializer: D) -> Result<Vec<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_vec(deserializer)
}

/// Generic helper: deserialize a field that may be a single value, a JSON array
/// string (from [`FlexibleForm`]), or a real sequence into a `Vec<T>`.
///
/// Form values arrive as strings, so string elements are parsed with `T::from_str`
/// (which handles numeric types like `u32`) before falling back to `Deserialize`.
fn deserialize_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + std::str::FromStr,
    T::Err: std::fmt::Display,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct VecVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T> Visitor<'de> for VecVisitor<T>
    where
        T: serde::de::DeserializeOwned + std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string, JSON array string, or sequence")
        }

        fn visit_str<E>(self, v: &str) -> Result<Vec<T>, E>
        where
            E: de::Error,
        {
            // Check if it's a JSON array string (from our custom extractor). The
            // array holds raw string elements, so parse each one into `T`.
            if v.starts_with('[') && v.ends_with(']') {
                let raw: Vec<String> = serde_json::from_str(v)
                    .map_err(|e| de::Error::custom(format!("invalid JSON array: {e}")))?;
                raw.into_iter()
                    .map(|s| Self::parse_one(&s))
                    .collect()
            } else {
                Self::parse_one(v).map(|x| vec![x])
            }
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Vec<T>, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(s) = seq.next_element::<T>()? {
                vec.push(s);
            }
            Ok(vec)
        }
    }

    impl<T> VecVisitor<T>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        fn parse_one<E: de::Error>(s: &str) -> Result<T, E> {
            s.parse::<T>().map_err(|e| de::Error::custom(format!("invalid value '{s}': {e}")))
        }
    }

    deserializer.deserialize_any(VecVisitor::<T>(std::marker::PhantomData))
}

fn default_mode() -> String {
    "all".to_string()
}

#[derive(Debug, Serialize)]
pub struct QueueResponse {
    pub items: Vec<HashMap<String, String>>,
    pub pending: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub active_workers: usize,
    pub parallelism_limit: usize,
}

#[derive(Debug, Deserialize)]
pub struct QueryParam {
    pub status: Option<String>,
}

// =============================================================================
// Handlers
// =============================================================================

/// Get the benchmark queue.
pub async fn get_queue(
    Extension(state): Extension<AppState>,
    Query(params): Query<QueryParam>,
) -> Json<QueueResponse> {
    let items = state.service.get_queue_items();
    let pending = items.iter().filter(|i| i.status == QueueItemStatus::PENDING).count();
    let running = items.iter().filter(|i| i.status == QueueItemStatus::RUNNING).count();
    let completed = items.iter().filter(|i| i.status == QueueItemStatus::COMPLETED).count();
    let failed = items.iter().filter(|i| i.status == QueueItemStatus::FAILED).count();
    let cancelled = items.iter().filter(|i| i.status == QueueItemStatus::CANCELLED).count();

    let filtered_items: Vec<HashMap<String, String>> = if let Some(ref status) = params.status {
        items.iter().filter(|i| i.status.to_string() == *status).map(|i| item_to_map(i)).collect()
    } else {
        items.iter().map(|i| item_to_map(i)).collect()
    };

    Json(QueueResponse {
        items: filtered_items,
        pending, running, completed, failed, cancelled,
        active_workers: state.service.get_active_worker_count().await,
        parallelism_limit: state.service.get_parallelism_limit(),
    })
}

/// Schedule a batch of benchmark runs.
/// Execution modes:
/// - "single": exercise param specifies which exercise to run per language
/// - "all": no exercise param — all exercises for selected languages
/// - "quick": special marker — runs a curated list of fast exercises (< 60s each)
/// - "slow": special marker — runs all exercises EXCEPT the quick-bench ones
pub async fn schedule_batch(
    Extension(state): Extension<AppState>,
    FlexibleForm(request): FlexibleForm<ScheduleRequest>,
) -> Json<serde_json::Value> {
    // Resolve exercise parameter based on mode
    let exercise = resolve_exercise_param(&request.mode, &request.exercise);

    // Reference agent ignores model — use "reference" so queue items display
    // correctly and results are saved to the right directory.
    let model = if request.agent == "reference" {
        "reference".to_string()
    } else {
        request.model.clone()
    };

    let items = state.service.schedule_batch_with_retry(
        request.agent.clone(), request.languages.clone(),
        model, request.thinking_level.clone(), exercise, request.retry,
    );

    let items_map: Vec<HashMap<String, String>> = items.iter().map(|i| item_to_map(i)).collect();
    let response = serde_json::json!({
        "status": "scheduled",
        "count": items.len(),
        "items": items_map,
    });
    Json(response)
}

/// Resolves the exercise parameter based on execution mode.
fn resolve_exercise_param(mode: &str, exercise: &Option<String>) -> Option<String> {
    match mode {
        "single" => exercise.clone(),
        "quick" => Some("__quick__".to_string()),
        "slow" => Some("__slow__".to_string()),
        _ => None, // "all" mode — no specific exercise
    }
}

/// Cancel a queue item.
pub async fn cancel_queue_item(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let success = state.service.cancel_queue_item(&id).await;
    if success {
        Json(serde_json::json!({
            "status": "cancelled",
            "itemId": id,
        }))
    } else {
        Json(serde_json::json!({
            "status": "error",
            "message": "Could not cancel - item not found or already completed",
        }))
    }
}

/// Clear pending items from queue.
pub async fn clear_pending_queue(Extension(state): Extension<AppState>) -> Json<serde_json::Value> {
    state.service.clear_pending_queue();
    Json(serde_json::json!({
        "status": "ok",
        "message": "Pending queue items cleared",
    }))
}

/// Clear completed and cancelled items from the queue.
pub async fn clear_completed_and_cancelled(Extension(state): Extension<AppState>) -> Json<serde_json::Value> {
    let removed = state.service.clear_completed_and_cancelled();
    Json(serde_json::json!({
        "status": "ok",
        "message": format!("{} completed/cancelled items cleared", removed),
        "removed": removed,
    }))
}

/// Retry a failed queue item.
pub async fn retry_queue_item(
    Extension(state): Extension<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let result = state.service.retry_queue_item(&id);
    if let Some(new_item) = result {
        Json(serde_json::json!({
            "status": "retried",
            "itemId": new_item.id,
            "message": "Item re-queued for retry",
        }))
    } else {
        Json(serde_json::json!({
            "status": "error",
            "message": "Could not retry - item not found or not in failed state",
        }))
    }
}

/// Helper to convert a queue item to a map.
fn item_to_map(item: &BenchmarkQueueItem) -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("id".to_string(), item.id.clone());
    map.insert("agent_name".to_string(), item.agent_name.clone());
    map.insert("model".to_string(), item.model.clone());
    if let Some(ref tl) = item.thinking_level {
        map.insert("thinking_level".to_string(), tl.clone());
    }
    map.insert("language".to_string(), item.language.clone());
    map.insert("exercise".to_string(), item.exercise.clone());
    map.insert("status".to_string(), item.status.to_string());
    map.insert("session_id".to_string(), item.session_id.clone().unwrap_or_default());
    map.insert("retry".to_string(), item.retry.to_string());
    map
}

// =============================================================================
// Router
// =============================================================================

/// Register queue routes.
pub fn register(app: Router<()>) -> Router<()> {
    app.route("/api/benchmark/queue", get(get_queue))
        .route("/api/benchmark/queue/schedule", post(schedule_batch))
        .route("/api/benchmark/queue/cancel/{id}", post(cancel_queue_item))
        .route("/api/benchmark/queue/clear", post(clear_pending_queue))
        .route("/api/benchmark/queue/clear-terminal", post(clear_completed_and_cancelled))
        .route("/api/benchmark/queue/retry/{id}", post(retry_queue_item))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct DaysReq {
        #[serde(default, deserialize_with = "deserialize_vec_u32")]
        days: Vec<u32>,
    }

    #[derive(Debug, Deserialize)]
    struct LangsReq {
        #[serde(default, deserialize_with = "deserialize_vec_string")]
        langs: Vec<String>,
    }

    fn parse_days(ser: &str) -> Vec<u32> {
        // Mimic FlexibleForm: repeated keys -> JSON array string of raw values.
        let mut pairs: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for pair in ser.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                pairs.entry(k.to_string()).or_default().push(v.to_string());
            }
        }
        let mut flattened: Vec<(String, String)> = Vec::new();
        for (k, vals) in pairs.into_iter() {
            if vals.len() == 1 {
                flattened.push((k, vals[0].clone()));
            } else {
                flattened.push((k, serde_json::to_string(&vals).unwrap()));
            }
        }
        let ser = serde_urlencoded::to_string(&flattened).unwrap();
        let req: DaysReq = serde_urlencoded::from_str(&ser).unwrap();
        req.days
    }

    #[test]
    fn repeated_days_parse_as_u32() {
        assert_eq!(parse_days("days=1&days=2&days=3"), vec![1, 2, 3]);
    }

    #[test]
    fn single_day_parses_as_one_element() {
        assert_eq!(parse_days("days=7"), vec![7]);
    }

    #[test]
    fn empty_days_is_empty_vec() {
        assert_eq!(parse_days("agent=pi"), Vec::<u32>::new());
    }

    #[test]
    fn string_elements_still_work_for_string_vectors() {
        // The generic helper must remain compatible with Vec<String>.
        let ser = "langs=java&langs=python";
        let mut pairs: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for pair in ser.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                pairs.entry(k.to_string()).or_default().push(v.to_string());
            }
        }
        let mut flattened: Vec<(String, String)> = Vec::new();
        for (k, vals) in pairs.into_iter() {
            flattened.push((k, serde_json::to_string(&vals).unwrap()));
        }
        let ser = serde_urlencoded::to_string(&flattened).unwrap();
        let req: LangsReq = serde_urlencoded::from_str(&ser).unwrap();
        assert_eq!(req.langs, vec!["java".to_string(), "python".to_string()]);
    }
}
