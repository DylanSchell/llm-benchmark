//! ResultService - mirrors Java ResultService.java
//! Service for reading and managing benchmark results.
//! Caches all results in memory on startup for fast access.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use tracing::info;

mod load;
mod query;
mod scoring;
mod statistics;
mod trace;

// =============================================================================
// Cached result data for fast in-memory access.
/// Optional result filters; `None` or empty matches everything for that dimension.
pub struct ResultFilter<'a> {
    pub language: Option<&'a str>,
    pub agent: Option<&'a str>,
    pub model: Option<&'a str>,
    pub exercise: Option<&'a str>,
}

/// Statistics rolled up over one group:
/// `(total, success, duration_ms, input, output, cached_input, uncached_input)`.
type GroupStats = (i32, i32, f64, u64, u64, u64, u64);

/// [`GroupStats`] keyed by the group value: language, agent, model, or category.
type GroupStatsMap = HashMap<String, GroupStats>;

/// Cached result data for fast in-memory access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResult {
    pub filename: String,
    pub directory: String,
    pub exercise: String,
    pub path: String,
    pub timestamp: Option<String>,
    pub agent: String,
    pub language: String,
    /// Benchmark category (e.g. `polyglot`, `aoc2015`). Defaults to `polyglot`.
    pub category: String,
    pub model: String,
    pub total_exercises: i32,
    pub successful: i32,
    pub failed: i32,
    pub success_rate: String,
    pub results: Vec<HashMap<String, String>>,
    pub trace_path: Option<String>,
    pub has_trace_file: bool,
    // Token tracking fields
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cached_input_tokens: u64,
    #[serde(default)]
    pub uncached_input_tokens: u64,
    // Character counts (parsed from trace message text at load time)
    #[serde(default)]
    pub input_chars: u64,
    #[serde(default)]
    pub output_chars: u64,
    // Turn and tool call counts (parsed from trace file at load time)
    #[serde(default)]
    pub turn_count: u64,
    #[serde(default)]
    pub tool_call_count: u64,
    // Timestamps for wall-clock computation
    #[serde(default)]
    pub start_time: String,
    #[serde(default)]
    pub end_time: String,
}

/// Result of listing individual results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndividualResult {
    pub filename: String,
    pub detail_url: String,
    pub trace_url: String,
    pub path: String,
    pub agent: String,
    pub language: String,
    /// Benchmark category (e.g. `polyglot`, `aoc2015`).
    pub category: String,
    pub model: String,
    pub exercise: String,
    pub success: bool,
    /// UTC ISO 8601 timestamp (e.g. "2024-01-01T00:00:00+00:00").
    /// Client-side JS converts to local timezone for display.
    pub timestamp: Option<String>,
    /// Unix epoch seconds for client-side sorting and display.
    pub timestamp_epoch: Option<f64>,
    pub has_trace_file: bool,
    pub duration: Option<String>,
    pub sort_duration: Option<f64>,
    // Token tracking fields
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cached_input_tokens: u64,
    #[serde(default)]
    pub uncached_input_tokens: u64,
    pub total_tokens: u64,
    // Character counts (parsed from trace message text)
    #[serde(default)]
    pub input_chars: u64,
    #[serde(default)]
    pub output_chars: u64,
    // Calculated field: tokens per second
    pub tokens_per_sec: Option<f64>,
    // Turn and tool call counts (from trace file)
    #[serde(default)]
    pub turn_count: u64,
    #[serde(default)]
    pub tool_call_count: u64,
    // Exercise count fields (from CachedResult)
    pub total_exercises: i32,
    pub successful: i32,
    // Scoring fields
    #[serde(default)]
    pub success_rate: f64,
    #[serde(default)]
    pub speed_score: f64,
    #[serde(default)]
    pub token_score: f64,
    #[serde(default)]
    pub composite_score: f64,
}

