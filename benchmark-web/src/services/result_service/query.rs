//! Queries over the cached results — the browse and list endpoints, and trace lookups.

use super::{CachedResult, IndividualResult, ResultFilter, ResultService};

use anyhow::Result;
use benchmark_types::util::recover_poisoned;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

impl ResultService {
    /// Check if a cached result matches every filter in `filter`.
    ///
    /// Takes the cached fields and the filters separately rather than interleaved: eight
    /// positional `&str`/`Option<&str>` arguments made a swapped pair compile silently.
    pub(super) fn matches_filter(
        cached_lang: &str,
        cached_agent: &str,
        cached_model: &str,
        cached_exercise: &str,
        filter: &ResultFilter<'_>,
    ) -> bool {
        // An absent or empty filter means "no filter" (match all).
        let matches =
            |cached: &str, wanted: Option<&str>| wanted.is_none_or(|f| f.is_empty() || cached == f);
        matches(cached_lang, filter.language)
            && matches(cached_agent, filter.agent)
            && matches(cached_model, filter.model)
            && matches(cached_exercise, filter.exercise)
    }

    /// Convert a CachedResult to a metadata map.
    pub(super) fn to_metadata_map(cached: &CachedResult) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("filename".to_string(), cached.filename.clone());
        metadata.insert("directory".to_string(), cached.directory.clone());
        metadata.insert(
            "detailUrl".to_string(),
            format!(
                "/results/{}/{}/{}/{}",
                cached.agent, cached.model, cached.language, cached.exercise
            ),
        );
        metadata.insert(
            "traceUrl".to_string(),
            format!(
                "/results/{}/{}/{}/{}/trace",
                cached.agent, cached.model, cached.language, cached.exercise
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
        let models = recover_poisoned(self.cached_models.read());
        let mut unique: Vec<String> = models.iter().cloned().collect();
        unique.sort();
        unique.dedup();
        unique
    }

    /// All benchmark categories present in the results, as a map of id → label.
    /// Categories are discovered from the cached results; unknown/empty values
    /// map to `polyglot`. Returned as a map so Tera can iterate it by key/value.
    pub fn get_categories(&self) -> std::collections::BTreeMap<String, String> {
        let cached = recover_poisoned(self.cached_results.read());
        let mut cats: Vec<String> = cached
            .values()
            .map(|c| {
                if c.category.is_empty() {
                    "polyglot".to_string()
                } else {
                    c.category.clone()
                }
            })
            .collect();
        cats.sort();
        cats.dedup();
        let mut map = std::collections::BTreeMap::new();
        for c in cats {
            let label = match c.as_str() {
                "aoc2015" => "AoC 2015".to_string(),
                "polyglot" => "Exercism / Polyglot".to_string(),
                other => other.to_string(),
            };
            map.insert(c, label);
        }
        map
    }

    /// Get all unique languages.
    pub fn get_languages(&self) -> Vec<String> {
        let cached = recover_poisoned(self.cached_results.read());
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
        let cached = recover_poisoned(self.cached_results.read());
        let mut exercises: Vec<String> = Vec::new();
        for cached_result in cached.values() {
            // Treat empty string as "no filter" (match all)
            let matches_language =
                language.is_none_or(|l| l.is_empty() || cached_result.language == *l);
            if matches_language && !cached_result.exercise.is_empty() {
                exercises.push(cached_result.exercise.clone());
            }
        }
        exercises.sort();
        exercises.dedup();
        exercises
    }

    /// Check if an exercise is part of quick bench.
    pub(super) fn is_quick_bench_exercise(language: &str, exercise: &str) -> bool {
        let quick_exercises =
            benchmark_types::config::QuickBenchConfig::get_exercises_for_language(language);
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
        category: Option<&str>,
    ) -> Vec<IndividualResult> {
        let cached = recover_poisoned(self.cached_results.read());
        let mut results: Vec<IndividualResult> = Vec::new();

        for cached_result in cached.values() {
            // Apply category filter (empty = all categories).
            let cat = category.unwrap_or("");
            if !cat.is_empty() && cached_result.category != cat {
                continue;
            }

            // Apply quick bench filter
            if quick_only {
                let is_quick =
                    Self::is_quick_bench_exercise(&cached_result.language, &cached_result.exercise);
                if !is_quick {
                    continue;
                }
            }

            if Self::matches_filter(
                &cached_result.language,
                &cached_result.agent,
                &cached_result.model,
                &cached_result.exercise,
                &ResultFilter {
                    language,
                    agent,
                    model,
                    exercise,
                },
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
                        cached_result.agent,
                        cached_result.model,
                        cached_result.language,
                        cached_result.exercise
                    ),
                    trace_url: format!(
                        "/results/{}/{}/{}/{}/trace",
                        cached_result.agent,
                        cached_result.model,
                        cached_result.language,
                        cached_result.exercise
                    ),
                    path: cached_result.path.clone(),
                    agent: cached_result.agent.clone(),
                    language: cached_result.language.clone(),
                    category: cached_result.category.clone(),
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
                    input_chars: cached_result.input_chars,
                    output_chars: cached_result.output_chars,
                    tokens_per_sec: None, // Will be calculated after duration is populated
                    turn_count: cached_result.turn_count,
                    tool_call_count: cached_result.tool_call_count,
                    total_exercises: cached_result.total_exercises,
                    successful: cached_result.successful,
                    success_rate: 0.0,    // Will be calculated in calculate_scores
                    speed_score: 0.0,     // Will be calculated in calculate_scores
                    token_score: 0.0,     // Will be calculated in calculate_scores
                    composite_score: 0.0, // Will be calculated in calculate_scores
                });
            }
        }

        // Sort by timestamp descending (use epoch for reliable numeric sort)
        results.sort_by(|a, b| {
            let epoch_a = a.timestamp_epoch.unwrap_or(0.0);
            let epoch_b = b.timestamp_epoch.unwrap_or(0.0);
            epoch_b
                .partial_cmp(&epoch_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Extract duration from cached results and calculate tokens/sec
        for result in &mut results {
            // Look up the cached result to get duration
            // Cache key format: {directory}/{language}/{exercise}
            // where directory = {agent}-{model} (internal implementation detail, NOT exposed in URLs).
            // For category-namespaced results (e.g. {agent}-{model}/aoc2015/) the run
            // directory must be derived, not the immediate parent (which is the category).
            let directory = Self::derive_run_directory(std::path::Path::new(&result.path));
            let cache_key = format!("{}/{}/{}", directory, result.language, result.exercise);
            if let Some(cached) = recover_poisoned(self.cached_results.read()).get(&cache_key) {
                if let Some(single_result) = cached.results.first() {
                    if let Some(dur_str) = single_result.get("duration") {
                        if let Ok(dur) = dur_str.parse::<f64>() {
                            result.duration = Some(Self::format_duration(dur));
                            result.sort_duration = Some(dur);
                            // Calculate tokens per second
                            if dur > 0.0 {
                                result.tokens_per_sec = Some(result.output_tokens as f64 / dur);
                            }
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
        let cached = recover_poisoned(self.cached_results.read());
        cached.get(key).map(Self::to_metadata_map)
    }

    /// Get the rendered HTML trace for a result.
    /// If an HTML trace file already exists, returns it directly.
    /// Otherwise, if a JSONL trace file exists, generates the HTML via 'pi --export'.
    pub fn get_trace_content(&self, key: &str) -> Result<Option<String>> {
        let cached = recover_poisoned(self.cached_results.read());
        let cached_result = match cached.get(key) {
            Some(c) => c,
            None => {
                info!("TRACE MISS: No cached result for key='{}'", key);
                return Ok(None);
            }
        };

        info!(
            "TRACE HIT: key='{}', filename='{}', path='{}'",
            key, cached_result.filename, cached_result.path
        );

        let path_buf = PathBuf::from(&cached_result.path);
        let result_dir = path_buf
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine parent directory"))?;

        // For category-namespaced results ({agent}-{model}/{category}/) the trace is
        // written into the run dir ({agent}-{model}/), so search both the result's
        // directory and its parent.
        let trace_dirs: Vec<&Path> = {
            let mut dirs = vec![result_dir];
            if let Some(parent) = result_dir.parent() {
                dirs.push(parent);
            }
            dirs
        };

        // Derive the trace filename prefix from the result filename
        let trace_prefix = Self::derive_trace_prefix(&cached_result.filename);

        // Step 1: Check for existing HTML trace in any candidate dir
        for dir in &trace_dirs {
            let html_trace_path = dir.join(format!("trace_{}.html", trace_prefix));
            info!(
                "TRACE: checking HTML trace: {} (exists={})",
                html_trace_path.display(),
                html_trace_path.exists()
            );
            if html_trace_path.exists() {
                info!("Loading existing HTML trace: {}", html_trace_path.display());
                return Ok(Some(fs::read_to_string(&html_trace_path)?));
            }
        }

        // Step 2: Check for JSONL trace and try to generate HTML. The generated HTML
        // lands next to the JSONL trace so future lookups find it.
        let jsonl_trace_path = trace_dirs
            .iter()
            .map(|dir| dir.join(format!("trace_{}.jsonl", trace_prefix)))
            .find(|p| p.exists());
        if let Some(jsonl_trace_path) = jsonl_trace_path {
            let html_trace_path = jsonl_trace_path.with_extension("html");
            info!(
                "Generating HTML trace from JSONL: {}",
                jsonl_trace_path.display()
            );
            let output = std::process::Command::new("pi")
                .args([
                    "--export",
                    jsonl_trace_path.to_str().unwrap(),
                    html_trace_path.to_str().unwrap(),
                ])
                .output();

            match output {
                Ok(output) => {
                    if output.status.success() && html_trace_path.exists() {
                        info!("Generated HTML trace: {}", html_trace_path.display());
                        return Ok(Some(fs::read_to_string(&html_trace_path)?));
                    } else {
                        warn!(
                            "pi --export failed (exit={})",
                            output.status.code().unwrap_or(-1)
                        );
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
    pub fn result_file_success(
        &self,
        exercise_name: &str,
        agent_name: &str,
        model: &str,
        language: &str,
        results_dir: &Path,
    ) -> bool {
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
                warn!(
                    "Failed to read result file {}: {}",
                    result_path.display(),
                    e
                );
                false
            }
        }
    }
}
