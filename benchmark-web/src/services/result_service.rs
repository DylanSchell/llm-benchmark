//! ResultService - mirrors Java ResultService.java
//! Service for reading and managing benchmark results.
//! Caches all results in memory on startup for fast access.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

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
    pub model: String,
    pub total_exercises: i32,
    pub successful: i32,
    pub failed: i32,
    pub success_rate: String,
    pub results: Vec<HashMap<String, String>>,
    pub trace_path: Option<String>,
    pub has_trace_file: bool,
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
    // For results page compatibility
    pub total_results: i32,
    pub successful_results: i32,
    pub language_stats: Vec<StatItem>,
    pub agent_stats: Vec<StatItem>,
    pub model_stats: Vec<StatItem>,
}

/// Service for reading and managing benchmark results.
#[derive(Debug, Clone)]
pub struct ResultService {
    results_dir: PathBuf,
    cached_results: Arc<RwLock<HashMap<String, CachedResult>>>,
    cached_models: Arc<RwLock<Vec<String>>>,
}

impl ResultService {
    /// Create a new ResultService.
    pub fn new(results_dir: PathBuf) -> Self {
        let service = Self {
            results_dir,
            cached_results: Arc::new(RwLock::new(HashMap::new())),
            cached_models: Arc::new(RwLock::new(Vec::new())),
        };
        service.load_all_results();
        service
    }

    /// Load all result files into the in-memory cache.
    pub fn load_all_results(&self) {
        info!("Loading all results into cache from: {}", self.results_dir.display());

        let mut cached = self.cached_results.write().unwrap();
        let mut models = self.cached_models.write().unwrap();

        cached.clear();
        models.clear();

        if !self.results_dir.exists() {
            warn!("Results directory does not exist: {}", self.results_dir.display());
            return;
        }

        let mut count = 0;
        let mut error_count = 0;

        // Walk the results directory for result_*.json files
        if let Ok(entries) = fs::read_dir(&self.results_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                // Walk subdirectories for result files
                if let Ok(sub_entries) = fs::read_dir(&path) {
                    for sub_entry in sub_entries.flatten() {
                        let file_path = sub_entry.path();
                        if !file_path.is_file() {
                            continue;
                        }
                        let filename = file_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();

                        if !filename.starts_with("result_") || !filename.ends_with(".json") {
                            continue;
                        }

                        match Self::load_cached_result(&file_path) {
                            Ok(Some(cached_result)) => {
                                let cache_key = format!(
                                    "{}/{}/{}",
                                    cached_result.directory, cached_result.language, cached_result.exercise
                                );
                                cached.insert(cache_key.clone(), cached_result.clone());
                                models.push(cached_result.model.clone());
                                count += 1;
                            }
                            Ok(None) => {}
                            Err(e) => {
                                warn!("Failed to load result file {}: {}", file_path.display(), e);
                                error_count += 1;
                            }
                        }
                    }
                }
            }
        }

        models.sort();
        models.dedup();