/// Statistics item for template rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatItem {
    pub name: String,
    pub total: i32,
    pub success: i32,
    pub success_rate_formatted: String,
    pub total_duration: f64,
    pub total_duration_formatted: String,
    // Wall-clock stats
    #[serde(default)]
    pub wall_clock_duration: f64,
    #[serde(default)]
    pub wall_clock_duration_formatted: String,
    // Token statistics
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub uncached_tokens: u64,
    // For model_stats: separate agent and model for URL construction
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    // Calculated: average tokens per second
    pub avg_tokens_per_sec: Option<f64>,
}

/// Aggregate statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistics {
    pub total_runs: i32,
    pub total_exercises: i32,
    pub successful_exercises: i32,
    pub success_rate: f64,
    pub success_rate_formatted: String,
    pub total_duration: f64,
    pub total_duration_formatted: String,
    // Wall-clock stats (merged overlapping intervals)
    pub wall_clock_duration: f64,
    pub wall_clock_duration_formatted: String,
    // For results page compatibility
    pub total_results: i32,
    pub successful_results: i32,
    pub language_stats: Vec<StatItem>,
    pub agent_stats: Vec<StatItem>,
    pub model_stats: Vec<StatItem>,
    // Per-category aggregation (e.g. polyglot, aoc2015)
    #[serde(default)]
    pub category_stats: Vec<StatItem>,
    // Token statistics
    #[serde(default)]
    pub total_input_tokens: u64,
    #[serde(default)]
    pub total_output_tokens: u64,
    #[serde(default)]
    pub total_cached_tokens: u64,
    #[serde(default)]
    pub total_uncached_tokens: u64,
    pub token_display: String,
}

/// Loading status information
#[derive(Debug, Clone, Serialize)]
pub struct LoadingStatus {
    pub loaded: bool,
    pub result_count: usize,
}

/// Service for reading and managing benchmark results.
#[derive(Debug, Clone)]
pub struct ResultService {
    results_dir: PathBuf,
    cached_results: Arc<RwLock<HashMap<String, CachedResult>>>,
    cached_models: Arc<RwLock<Vec<String>>>,
    /// Flag indicating if initial cache loading is complete
    loaded: Arc<AtomicBool>,
    /// Total number of results in cache (for progress reporting)
    result_count: Arc<AtomicUsize>,
    /// Timestamp of last full cache refresh (ms since epoch).
    last_refresh: Arc<std::sync::Mutex<u64>>,
}

impl ResultService {
    /// Create a new ResultService and start loading results in background.
    pub fn new(results_dir: PathBuf) -> Self {
        let loaded = Arc::new(AtomicBool::new(false));
        let result_count = Arc::new(AtomicUsize::new(0));

        let service = Self {
            results_dir: results_dir.clone(),
            cached_results: Arc::new(RwLock::new(HashMap::new())),
            cached_models: Arc::new(RwLock::new(Vec::new())),
            loaded: loaded.clone(),
            result_count: result_count.clone(),
            last_refresh: Arc::new(std::sync::Mutex::new(0)),
        };

        // Start background loading task
        let cached_results = service.cached_results.clone();
        let cached_models = service.cached_models.clone();
        let results_dir = service.results_dir.clone();
        let loaded_flag = loaded.clone();
        let count_flag = result_count.clone();

        tokio::task::spawn_blocking(move || {
            info!(
                "Starting background loading of benchmark results from: {}",
                results_dir.display()
            );

            // Perform the actual loading (reusing existing logic)
            let temp_service = ResultService {
                results_dir,
                cached_results: cached_results.clone(),
                cached_models: cached_models.clone(),
                loaded: loaded_flag.clone(),
                result_count: count_flag.clone(),
                last_refresh: Arc::new(std::sync::Mutex::new(0)),
            };

            temp_service.load_all_results();

            // Mark loading as complete
            loaded_flag.store(true, Ordering::SeqCst);
            info!("Background result loading complete");
        });

        info!("ResultService initialized - results will be available shortly");
        service
    }
}

/// Completeness info for dashboard filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletenessInfo {
    /// Total expected exercise count (full or quick-bench).
    pub total_exercises: usize,
    /// List of "agent - model" keys that have results for ALL exercises.
    pub complete_keys: Vec<String>,
}

