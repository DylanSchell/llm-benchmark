//! Cross-model scoring and coverage, which back the compare view.

use super::{CompletenessInfo, ModelScore, ResultService, ScoredResult};

use benchmark_types::config::QuickBenchConfig;
use benchmark_types::util::recover_poisoned;
use std::collections::HashMap;

impl ResultService {
    /// Calculate composite scores for all individual results.
    /// Returns a vector of scored results sorted by composite score (descending).
    pub fn calculate_scores(
        &self,
        language: Option<&str>,
        agent: Option<&str>,
        model: Option<&str>,
        exercise: Option<&str>,
        quick_only: bool,
    ) -> Vec<ScoredResult> {
        let results =
            self.list_individual_results(language, agent, model, exercise, quick_only, None);

        if results.is_empty() {
            return vec![];
        }

        // Calculate normalization bounds from successful runs only
        let successful: Vec<_> = results
            .iter()
            .filter(|r| r.success && r.sort_duration.unwrap_or(0.0) > 0.0)
            .collect();

        if successful.is_empty() {
            // No successful runs - all scores are 0
            return results
                .into_iter()
                .map(|r| ScoredResult {
                    agent: r.agent,
                    model: r.model,
                    language: r.language,
                    exercise: r.exercise,
                    success: r.success,
                    successful: r.successful,
                    success_rate: 0.0,
                    speed_score: 0.0,
                    token_score: 0.0,
                    composite_score: 0.0,
                    duration: r.duration,
                    output_tokens: r.output_tokens,
                    input_chars: r.input_chars,
                    output_chars: r.output_chars,
                    tokens_per_sec: r.tokens_per_sec,
                    detail_url: r.detail_url,
                })
                .collect();
        }

        // Pre-compute per-exercise min/max for speed and token normalization.
        // We need to do this before consuming `results` since `successful` borrows it.
        use std::collections::HashMap;
        #[derive(Default)]
        struct ExBounds {
            min_dur: Option<f64>,
            max_dur: Option<f64>,
            min_tok: Option<u64>,
            max_tok: Option<u64>,
        }
        let mut ex_bounds: HashMap<(String, String), ExBounds> = HashMap::new();
        for s in &successful {
            let key = (s.language.clone(), s.exercise.clone());
            let b = ex_bounds.entry(key).or_default();
            if let Some(d) = s.sort_duration {
                b.min_dur = Some(b.min_dur.map_or(d, |v| v.min(d)));
                b.max_dur = Some(b.max_dur.map_or(d, |v| v.max(d)));
            }
            b.min_tok = Some(
                b.min_tok
                    .map_or(s.output_tokens, |v| v.min(s.output_tokens)),
            );
            b.max_tok = Some(
                b.max_tok
                    .map_or(s.output_tokens, |v| v.max(s.output_tokens)),
            );
        }

        // Calculate scores for all results, normalizing per-exercise so that
        // comparison is apples-to-apples: a C++ build isn't compared against
        // a trivial Python script's token count.
        results
            .into_iter()
            .map(|r| {
                let success_rate = if r.total_exercises > 0 {
                    r.successful as f64 / r.total_exercises as f64
                } else {
                    0.0
                };

                // Speed score: linear normalization within this exercise, faster = higher
                let speed_score = if r.success && r.sort_duration.unwrap_or(0.0) > 0.0 {
                    let key = (r.language.clone(), r.exercise.clone());
                    if let Some(b) = ex_bounds.get(&key) {
                        let min_d = b.min_dur.unwrap_or(0.0);
                        let max_d = b.max_dur.unwrap_or(0.0);
                        if max_d == min_d {
                            1.0
                        } else {
                            (max_d - r.sort_duration.unwrap_or(0.0)) / (max_d - min_d)
                        }
                    } else {
                        1.0
                    }
                } else {
                    0.0
                };

                // Token score: power-law normalization within this exercise, fewer = higher
                let token_score = if r.success && r.output_tokens > 0 {
                    let key = (r.language.clone(), r.exercise.clone());
                    if let Some(b) = ex_bounds.get(&key) {
                        let min_t = b.min_tok.unwrap_or(0);
                        let max_t = b.max_tok.unwrap_or(0);
                        if max_t == min_t || max_t == 0 {
                            1.0
                        } else {
                            let normalized = (r.output_tokens as f64 - min_t as f64)
                                / (max_t as f64 - min_t as f64);
                            let power_normalized = normalized.powf(0.3);
                            1.0 - power_normalized
                        }
                    } else {
                        1.0
                    }
                } else {
                    0.0
                };

                // Composite score: weighted sum
                // 70% correctness, 20% speed, 10% token efficiency
                // Correctness is paramount - incomplete or incorrect results should not rank high
                let composite_score = if success_rate > 0.0 {
                    0.7 * success_rate + 0.2 * speed_score + 0.1 * token_score
                } else {
                    0.0
                };

                ScoredResult {
                    agent: r.agent,
                    model: r.model,
                    language: r.language,
                    exercise: r.exercise,
                    success: r.success,
                    successful: r.successful,
                    success_rate,
                    speed_score,
                    token_score,
                    composite_score,
                    duration: r.duration,
                    output_tokens: r.output_tokens,
                    input_chars: r.input_chars,
                    output_chars: r.output_chars,
                    tokens_per_sec: r.tokens_per_sec,
                    detail_url: r.detail_url,
                }
            })
            .collect()
    }

