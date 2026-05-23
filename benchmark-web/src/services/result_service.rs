//! ResultService - mirrors Java ResultService.java
//! Service for reading and managing benchmark results.
//! Caches all results in memory on startup for fast access.

use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use tracing::{info, warn};

// =============================================================================
// Token parsing models (mirrors benchmark-reporter)
// =============================================================================

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum PiLogEntry {
    #[serde(rename = "message")]
    Message(PiMessage),
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct PiMessage {
    message: Option<PiMessageData>,
}

#[derive(Debug, Deserialize)]
struct PiMessageData {
    usage: Option<PiUsage>,
}

#[derive(Debug, Deserialize)]
struct PiUsage {
    input: Option<i64>,
    output: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum LogEntry {
    #[serde(rename = "assistant")]
    Assistant(AssistantEntry),
    #[serde(rename = "user")]
    User(UserEntry),
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct AssistantEntry {
    message: Option<Message>,
}

#[derive(Debug, Deserialize)]
struct UserEntry {
    message: Option<Message>,
}

#[derive(Debug, Deserialize)]
struct Message {
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: u64,
    output_tokens: u64,
}

/// Calculate tokens from a Pi agent trace file.
/// Returns (input, output, cached, uncached)
fn calculate_pi_tokens(trace_path: &Path) -> (u64, u64, u64, u64) {
    let mut input = 0u64;
    let mut output = 0u64;
    
    if let Ok(file) = File::open(trace_path) {
        for line in BufReader::new(file).lines().flatten() {
            if let Ok(PiLogEntry::Message(msg)) = serde_json::from_str::<PiLogEntry>(line.trim()) {
                if let Some(ref data) = msg.message {
                    if let Some(ref usage) = data.usage {
                        input += usage.input.unwrap_or(0) as u64;
                        output += usage.output.unwrap_or(0) as u64;
                    }
                }
            }
        }
    }
    
    // Pi doesn't track cached/uncached separately
    (input, output, 0, 0)
}

/// Calculate tokens from a Claude agent trace file.
/// Returns (input, output, cached, uncached)
fn calculate_claude_tokens(trace_path: &Path) -> (u64, u64, u64, u64) {
    let mut input = 0u64;
    let mut output = 0u64;
    let mut cached = 0u64;
    let mut uncached = 0u64;
    let mut prev_input: u64 = 0;
    
    if let Ok(file) = File::open(trace_path) {
        for line in BufReader::new(file).lines().flatten() {
            if let Ok(entry) = serde_json::from_str::<LogEntry>(line.trim()) {
                let msg = match &entry {
                    LogEntry::Assistant(a) => a.message.as_ref(),
                    LogEntry::User(u) => u.message.as_ref(),
                    _ => continue,
                };
                
                if let Some(m) = msg {
                    if let Some(ref usage) = m.usage {
                        input += usage.input_tokens;
                        output += usage.output_tokens;
                        
                        // Calculate cached vs uncached based on delta from previous input
                        let new = usage.input_tokens.saturating_sub(prev_input);
                        if new > 0 {
                            uncached += new;
                            cached += usage.input_tokens - new;
                        } else {
                            uncached += usage.input_tokens;
                        }
                        prev_input = usage.input_tokens;
                    }
                }
            }
        }
    }
    
    (input, output, cached, uncached)
}

/// Parse tokens from a trace file based on agent type.
fn parse_tokens_from_trace(trace_path: &Path, agent: &str) -> (u64, u64, u64, u64) {
    if !trace_path.exists() {
        return (0, 0, 0, 0);
    }
    
    if agent.starts_with("pi") {
        calculate_pi_tokens(trace_path)
    } else {
        // Default to Claude format for claude, reference, etc.
        calculate_claude_tokens(trace_path)
    }
}

// =============================================================================
// Cached result data for fast in-memory access.
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
    // Token tracking fields
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cached_input_tokens: u64,
    #[serde(default)]
    pub uncached_input_tokens: u64,
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
        };

        // Start background loading task
        let cached_results = service.cached_results.clone();
        let cached_models = service.cached_models.clone();
        let results_dir = service.results_dir.clone();
        let loaded_flag = loaded.clone();
        let count_flag = result_count.clone();

        thread::spawn(move || {
            info!("Starting background loading of benchmark results from: {}", results_dir.display());
            
            // Perform the actual loading (reusing existing logic)
            let temp_service = ResultService {
                results_dir,
                cached_results: cached_results.clone(),
                cached_models: cached_models.clone(),
                loaded: loaded_flag.clone(),
                result_count: count_flag.clone(),
            };
            
            temp_service.load_all_results();
            
            // Mark loading as complete
            loaded_flag.store(true, Ordering::SeqCst);
            info!("Background result loading complete");
        });

        info!("ResultService initialized - results will be available shortly");
        service
    }

    /// Load all result files into the in-memory cache using parallel processing.
    pub fn load_all_results(&self) {
        info!("Loading all results into cache from: {}", self.results_dir.display());

        if !self.results_dir.exists() {
            warn!("Results directory does not exist: {}", self.results_dir.display());
            return;
        }

        // Collect all result file paths first (this is fast, just directory traversal)
        let mut result_paths: Vec<PathBuf> = Vec::new();

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

                        if filename.starts_with("result_") && filename.ends_with(".json") {
                            result_paths.push(file_path);
                        }
                    }
                }
            }
        }

        let total_files = result_paths.len();
        info!("Found {} result files to process", total_files);

        // Shared collections for errors and skipped files
        let error_count = AtomicUsize::new(0);
        let error_messages: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
        let skipped_count = AtomicUsize::new(0);
        let skipped_files: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));

        // Process files in parallel using rayon
        // Cache key format: {directory}/{language}/{exercise}
        // where directory = {agent}-{model} (internal implementation detail)
        let results: Vec<(String, CachedResult)> = result_paths
            .par_iter()
            .filter_map(|file_path| {
                match Self::load_cached_result(file_path) {
                    Ok(Some(cached_result)) => {
                        // Internal cache key uses directory name; URLs use separate agent/model
                        let cache_key = format!(
                            "{}/{}/{}",
                            cached_result.directory, cached_result.language, cached_result.exercise
                        );
                        Some((cache_key, cached_result))
                    }
                    Ok(None) => {
                        // Not a valid exercise result - log why it was skipped
                        skipped_count.fetch_add(1, Ordering::SeqCst);
                        let file_name = file_path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        let parent = file_path.parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        
                        if let Ok(mut skipped) = skipped_files.write() {
                            skipped.push(format!("{}/{}", parent, file_name));
                        }
                        None
                    }
                    Err(e) => {
                        // Increment error counter and store message
                        error_count.fetch_add(1, Ordering::SeqCst);
                        let error_msg = format!("Failed to load {}: {}", file_path.display(), e);
                        if let Ok(mut errors) = error_messages.write() {
                            errors.push(error_msg);
                        }
                        None
                    }
                }
            })
            .collect();

        let count = results.len();
        let actual_error_count = error_count.load(Ordering::SeqCst);
        let actual_skipped_count = skipped_count.load(Ordering::SeqCst);

        // Summary of what happened
        info!(
            "Parallel load complete: {} files processed, {} loaded successfully, {} skipped (invalid format), {} errors",
            total_files, count, actual_skipped_count, actual_error_count
        );

        // Log skipped files (not valid exercise results)
        if actual_skipped_count > 0 {
            info!("Skipped {} files that are not valid exercise results:", actual_skipped_count);
            if let Ok(skipped) = skipped_files.read() {
                for file_path in skipped.iter().take(10) {
                    // Extract just the relative path from results dir for readability
                    let relative = file_path.strip_prefix(self.results_dir.to_string_lossy().as_ref())
                        .unwrap_or(file_path);
                    info!("  - {}", relative);
                }
                if skipped.len() > 10 {
                    info!("  ... and {} more files", skipped.len() - 10);
                }
            }
        }

        // Log actual errors if any occurred
        if actual_error_count > 0 {
            info!("Encountered {} actual errors while loading result files:", actual_error_count);
            if let Ok(errors) = error_messages.read() {
                for error_msg in errors.iter().take(10) {
                    info!("  - {}", error_msg);
                }
                if errors.len() > 10 {
                    info!("  ... and {} more errors", errors.len() - 10);
                }
            }
        }

        // Now merge results into the shared cache (single-threaded, fast write)
        let mut cached = self.cached_results.write().unwrap();
        let mut models: Vec<String> = Vec::new();

        for (cache_key, cached_result) in results {
            cached.insert(cache_key, cached_result.clone());
            models.push(cached_result.model);
        }

        models.sort();
        models.dedup();

        drop(cached);

        // Update models cache
        let mut models_cache = self.cached_models.write().unwrap();
        *models_cache = models;

        // Update result count
        self.result_count.store(count, Ordering::SeqCst);

        info!(
            "Loaded {} cached result files into cache ({} errors)",
            count, actual_error_count
        );
        info!("Cached models: {:?}", models_cache);
    }

    /// Load a single result file into a CachedResult.
    fn load_cached_result(file_path: &Path) -> Result<Option<CachedResult>> {
        let content = fs::read_to_string(file_path)?;
        
        // Try to deserialize as ExerciseResult first (handles both camelCase and snake_case via serde aliases)
        let exercise_result: benchmark_types::exercise::ExerciseResult = match serde_json::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                // Log first few deserialization errors for debugging
                static ERROR_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                let count = ERROR_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if count < 3 {
                    warn!("Failed to deserialize {}: {}\nContent preview: {}",
                        file_path.display(),
                        e,
                        content.chars().take(200).collect::<String>());
                }
                return Ok(None);
            }
        };
        
        // Skip if it doesn't have the required fields (empty exercise name means it's not a real result)
        if exercise_result.exercise_name.is_empty() || exercise_result.language.is_empty() {
            static SKIP_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let count = SKIP_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count < 3 {
                info!("Skipped {} - empty exercise_name='{}' or language='{}'",
                    file_path.display(),
                    exercise_result.exercise_name,
                    exercise_result.language);
            }
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

        // Use fields from the deserialized ExerciseResult
        let exercise = exercise_result.exercise_name.clone();
        let language = exercise_result.language.clone();
        let success = exercise_result.success;
        
        // Model: use from result if present, otherwise derive from directory
        let model = if !exercise_result.model.is_empty() {
            exercise_result.model.clone()
        } else {
            directory.clone()
        };

        // Timestamp: format end_time as RFC3339
        let timestamp = if !exercise_result.end_time.is_empty() {
            Some(exercise_result.end_time.clone())
        } else {
            None
        };

        // Agent: try to get from result, otherwise derive from filename
        let agent = if !exercise_result.model.is_empty() && exercise_result.exercise_name.starts_with("result_") {
            // Try to extract agent from model field if it contains agent info
            exercise_result.model.clone()
        } else {
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
        };

        // Duration: convert ms to seconds for storage (will be formatted later)
        let duration_seconds = if exercise_result.duration_ms > 0 {
            exercise_result.duration_ms as f64 / 1000.0
        } else {
            0.0
        };

        let output = exercise_result.output.clone();

        // Build results list for backward compatibility
        // Store duration as numeric value for proper sorting, format later for display
        let mut single_result = HashMap::new();
        single_result.insert("language".to_string(), language.clone());
        single_result.insert("exercise".to_string(), exercise.clone());
        single_result.insert("success".to_string(), success.to_string());
        single_result.insert("duration".to_string(), duration_seconds.to_string());
        single_result.insert("output".to_string(), output.clone());

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

        // Use tokens from deserialized result if present, otherwise parse from trace file
        let (input_tokens, output_tokens, cached_input_tokens, uncached_input_tokens) =
            if exercise_result.input_tokens > 0 || exercise_result.output_tokens > 0 {
                // Tokens already in the result file
                (
                    exercise_result.input_tokens,
                    exercise_result.output_tokens,
                    exercise_result.cached_input_tokens,
                    exercise_result.uncached_input_tokens,
                )
            } else if let Some(ref trace_path_str) = trace_path {
                if trace_path_str.ends_with(".jsonl") {
                    // Parse tokens from trace file
                    let trace_path_buf = PathBuf::from(trace_path_str);
                    parse_tokens_from_trace(&trace_path_buf, &agent)
                } else {
                    (0, 0, 0, 0)
                }
            } else {
                (0, 0, 0, 0)
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
            input_tokens,
            output_tokens,
            cached_input_tokens,
            uncached_input_tokens,
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
                        cached_result.agent, cached_result.model, cached_result.language, cached_result.exercise
                    ),
                    trace_url: format!(
                        "/results/{}/{}/{}/{}/trace",
                        cached_result.agent, cached_result.model, cached_result.language, cached_result.exercise
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
                    input_tokens: cached_result.input_tokens,
                    output_tokens: cached_result.output_tokens,
                    cached_input_tokens: cached_result.cached_input_tokens,
                    uncached_input_tokens: cached_result.uncached_input_tokens,
                    total_tokens: cached_result.input_tokens + cached_result.output_tokens,
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
            // Cache key format: {directory}/{language}/{exercise}
            // where directory = {agent}-{model} (internal implementation detail, NOT exposed in URLs)
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
        let mut total_input_tokens: u64 = 0;
        let mut total_output_tokens: u64 = 0;
        let mut total_cached_tokens: u64 = 0;
        let mut total_uncached_tokens: u64 = 0;
        let mut by_language: HashMap<String, (i32, i32, f64, u64, u64, u64, u64)> = HashMap::new();
        let mut by_agent: HashMap<String, (i32, i32, f64, u64, u64, u64, u64)> = HashMap::new();
        let mut by_model: HashMap<String, (i32, i32, f64, u64, u64, u64, u64)> = HashMap::new();

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

            // Accumulate tokens
            total_input_tokens += cached_result.input_tokens;
            total_output_tokens += cached_result.output_tokens;
            total_cached_tokens += cached_result.cached_input_tokens;
            total_uncached_tokens += cached_result.uncached_input_tokens;

            // By language - track (total, success, duration, input_tokens, output_tokens, cached_tokens, uncached_tokens)
            *by_language
                .entry(cached_result.language.clone())
                .or_insert((0, 0, 0.0, 0, 0, 0, 0)) = {
                let (t, s, d, ti, to, tc, tu) = by_language
                    .get(&cached_result.language)
                    .copied()
                    .unwrap_or((0, 0, 0.0, 0, 0, 0, 0));
                (
                    t + cached_result.total_exercises,
                    s + cached_result.successful,
                    d + entry_duration,
                    ti + cached_result.input_tokens,
                    to + cached_result.output_tokens,
                    tc + cached_result.cached_input_tokens,
                    tu + cached_result.uncached_input_tokens,
                )
            };

            // By agent - track (total, success, duration, tokens)
            *by_agent
                .entry(cached_result.agent.clone())
                .or_insert((0, 0, 0.0, 0, 0, 0, 0)) = {
                let (t, s, d, ti, to, tc, tu) = by_agent
                    .get(&cached_result.agent)
                    .copied()
                    .unwrap_or((0, 0, 0.0, 0, 0, 0, 0));
                (
                    t + cached_result.total_exercises,
                    s + cached_result.successful,
                    d + entry_duration,
                    ti + cached_result.input_tokens,
                    to + cached_result.output_tokens,
                    tc + cached_result.cached_input_tokens,
                    tu + cached_result.uncached_input_tokens,
                )
            };

            // By model - track (total, success, duration, tokens)
            let model_key = format!("{} - {}", cached_result.agent, cached_result.model);
            *by_model
                .entry(model_key.clone())
                .or_insert((0, 0, 0.0, 0, 0, 0, 0)) = {
                let (t, s, d, ti, to, tc, tu) = by_model
                    .get(&model_key)
                    .copied()
                    .unwrap_or((0, 0, 0.0, 0, 0, 0, 0));
                (
                    t + cached_result.total_exercises,
                    s + cached_result.successful,
                    d + entry_duration,
                    ti + cached_result.input_tokens,
                    to + cached_result.output_tokens,
                    tc + cached_result.cached_input_tokens,
                    tu + cached_result.uncached_input_tokens,
                )
            };
        }

        let success_rate = if total_exercises > 0 {
            (successful_exercises as f64 / total_exercises as f64) * 100.0
        } else {
            0.0
        };
        let success_rate_formatted = format!("{:.1}", success_rate);

        // Convert HashMaps to sorted Vec<StatItem> with duration and tokens
        let mut language_stats: Vec<StatItem> = by_language
            .iter()
            .map(|(name, (total, success, duration, input_tokens, output_tokens, cached_tokens, uncached_tokens))| {
                let rate = if *total > 0 { (*success as f64 / *total as f64) * 100.0 } else { 0.0 };
                StatItem {
                    name: name.clone(),
                    total: *total,
                    success: *success,
                    success_rate_formatted: format!("{:.1}", rate),
                    total_duration: *duration,
                    total_duration_formatted: Self::format_duration(*duration),
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                    cached_tokens: *cached_tokens,
                    uncached_tokens: *uncached_tokens,
                    agent: None,
                    model: None,
                }
            })
            .collect();
        language_stats.sort_by(|a, b| b.total.cmp(&a.total));

        let mut agent_stats: Vec<StatItem> = by_agent
            .iter()
            .map(|(name, (total, success, duration, input_tokens, output_tokens, cached_tokens, uncached_tokens))| {
                let rate = if *total > 0 { (*success as f64 / *total as f64) * 100.0 } else { 0.0 };
                StatItem {
                    name: name.clone(),
                    total: *total,
                    success: *success,
                    success_rate_formatted: format!("{:.1}", rate),
                    total_duration: *duration,
                    total_duration_formatted: Self::format_duration(*duration),
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                    cached_tokens: *cached_tokens,
                    uncached_tokens: *uncached_tokens,
                    agent: None,
                    model: None,
                }
            })
            .collect();
        agent_stats.sort_by(|a, b| b.total.cmp(&a.total));

        let mut model_stats: Vec<StatItem> = by_model
            .iter()
            .map(|(name, (total, success, duration, input_tokens, output_tokens, cached_tokens, uncached_tokens))| {
                let rate = if *total > 0 { (*success as f64 / *total as f64) * 100.0 } else { 0.0 };
                // Parse "agent - model" into separate fields for URL construction
                let (agent, model) = name.split_once(" - ")
                    .map(|(a, m)| (Some(a.to_string()), Some(m.to_string())))
                    .unwrap_or((None, None));
                
                StatItem {
                    name: name.clone(),
                    total: *total,
                    success: *success,
                    success_rate_formatted: format!("{:.1}", rate),
                    total_duration: *duration,
                    total_duration_formatted: Self::format_duration(*duration),
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                    cached_tokens: *cached_tokens,
                    uncached_tokens: *uncached_tokens,
                    agent,
                    model,
                }
            })
            .collect();
        model_stats.sort_by(|a, b| {
            // Sort by total descending, then by name ascending for consistency
            b.total.cmp(&a.total).then_with(|| a.name.cmp(&b.name))
        });

        let token_display = Self::format_tokens(total_uncached_tokens, total_cached_tokens, total_output_tokens);

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
            total_input_tokens,
            total_output_tokens,
            total_cached_tokens,
            total_uncached_tokens,
            token_display,
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

    /// Format tokens for display: "uncached / cached / output"
    fn format_tokens(uncached: u64, cached: u64, output: u64) -> String {
        format!(
            "{} / {} / {}",
            Self::format_number(uncached as i64),
            Self::format_number(cached as i64),
            Self::format_number(output as i64)
        )
    }

    /// Format large numbers with K/M/G suffixes.
    fn format_number(num: i64) -> String {
        let abs = num.unsigned_abs();
        if abs >= 1_000_000_000 {
            format!("{:.1}G", num as f64 / 1_000_000_000.0)
        } else if abs >= 1_000_000 {
            format!("{:.1}M", num as f64 / 1_000_000.0)
        } else if abs >= 1_000 {
            format!("{:.1}K", num as f64 / 1_000.0)
        } else {
            num.to_string()
        }
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

    /// Check if initial cache loading is complete.
    pub fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    /// Get the number of results currently in cache.
    pub fn result_count(&self) -> usize {
        self.result_count.load(Ordering::SeqCst)
    }

    /// Get loading status information.
    pub fn get_loading_status(&self) -> LoadingStatus {
        LoadingStatus {
            loaded: self.loaded.load(Ordering::SeqCst),
            result_count: self.result_count.load(Ordering::SeqCst),
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