/// A scored individual result.
#[derive(Debug, Serialize, Clone)]
pub struct ScoredResult {
    pub agent: String,
    pub model: String,
    pub language: String,
    pub exercise: String,
    pub success: bool,
    pub successful: i32,
    pub success_rate: f64,
    pub speed_score: f64,
    pub token_score: f64,
    pub composite_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    pub output_tokens: u64,
    pub input_chars: u64,
    pub output_chars: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_sec: Option<f64>,
    pub detail_url: String,
}

/// Aggregated model score.
#[derive(Debug, Serialize, Clone)]
pub struct ModelScore {
    pub name: String,
    pub avg_composite_score: f64,
    pub avg_success_rate: f64,
    pub avg_speed_score: f64,
    pub avg_token_score: f64,
    pub total_tokens: u64,
    pub total_chars: u64,
    pub total_runs: u32,
}

#[cfg(test)]
mod tests {
    use super::trace::parse_chars_from_trace;
    use super::*;

    fn write_trace(content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("trace_test_{}.jsonl", std::process::id()));
        std::fs::write(&path, content).expect("write test trace");
        path
    }

    #[test]
    fn parses_pi_and_claude_chars() {
        let pi = r#"{"type":"session","id":"s1"}
{"type":"message","message":{"role":"user","content":[{"type":"text","text":"hello"},{"type":"text","text":" world"}]}}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"hi there"},{"type":"toolCall","name":"read"}]}}
"#;
        let p = write_trace(pi);
        // input = "hello" + " world" = 11; output = "hi there" = 8
        assert_eq!(parse_chars_from_trace(&p), (11, 8));
        std::fs::remove_file(&p).ok();

        let claude = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"ok"}]}}
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"abc"}]}}
{"type":"custom_message","content":[{"type":"text","text":"ignored"}]}
"#;
        let p2 = write_trace(claude);
        assert_eq!(parse_chars_from_trace(&p2), (3, 2));
        std::fs::remove_file(&p2).ok();
    }

    #[test]
    fn missing_trace_returns_zero() {
        assert_eq!(
            parse_chars_from_trace(std::path::Path::new("/nonexistent/xyz.jsonl")),
            (0, 0)
        );
    }

    #[test]
    fn derive_run_directory_handles_category_nesting() {
        let base = std::path::Path::new("/tmp/results");
        // Top-level result: {agent}-{model}/result_*.json
        let top = base.join("pi-sonnet").join("result_pi_java_series.json");
        assert_eq!(ResultService::derive_run_directory(&top), "pi-sonnet");
        // Category-namespaced result: {agent}-{model}/aoc2015/result_*.json
        let nested = base
            .join("pi-sonnet")
            .join("aoc2015")
            .join("result_pi_aoc2015_day07.json");
        assert_eq!(ResultService::derive_run_directory(&nested), "pi-sonnet");
    }

    #[tokio::test]
    async fn recursive_loader_discovers_category_nested_results() {
        let dir = std::env::temp_dir().join(format!("rs-cat-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let agent_dir = dir.join("pi-sonnet");
        let category_dir = agent_dir.join("aoc2015");
        std::fs::create_dir_all(&category_dir).unwrap();

        // A valid AoC result JSON.
        let json = r#"{
            "exerciseName": "day07",
            "language": "aoc2015",
            "success": true,
            "exitCode": 0,
            "output": "",
            "duration": 4.5,
            "startTime": "2026-09-14T11:36:50.836231+00:00",
            "endTime": "2026-09-14T11:36:55.752601+00:00",
            "category": "aoc2015",
            "model": "sonnet"
        }"#;
        std::fs::write(category_dir.join("result_pi_aoc2015_day07.json"), json).unwrap();

        let service = ResultService::new(dir.clone());
        service.load_all_results();
        let count = service.result_count();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            count, 1,
            "recursive loader must find category-nested results"
        );
    }
}