    /// Get aggregated scoring statistics by model.
    ///
    /// The success-rate denominator is the **complete** expected benchmark
    /// (the authoritative exercise list from the exercise runner), not the set
    /// of results that happen to be cached. Using cached results as the
    /// denominator lets a model that skips exercises look perfect — the
    /// denominator shrinks to whatever it actually ran — so the full expected
    /// set must be used to properly penalize skipped exercises.
    pub fn get_model_scores(
        &self,
        language: Option<&str>,
        agent: Option<&str>,
        quick_only: bool,
        expected_exercises: &HashMap<String, Vec<String>>,
    ) -> Vec<ModelScore> {
        // Build the complete expected benchmark key set (language-qualified)
        // from the authoritative exercise list, respecting the language filter.
        let mut expected_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (lang, exercises) in expected_exercises {
            let matches_language = language.is_none_or(|l| l.is_empty() || l == lang);
            if !matches_language {
                continue;
            }
            for ex in exercises {
                if !quick_only || Self::is_quick_bench_exercise(lang, ex) {
                    expected_keys.insert(format!("{}:{}", lang, ex));
                }
            }
        }
        let benchmark_size = expected_keys.len() as u32;

        tracing::info!(
            "Calculated benchmark size from expected exercise set: {} (quick_only={}, language={:?})",
            benchmark_size, quick_only, language
        );

        let scored_results = self.calculate_scores(language, agent, None, None, quick_only);

        // Aggregate by model.
        #[derive(Default)]
        struct ModelAgg {
            composite: f64,
            successful: i32,
            runs: u32,
            speed: f64,
            token: f64,
            tokens: f64,
            input_chars: u64,
            output_chars: u64,
        }
        let mut model_map: std::collections::HashMap<String, ModelAgg> =
            std::collections::HashMap::new();

        for result in &scored_results {
            let key = format!("{} - {}", result.agent, result.model);
            let e = model_map.entry(key).or_default();

            e.composite += result.composite_score;
            // Count this as a successful exercise if the run succeeded
            if result.success {
                e.successful += 1; // One successful exercise
            }
            e.runs += 1; // Count this run (total exercises attempted)
            e.speed += result.speed_score;
            e.token += result.token_score;
            e.tokens += result.output_tokens as f64;
            e.input_chars += result.input_chars;
            e.output_chars += result.output_chars;
        }

        // Collect raw model averages first (before post-normalization)
        let raw_models: Vec<(String, f64, f64, f64, u64, u64, u32)> = model_map
            .into_iter()
            .map(|(name, a)| {
                let avg_success_rate = if benchmark_size > 0 {
                    let rate = a.successful as f64 / benchmark_size as f64;
                    rate.min(1.0)
                } else {
                    0.0
                };
                let avg_speed = a.speed / a.runs as f64;
                let avg_token = a.token / a.runs as f64;
                let total_chars = a.input_chars + a.output_chars;
                (
                    name,
                    avg_success_rate,
                    avg_speed,
                    avg_token,
                    a.tokens as u64,
                    total_chars,
                    a.runs,
                )
            })
            .collect();

        // Find maximum speed and token scores across all models so the best
        // model always gets 100% for that dimension.
        let max_speed = raw_models
            .iter()
            .map(|(_, _, s, _, _, _, _)| *s)
            .fold(0.0f64, f64::max);
        let max_token = raw_models
            .iter()
            .map(|(_, _, _, t, _, _, _)| *t)
            .fold(0.0f64, f64::max);

        // Normalize so the best model for each dimension gets 100%,
        // then compute the final composite score.
        raw_models
            .into_iter()
            .map(
                |(
                    name,
                    avg_success_rate,
                    avg_speed,
                    avg_token,
                    total_tokens,
                    total_chars,
                    total_runs,
                )| {
                    let norm_speed = if max_speed > 0.0 {
                        avg_speed / max_speed
                    } else {
                        0.0
                    };
                    let norm_token = if max_token > 0.0 {
                        avg_token / max_token
                    } else {
                        0.0
                    };
                    let avg_composite_score = if avg_success_rate > 0.0 {
                        0.7 * avg_success_rate + 0.2 * norm_speed + 0.1 * norm_token
                    } else {
                        0.0
                    };

                    ModelScore {
                        name,
                        avg_composite_score,
                        avg_success_rate,
                        avg_speed_score: norm_speed,
                        avg_token_score: norm_token,
                        total_tokens,
                        total_chars,
                        total_runs,
                    }
                },
            )
            .collect()
    }