        info!(
            "Loaded {} cached result files into cache ({} errors)",
            count, error_count
        );
        info!("Cached models: {:?}", models);
    }

    /// Load a single result file into a CachedResult.
    fn load_cached_result(file_path: &Path) -> Result<Option<CachedResult>> {
        let content = fs::read_to_string(file_path)?;
        let value: serde_json::Value = serde_json::from_str(&content)?;

        // Validate this is an individual exercise result
        if value.get("exerciseName").is_none() || value.get("language").is_none() {
            return Ok(None);
        }

        let filename = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let directory = file_path
            .parent()
            .and_then(|p| p.file_name())
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Extract fields
        let model = value
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or(&directory)
            .to_string();

        let timestamp = value
            .get("endTime")
            .or_else(|| value.get("timestamp"))
            .and_then(|v| {
                // Try string first (RFC3339), then number (Unix epoch)
                // Store as raw UTC ISO 8601 — timezone conversion is done client-side
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else if let Some(n) = v.as_f64() {
                    // Convert Unix epoch to UTC ISO 8601
                    let secs = n as i64;
                    let nanos = ((n - secs as f64) * 1_000_000_000.0) as u32;
                    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nanos)
                        .map(|dt| dt.to_rfc3339())
                } else {
                    None
                }
            });

        let agent = value
            .get("agent")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Derive from filename: result_<agent>_<lang>_<exercise>.json
                if filename.starts_with("result_") {
                    let without_prefix = &filename[7..];
                    without_prefix
                        .split('_')
                        .next()
                        .unwrap_or("unknown")
                        .to_string()
                } else {
                    "unknown".to_string()
                }
            });

        let language = value
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let exercise = value
            .get("exerciseName")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let success = value
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let duration = value
            .get("duration")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "0".to_string());

        let output = value
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Build results list
        let mut single_result = HashMap::new();
        single_result.insert("language".to_string(), language.clone());
        single_result.insert("exercise".to_string(), exercise.clone());
        single_result.insert("success".to_string(), success.to_string());
        single_result.insert("duration".to_string(), duration.clone());
        single_result.insert("output".to_string(), output);

        let total_exercises = 1;
        let successful = if success { 1 } else { 0 };
        let failed = if success { 0 } else { 1 };
        let success_rate = if successful > 0 {
            "100.0%".to_string()
        } else {
            "0.0%".to_string()
        };

        // Check for existing trace files on disk (embedded traces are legacy and ignored)
        let trace_prefix = Self::derive_trace_prefix(&filename);
        let trace_path = if let Some(parent) = file_path.parent() {
            let jsonl_path = parent.join(format!("trace_{}.jsonl", trace_prefix));
            if jsonl_path.exists() {
                Some(jsonl_path.to_string_lossy().to_string())
            } else {
                let html_path = parent.join(format!("trace_{}.html", trace_prefix));
                if html_path.exists() {
                    Some(html_path.to_string_lossy().to_string())
                } else {
                    None
                }
            }
        } else {
            None
        };

        Ok(Some(CachedResult {
            filename,
            directory,
            exercise,
            path: file_path.to_string_lossy().to_string(),
            timestamp,
            agent,
            language,
            model,
            total_exercises,
            successful,
            failed,
            success_rate,
            results: vec![single_result],
            trace_path: trace_path.clone(),
            has_trace_file: trace_path.is_some(),
        }))
    }

    /// Derive trace prefix from result filename.
    fn derive_trace_prefix(filename: &str) -> String {
        if filename.starts_with("result_") {
            let without_prefix = &filename[7..]; // Remove "result_"
            let without_ext = without_prefix.trim_end_matches(".json");
            // Remove agent prefix: <agent>_<lang>_<exercise> -> <lang>_<exercise>
            if let Some(first_underscore) = without_ext.find('_') {
                without_ext[first_underscore + 1..].to_string()
            } else {
                without_ext.to_string()
            }
        } else {
            filename.trim_end_matches(".json").to_string()
        }
    }

    /// Check if a cached result matches the filter criteria.
    fn matches_filter(
        cached_lang: &str,
        filter_lang: Option<&str>,
        cached_agent: &str,
        filter_agent: Option<&str>,
        cached_model: &str,
        filter_model: Option<&str>,
        cached_exercise: &str,
        filter_exercise: Option<&str>,
    ) -> bool {
        // Treat empty string as "no filter" (match all)
        let matches_language = filter_lang.map_or(true, |f| !f.is_empty() && cached_lang == f);
        let matches_agent = filter_agent.map_or(true, |f| !f.is_empty() && cached_agent == f);
        let matches_model = filter_model.map_or(true, |f| !f.is_empty() && cached_model == f);
        let matches_exercise = filter_exercise.map_or(true, |f| !f.is_empty() && cached_exercise == f);
        matches_language && matches_agent && matches_model && matches_exercise
    }

    /// Convert a CachedResult to a metadata map.
    fn to_metadata_map(cached: &CachedResult) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("filename".to_string(), cached.filename.clone());
        metadata.insert("directory".to_string(), cached.directory.clone());
        metadata.insert(
            "detailUrl".to_string(),
            format!(
                "/results/{}/{}/{}/{}",
                cached.agent, cached.directory, cached.language, cached.exercise
            ),
        );
        metadata.insert(
            "traceUrl".to_string(),
            format!(
                "/results/{}/{}/{}/{}/trace",
                cached.agent, cached.directory, cached.language, cached.exercise
            ),
        );
        metadata.insert("path".to_string(), cached.path.clone());
        metadata.insert(
            "timestamp".to_string(),
            cached.timestamp.clone().unwrap_or_default(),
        );
        metadata.insert("agent".to_string(), cached.agent.clone());
        metadata.insert("language".to_string(), cached.language.clone());
        metadata.insert("model".to_string(), cached.model.clone());
        metadata.insert(
            "total_exercises".to_string(),
            cached.total_exercises.to_string(),
        );
        metadata.insert("successful".to_string(), cached.successful.to_string());
        metadata.insert("failed".to_string(), cached.failed.to_string());
        metadata.insert("success_rate".to_string(), cached.success_rate.clone());
        if let Some(ref trace_path) = cached.trace_path {
            metadata.insert("tracePath".to_string(), trace_path.clone());
        }
        metadata
    }

    /// Get all model names.
    pub fn get_models(&self) -> Vec<String> {
        let models = self.cached_models.read().unwrap();
        let mut unique: Vec<String> = models.iter().cloned().collect();
        unique.sort();
        unique.dedup();
        unique
    }

    /// Get all unique languages.
    pub fn get_languages(&self) -> Vec<String> {
        let cached = self.cached_results.read().unwrap();
        let mut languages: Vec<String> = Vec::new();
        for cached_result in cached.values() {
            if !cached_result.language.is_empty() {
                languages.push(cached_result.language.clone());
            }
        }
        languages.sort();
        languages.dedup();
        languages
    }

    /// Get all exercise names, optionally filtered by language.
    pub fn get_exercises(&self, language: Option<&str>) -> Vec<String> {
        let cached = self.cached_results.read().unwrap();
        let mut exercises: Vec<String> = Vec::new();
        for cached_result in cached.values() {
            // Treat empty string as "no filter" (match all)
            let matches_language = language.map_or(true, |l| !l.is_empty() && cached_result.language == *l);
            if matches_language && !cached_result.exercise.is_empty() {
                exercises.push(cached_result.exercise.clone());
            }
        }
        exercises.sort();
        exercises.dedup();
        exercises
    }

    /// Check if an exercise is part of quick bench.
    fn is_quick_bench_exercise(language: &str, exercise: &str) -> bool {
        let quick_exercises = benchmark_types::config::QuickBenchConfig::get_exercises_for_language(language);
        quick_exercises.contains(&exercise.to_string())
    }

    /// List individual results with filtering.
    pub fn list_individual_results(
        &self,
        language: Option<&str>,
        agent: Option<&str>,
        model: Option<&str>,
        exercise: Option<&str>,
        quick_only: bool,
    ) -> Vec<IndividualResult> {
        let cached = self.cached_results.read().unwrap();
        let mut results: Vec<IndividualResult> = Vec::new();

        for cached_result in cached.values() {
            // Apply quick bench filter
            if quick_only {
                let is_quick = Self::is_quick_bench_exercise(&cached_result.language, &cached_result.exercise);
                if !is_quick {
                    continue;
                }
            }

            if Self::matches_filter(
                &cached_result.language,
                language,
                &cached_result.agent,
                agent,
                &cached_result.model,
                model,
                &cached_result.exercise,
                exercise,
            ) {
                // Extract Unix epoch from the raw UTC timestamp for client-side display/sorting
                let timestamp_epoch = cached_result.timestamp.as_ref().and_then(|ts| {
                    chrono::DateTime::parse_from_rfc3339(ts)
                        .ok()
                        .map(|dt| dt.timestamp() as f64)
                });

                results.push(IndividualResult {
                    filename: cached_result.filename.clone(),
                    detail_url: format!(
                        "/results/{}/{}/{}/{}",
                        cached_result.agent, cached_result.directory, cached_result.language, cached_result.exercise
                    ),
                    trace_url: format!(
                        "/results/{}/{}/{}/{}/trace",
                        cached_result.agent, cached_result.directory, cached_result.language, cached_result.exercise
                    ),
                    path: cached_result.path.clone(),
                    agent: cached_result.agent.clone(),
                    language: cached_result.language.clone(),
                    model: cached_result.model.clone(),
                    exercise: cached_result.exercise.clone(),
                    success: cached_result.successful > 0,
                    timestamp: cached_result.timestamp.clone(),
                    timestamp_epoch,
                    has_trace_file: cached_result.has_trace_file,
                    duration: None,
                    sort_duration: None,
                });
            }
        }

        // Sort by timestamp descending (use epoch for reliable numeric sort)
        results.sort_by(|a, b| {
            let epoch_a = a.timestamp_epoch.unwrap_or(0.0);
            let epoch_b = b.timestamp_epoch.unwrap_or(0.0);
            epoch_b.partial_cmp(&epoch_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Extract duration from cached results
        for result in &mut results {
            // Look up the cached result to get duration
            // Cache key format: directory/language/exercise
            // Extract directory from path (e.g., "/path/to/pi-qwen35-122b/result_pi_java_exercise.json" -> "pi-qwen35-122b")
            let directory = std::path::Path::new(&result.path)
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let cache_key = format!("{}/{}/{}", directory, result.language, result.exercise);
            if let Some(cached) = self.cached_results.read().unwrap().get(&cache_key) {
                if let Some(single_result) = cached.results.first() {
                    if let Some(dur_str) = single_result.get("duration") {
                        if let Ok(dur) = dur_str.parse::<f64>() {
                            result.duration = Some(Self::format_duration(dur));
                            result.sort_duration = Some(dur);
                        } else {
                            result.duration = Some(dur_str.clone());
                        }
                    }
                }
            }
        }

        results
    }

    /// Get a result by its cache key (directory/language/exercise).
    pub fn get_result_by_key(&self, key: &str) -> Option<HashMap<String, String>> {
        let cached = self.cached_results.read().unwrap();
        cached.get(key).map(Self::to_metadata_map)
    }

    /// Get statistics.
    pub fn get_statistics(
        &self,
        language: Option<&str>,
        agent: Option<&str>,
        model: Option<&str>,
        exercise: Option<&str>,
        quick_only: bool,
    ) -> Statistics {
        let cached = self.cached_results.read().unwrap();
        let mut total_runs: i32 = 0;
        let mut total_exercises: i32 = 0;
        let mut successful_exercises: i32 = 0;
        let mut total_duration: f64 = 0.0;
        let mut by_language: HashMap<String, (i32, i32, f64)> = HashMap::new();
        let mut by_agent: HashMap<String, (i32, i32, f64)> = HashMap::new();
        let mut by_model: HashMap<String, (i32, i32, f64)> = HashMap::new();

        for cached_result in cached.values() {
            if !Self::matches_filter(
                &cached_result.language,
                language,
                &cached_result.agent,
                agent,
                &cached_result.model,
                model,
                &cached_result.exercise,
                exercise,
            ) {
                continue;
            }

            // Apply quick bench filter
            if quick_only && !Self::is_quick_bench_exercise(&cached_result.language, &cached_result.exercise) {
                continue;
            }

            total_runs += 1;
            total_exercises += cached_result.total_exercises;
            successful_exercises += cached_result.successful;

            // Parse duration from cached results and accumulate
            let mut entry_duration: f64 = 0.0;
            if let Some(single_result) = cached_result.results.first() {
                if let Some(dur_str) = single_result.get("duration") {
                    if let Some(dur) = Self::parse_duration(dur_str) {
                        entry_duration = dur;
                        total_duration += dur;
                    }
                }
            }

            // By language - track (total, success, duration)
            *by_language
                .entry(cached_result.language.clone())
                .or_insert((0, 0, 0.0)) = {
                let (t, s, d) = by_language.get(&cached_result.language).copied().unwrap_or((0, 0, 0.0));
                (t + cached_result.total_exercises, s + cached_result.successful, d + entry_duration)
            };

            // By agent - track (total, success, duration)
            *by_agent
                .entry(cached_result.agent.clone())
                .or_insert((0, 0, 0.0)) = {
                let (t, s, d) = by_agent.get(&cached_result.agent).copied().unwrap_or((0, 0, 0.0));
                (t + cached_result.total_exercises, s + cached_result.successful, d + entry_duration)
            };

            // By model - track (total, success, duration) using agent + model combination
            let model_key = format!("{} - {}", cached_result.agent, cached_result.model);
            *by_model
                .entry(model_key)
                .or_insert((0, 0, 0.0)) = {
                let (t, s, d) = by_model.get(&model_key).copied().unwrap_or((0, 0, 0.0));
                (t + cached_result.total_exercises, s + cached_result.successful, d + entry_duration)
            };
        }

        let success_rate = if total_exercises > 0 {
            (successful_exercises as f64 / total_exercises as f64) * 100.0
        } else {
            0.0
        };
        let success_rate_formatted = format!("{:.1}", success_rate);

        // Convert HashMaps to sorted Vec<StatItem> with duration
        let mut language_stats: Vec<StatItem> = by_language
            .iter()
            .map(|(name, (total, success, duration))| {
                let rate = if *total > 0 { (*success as f64 / *total as f64) * 100.0 } else { 0.0 };
                StatItem {
                    name: name.clone(),
                    total: *total,
                    success: *success,
                    success_rate_formatted: format!("{:.1}", rate),
                    total_duration: *duration,
                    total_duration_formatted: Self::format_duration(*duration),
                }
            })
            .collect();
        language_stats.sort_by(|a, b| b.total.cmp(&a.total));

        let mut agent_stats: Vec<StatItem> = by_agent
            .iter()
            .map(|(name, (total, success, duration))| {
                let rate = if *total > 0 { (*success as f64 / *total as f64) * 100.0 } else { 0.0 };
                StatItem {
                    name: name.clone(),
                    total: *total,
                    success: *success,
                    success_rate_formatted: format!("{:.1}", rate),
                    total_duration: *duration,
                    total_duration_formatted: Self::format_duration(*duration),
                }
            })
            .collect();
        agent_stats.sort_by(|a, b| b.total.cmp(&a.total));

        let mut model_stats: Vec<StatItem> = by_model
            .iter()
            .map(|(name, (total, success, duration))| {
                let rate = if *total > 0 { (*success as f64 / *total as f64) * 100.0 } else { 0.0 };
                StatItem {
                    name: name.clone(),
                    total: *total,
                    success: *success,
                    success_rate_formatted: format!("{:.1}", rate),
                    total_duration: *duration,
                    total_duration_formatted: Self::format_duration(*duration),
                }
            })
            .collect();
        model_stats.sort_by(|a, b| {
            // Sort by total descending, then by name ascending for consistency
            b.total.cmp(&a.total).then_with(|| a.name.cmp(&b.name))
        });

        Statistics {
            total_runs,
            total_exercises,
            successful_exercises,
            success_rate,
            success_rate_formatted,
            total_duration,
            total_duration_formatted: Self::format_duration(total_duration),
            total_results: total_runs,
            successful_results: successful_exercises,
            language_stats,
            agent_stats,
            model_stats,
        }
    }

    /// Parse duration string to seconds. Handles formats like "10s", "1000ms", "10.5s", "10.5".
    fn parse_duration(dur_str: &str) -> Option<f64> {
        let s = dur_str.trim();
        if s.is_empty() || s == "null" {
            return Some(0.0);
        }
        if s.ends_with("ms") {
            s.trim_end_matches("ms").parse::<f64>().map(|v| v / 1000.0).ok()
        } else if s.ends_with('s') {
            s.trim_end_matches('s').parse::<f64>().ok()
        } else {
            s.parse::<f64>().ok()
        }
    }

    /// Format duration in seconds to human-readable string.
    pub fn format_duration(total_seconds: f64) -> String {
        if total_seconds == 0.0 {
            return "0s".to_string();
        }

        let days = (total_seconds / 86400.0) as i64;
        let hours = ((total_seconds % 86400.0) / 3600.0) as i64;
        let minutes = ((total_seconds % 3600.0) / 60.0) as i64;
        let seconds = (total_seconds % 60.0) as i64;

        let mut parts = Vec::new();
        if days > 0 {
            parts.push(format!("{}d", days));
        }
        if hours > 0 {
            parts.push(format!("{}h", hours));
        }
        if minutes > 0 {
            parts.push(format!("{}m", minutes));
        }
        parts.push(format!("{}s", seconds));

        parts.join(" ")
    }

    /// Refresh the result cache.
    pub fn refresh_cache(&self) {
        self.load_all_results();
    }

    /// Update the in-memory cache with a single new or updated result file.
    /// Reads only that one file from disk and inserts it into the cache,
    /// replacing any existing entry for the same (directory, language, exercise) key.
    /// This is much more efficient than a full cache reload when exactly one result
    /// has been written to disk (e.g., after a benchmark exercise completes).
    pub fn update_single_result(&self, file_path: &Path) {
        info!("Updating single result from file: {}", file_path.display());

        match Self::load_cached_result(file_path) {
            Ok(Some(cached_result)) => {
                let cache_key = format!(
                    "{}/{}/{}",
                    cached_result.directory, cached_result.language, cached_result.exercise
                );
                let mut cached = self.cached_results.write().unwrap();
                let was_present = cached.contains_key(&cache_key);
                cached.insert(cache_key.clone(), cached_result.clone());
                drop(cached);

                // Also ensure the model is in the models list
                let mut models = self.cached_models.write().unwrap();
                if !models.contains(&cached_result.model) {
                    models.push(cached_result.model.clone());
                    models.sort();
                    models.dedup();
                }
                drop(models);

                if was_present {
                    info!("Updated existing cached result: {}", cache_key);
                } else {
                    info!("Inserted new cached result: {}", cache_key);
                }
            }
            Ok(None) => {
                // Not a valid exercise result file (e.g., a summary file), ignore silently
            }
            Err(e) => {
                warn!("Failed to update single result from {}: {}", file_path.display(), e);
            }
        }
    }

    /// Get the results directory.
    pub fn results_dir(&self) -> &Path {
        &self.results_dir
    }

    /// Get the rendered HTML trace for a result.
    /// If an HTML trace file already exists, returns it directly.
    /// Otherwise, if a JSONL trace file exists, generates the HTML via 'pi --export'.
    pub fn get_trace_content(&self, key: &str) -> Result<Option<String>> {
        let cached = self.cached_results.read().unwrap();
        let cached_result = match cached.get(key) {
            Some(c) => c,
            None => {
                info!("TRACE MISS: No cached result for key='{}'", key);
                return Ok(None);
            }
        };

        info!("TRACE HIT: key='{}', filename='{}', path='{}'", key, cached_result.filename, cached_result.path);

        let path_buf = PathBuf::from(&cached_result.path);
        let result_dir = path_buf
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine parent directory"))?;

        // Derive the trace filename prefix from the result filename
        let trace_prefix = Self::derive_trace_prefix(&cached_result.filename);
        info!("TRACE: resultDir='{}', tracePrefix='{}'", result_dir.display(), trace_prefix);

        // Step 1: Check for existing HTML trace
        let html_trace_path = result_dir.join(format!("trace_{}.html", trace_prefix));
        info!("TRACE: checking HTML trace: {} (exists={})", html_trace_path.display(), html_trace_path.exists());
        if html_trace_path.exists() {
            info!("Loading existing HTML trace: {}", html_trace_path.display());
            return Ok(Some(fs::read_to_string(&html_trace_path)?));
        }

        // Step 2: Check for JSONL trace and try to generate HTML
        let jsonl_trace_path = result_dir.join(format!("trace_{}.jsonl", trace_prefix));
        if jsonl_trace_path.exists() {
            info!("Generating HTML trace from JSONL: {}", jsonl_trace_path.display());
            let output = std::process::Command::new("pi")
                .args(&["--export", jsonl_trace_path.to_str().unwrap(), html_trace_path.to_str().unwrap()])
                .output();

            match output {
                Ok(output) => {
                    if output.status.success() && html_trace_path.exists() {
                        info!("Generated HTML trace: {}", html_trace_path.display());
                        return Ok(Some(fs::read_to_string(&html_trace_path)?));
                    } else {
                        warn!("pi --export failed (exit={})", output.status.code().unwrap_or(-1));
                    }
                }
                Err(e) => {
                    warn!("Failed to run pi --export: {}", e);
                }
            }
        }

        Ok(None)
    }

    /// Check if a result file exists and was successful.
    pub fn result_file_success(&self, exercise_name: &str, agent_name: &str, model: &str, language: &str, results_dir: &Path) -> bool {
        let subdir = format!("{}-{}", agent_name, model);
        let result_path = results_dir.join(&subdir).join(format!(
            "result_{}_{}_{}.json",
            agent_name, language, exercise_name
        ));

        if !result_path.exists() {
            return false;
        }

        match fs::read_to_string(&result_path) {
            Ok(content) => {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                    value
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            Err(e) => {
                warn!("Failed to read result file {}: {}", result_path.display(), e);
                false
            }
        }
    }
}
