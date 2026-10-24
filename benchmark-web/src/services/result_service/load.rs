//! Result loading: scanning the results directory and keeping the in-memory cache fresh.

use super::trace::{parse_chars_from_trace, parse_tokens_from_trace, parse_trace_counts};
use super::{CachedResult, LoadingStatus, ResultService};

use anyhow::Result;
use benchmark_types::util::recover_poisoned;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

impl ResultService {
    /// Load all result files into the in-memory cache using parallel processing.
    pub fn load_all_results(&self) {
        info!(
            "Loading all results into cache from: {}",
            self.results_dir.display()
        );

        if !self.results_dir.exists() {
            warn!(
                "Results directory does not exist: {}",
                self.results_dir.display()
            );
            return;
        }

        // Collect all result file paths first (this is fast, just directory traversal).
        // Recursive so category-namespaced results (e.g. `{agent}-{model}/aoc2015/`)
        // are discovered too.
        let mut result_paths: Vec<PathBuf> = Vec::new();
        let mut stack: Vec<PathBuf> = vec![self.results_dir.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let filename = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if filename.starts_with("result_") && filename.ends_with(".json") {
                    result_paths.push(path);
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
                        let file_name = file_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        let parent = file_path
                            .parent()
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
            info!(
                "Skipped {} files that are not valid exercise results:",
                actual_skipped_count
            );
            if let Ok(skipped) = skipped_files.read() {
                for file_path in skipped.iter().take(10) {
                    // Extract just the relative path from results dir for readability
                    let relative = file_path
                        .strip_prefix(self.results_dir.to_string_lossy().as_ref())
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
            info!(
                "Encountered {} actual errors while loading result files:",
                actual_error_count
            );
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
        let mut cached = recover_poisoned(self.cached_results.write());
        let mut models: Vec<String> = Vec::new();

        for (cache_key, cached_result) in results {
            cached.insert(cache_key, cached_result.clone());
            models.push(cached_result.model);
        }

        models.sort();
        models.dedup();

        drop(cached);

        // Update models cache
        let mut models_cache = recover_poisoned(self.cached_models.write());
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
    pub(super) fn load_cached_result(file_path: &Path) -> Result<Option<CachedResult>> {
        let content = fs::read_to_string(file_path)?;

        // Deserialize as AgentResult (handles both camelCase and snake_case via serde aliases)
        let agent_result: benchmark_types::agent::AgentResult = match serde_json::from_str(&content)
        {
            Ok(r) => r,
            Err(e) => {
                // Log first few deserialization errors for debugging
                static ERROR_COUNT: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let count = ERROR_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if count < 3 {
                    warn!(
                        "Failed to deserialize {}: {}\nContent preview: {}",
                        file_path.display(),
                        e,
                        content.chars().take(200).collect::<String>()
                    );
                }
                return Ok(None);
            }
        };

        // Skip if it doesn't have the required fields (empty exercise name means it's not a real result)
        if agent_result.exercise_name.is_empty() || agent_result.language.is_empty() {
            static SKIP_COUNT: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            let count = SKIP_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count < 3 {
                info!(
                    "Skipped {} - empty exercise_name='{}' or language='{}'",
                    file_path.display(),
                    agent_result.exercise_name,
                    agent_result.language
                );
            }
            return Ok(None);
        }

        let filename = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        // `directory` is the `{agent}-{model}` run directory. Results may be
        // nested under a category subdirectory (e.g. `{agent}-{model}/aoc2015/`),
        // so walk up past any category-namespaced component to the run dir.
        let directory = Self::derive_run_directory(file_path);

        // Use fields from the deserialized AgentResult
        let exercise = agent_result.exercise_name.clone();
        let language = agent_result.language.clone();
        let success = agent_result.success;

        // Model: use from result if present, otherwise derive from directory
        let model = if !agent_result.model.is_empty() {
            agent_result.model.clone()
        } else {
            directory.clone()
        };

        // Timestamp: format end_time as RFC3339
        let timestamp = if !agent_result.end_time.is_empty() {
            Some(agent_result.end_time.clone())
        } else {
            None
        };

        let start_time = agent_result.start_time.clone();
        let end_time = agent_result.end_time.clone();

        // Agent: derive from filename: result_<agent>_<lang>_<exercise>.json
        let agent = if let Some(without_prefix) = filename.strip_prefix("result_") {
            without_prefix
                .split('_')
                .next()
                .unwrap_or("unknown")
                .to_string()
        } else {
            "unknown".to_string()
        };

        // Duration: convert ms to seconds for storage (will be formatted later)
        let duration_seconds = if agent_result.duration_ms > 0 {
            agent_result.duration_ms as f64 / 1000.0
        } else {
            0.0
        };

        let output = agent_result.output.clone();

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

        // Check for existing trace files on disk (embedded traces are legacy and ignored).
        // For category-namespaced results ({agent}-{model}/{category}/) the trace is
        // written into the run dir ({agent}-{model}/), so search both the result's
        // directory and its parent.
        let trace_prefix = Self::derive_trace_prefix(&filename);
        let mut trace_dirs: Vec<&Path> = Vec::new();
        if let Some(parent) = file_path.parent() {
            trace_dirs.push(parent);
            if let Some(grandparent) = parent.parent() {
                trace_dirs.push(grandparent);
            }
        }
        let trace_path = trace_dirs.into_iter().find_map(|dir| {
            let jsonl_path = dir.join(format!("trace_{}.jsonl", trace_prefix));
            if jsonl_path.exists() {
                Some(jsonl_path.to_string_lossy().to_string())
            } else {
                let html_path = dir.join(format!("trace_{}.html", trace_prefix));
                if html_path.exists() {
                    Some(html_path.to_string_lossy().to_string())
                } else {
                    None
                }
            }
        });

        // Use tokens from deserialized result if present, otherwise parse from trace file
        let (input_tokens, output_tokens, cached_input_tokens, uncached_input_tokens) =
            if agent_result.input_tokens > 0 || agent_result.output_tokens > 0 {
                // Tokens already in the result file
                (
                    agent_result.input_tokens,
                    agent_result.output_tokens,
                    agent_result.cached_input_tokens,
                    agent_result.uncached_input_tokens,
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

        // Parse character counts from trace file (tokenizer-independent text volume)
        let (input_chars, output_chars) = if let Some(ref tp) = trace_path {
            let trace_path_buf = PathBuf::from(tp);
            parse_chars_from_trace(&trace_path_buf)
        } else {
            (0, 0)
        };

        // Parse turn count and tool call count from trace file
        let (turn_count, tool_call_count) = if let Some(ref tp) = trace_path {
            let trace_path_buf = PathBuf::from(tp);
            parse_trace_counts(&trace_path_buf, &agent)
        } else {
            (0, 0)
        };

        Ok(Some(CachedResult {
            filename,
            directory,
            exercise,
            path: file_path.to_string_lossy().to_string(),
            timestamp,
            agent,
            language,
            category: agent_result.category.to_string(),
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
            input_chars,
            output_chars,
            turn_count,
            tool_call_count,
            start_time,
            end_time,
        }))
    }

    /// Derive the `{agent}-{model}` run directory from a result file path.
    ///
    /// Results live at `{results_dir}/{agent}-{model}/result_*.json` or, for a
    /// non-default category, `{results_dir}/{agent}-{model}/{category}/result_*.json`.
    /// Returns the `{agent}-{model}` component (the first path segment under the
    /// results dir), falling back to the immediate parent name.
    pub(super) fn derive_run_directory(file_path: &Path) -> String {
        // Walk from the file's parent up to find a component that looks like a
        // run dir (`{agent}-{model}`) — i.e. not a known category name.
        let mut current = file_path.parent();
        while let Some(dir) = current {
            if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
                // Category subdirs are single, non-dashed names like `aoc2015`;
                // run dirs are `{agent}-{model}` (contain a hyphen). Prefer the
                // first non-category ancestor.
                if !matches!(name, "aoc2015" | "polyglot") {
                    return name.to_string();
                }
            }
            current = dir.parent();
        }
        file_path
            .parent()
            .and_then(|p| p.file_name())
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    /// Derive trace prefix from result filename.
    pub(super) fn derive_trace_prefix(filename: &str) -> String {
        if let Some(without_prefix) = filename.strip_prefix("result_") {
            // Remove "result_"
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

    /// Refresh the result cache.
    /// Refresh the in-memory cache, but only if more than 5 seconds have passed
    /// since the last refresh (to avoid rebuilding on every request).
    pub fn refresh_cache(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut last = recover_poisoned(self.last_refresh.lock());
        if now - *last > 5000 {
            self.load_all_results();
            *last = now;
        }
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
                let mut cached = recover_poisoned(self.cached_results.write());
                let was_present = cached.contains_key(&cache_key);
                cached.insert(cache_key.clone(), cached_result.clone());
                drop(cached);

                // Also ensure the model is in the models list
                let mut models = recover_poisoned(self.cached_models.write());
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
                warn!(
                    "Failed to update single result from {}: {}",
                    file_path.display(),
                    e
                );
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
}