    /// Get completeness info: which agent-model combinations have results for all exercises.
    ///
    /// `expected_exercises` provides the authoritative exercise list per language
    /// (from ExerciseRunner::get_exercises_for_language). When None and not in
    /// quick-only mode, falls back to discovered exercises from results.
    pub fn get_completeness_info(
        &self,
        quick_only: bool,
        expected_exercises: Option<HashMap<String, Vec<String>>>,
    ) -> Vec<CompletenessInfo> {
        let cached = recover_poisoned(self.cached_results.read());

        // Determine the total expected exercise count.
        // If quick_only, sum up QuickBenchConfig counts per language.
        // Otherwise, use all discovered exercises (deduplicated by name).
        let mut all_exercise_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        if quick_only {
            // Build expected set from QuickBenchConfig
            if let Some(ref expected) = expected_exercises {
                for lang in expected.keys() {
                    let quick = QuickBenchConfig::get_exercises_for_language(lang);
                    for ex in quick {
                        all_exercise_names.insert(ex);
                    }
                }
            }
        } else {
            // Use authoritative list from exercise runner if available
            if let Some(ref expected) = expected_exercises {
                for exercises in expected.values() {
                    for ex in exercises {
                        all_exercise_names.insert(ex.clone());
                    }
                }
            } else {
                // Fallback: use all discovered exercise names from results
                for cr in cached.values() {
                    if !cr.exercise.is_empty() {
                        all_exercise_names.insert(cr.exercise.clone());
                    }
                }
            }
        }

        let total_expected = all_exercise_names.len();

        // Build map of agent-model -> set of exercises that have results
        let mut key_exercises: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
        for cr in cached.values() {
            if cr.exercise.is_empty() {
                continue;
            }
            let key = format!("{} - {}", cr.agent, cr.model);
            key_exercises
                .entry(key)
                .or_default()
                .insert(cr.exercise.clone());
        }

        // Find keys that have ALL expected exercises
        let complete_keys: Vec<String> = key_exercises
            .into_iter()
            .filter(|(_, exercises)| all_exercise_names.is_subset(exercises))
            .map(|(key, _)| key)
            .collect();

        vec![CompletenessInfo {
            total_exercises: total_expected,
            complete_keys,
        }]
    }
}
