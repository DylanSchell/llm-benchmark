//! Aggregate statistics for the dashboard, plus the formatting they render as.

use super::{GroupStatsMap, ResultFilter, ResultService, StatItem, Statistics};

use benchmark_types::agent::{merge_intervals_and_total_duration, parse_rfc3339_ms, TimeInterval};
use benchmark_types::util::recover_poisoned;
use std::collections::HashMap;

impl ResultService {
    /// Get statistics.
    pub fn get_statistics(
        &self,
        language: Option<&str>,
        agent: Option<&str>,
        model: Option<&str>,
        exercise: Option<&str>,
        quick_only: bool,
        category: Option<&str>,
    ) -> Statistics {
        let cached = recover_poisoned(self.cached_results.read());
        let mut total_runs: i32 = 0;
        let mut total_exercises: i32 = 0;
        let mut successful_exercises: i32 = 0;
        let mut total_duration: f64 = 0.0;
        let mut total_input_tokens: u64 = 0;
        let mut total_output_tokens: u64 = 0;
        let mut total_cached_tokens: u64 = 0;
        let mut total_uncached_tokens: u64 = 0;
        let mut by_language: GroupStatsMap = HashMap::new();
        let mut by_agent: GroupStatsMap = HashMap::new();
        let mut by_model: GroupStatsMap = HashMap::new();
        let mut by_category: GroupStatsMap = HashMap::new();
        // Wall-clock intervals per group
        let mut wall_by_language: HashMap<String, Vec<TimeInterval>> = HashMap::new();
        let mut wall_by_agent: HashMap<String, Vec<TimeInterval>> = HashMap::new();
        let mut wall_by_model: HashMap<String, Vec<TimeInterval>> = HashMap::new();
        let mut wall_by_category: HashMap<String, Vec<TimeInterval>> = HashMap::new();

        for cached_result in cached.values() {
            // Apply category filter (empty = all categories).
            let cat = category.unwrap_or("");
            if !cat.is_empty() && cached_result.category != cat {
                continue;
            }

            if !Self::matches_filter(
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
                continue;
            }

            // Apply quick bench filter
            if quick_only
                && !Self::is_quick_bench_exercise(&cached_result.language, &cached_result.exercise)
            {
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
            let lang_intervals = wall_by_language
                .entry(cached_result.language.clone())
                .or_default();
            if let (Some(s), Some(e)) = (
                parse_rfc3339_ms(&cached_result.start_time),
                parse_rfc3339_ms(&cached_result.end_time),
            ) {
                if e >= s {
                    lang_intervals.push(TimeInterval { start: s, end: e });
                }
            }
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
            let agent_intervals = wall_by_agent
                .entry(cached_result.agent.clone())
                .or_default();
            if let (Some(s), Some(e)) = (
                parse_rfc3339_ms(&cached_result.start_time),
                parse_rfc3339_ms(&cached_result.end_time),
            ) {
                if e >= s {
                    agent_intervals.push(TimeInterval { start: s, end: e });
                }
            }
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
            let model_intervals = wall_by_model.entry(model_key.clone()).or_default();
            if let (Some(s), Some(e)) = (
                parse_rfc3339_ms(&cached_result.start_time),
                parse_rfc3339_ms(&cached_result.end_time),
            ) {
                if e >= s {
                    model_intervals.push(TimeInterval { start: s, end: e });
                }
            }
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

            // By category - track (total, success, duration, tokens)
            let cat_key = if cached_result.category.is_empty() {
                "polyglot".to_string()
            } else {
                cached_result.category.clone()
            };
            let cat_intervals = wall_by_category.entry(cat_key.clone()).or_default();
            if let (Some(s), Some(e)) = (
                parse_rfc3339_ms(&cached_result.start_time),
                parse_rfc3339_ms(&cached_result.end_time),
            ) {
                if e >= s {
                    cat_intervals.push(TimeInterval { start: s, end: e });
                }
            }
            *by_category
                .entry(cat_key.clone())
                .or_insert((0, 0, 0.0, 0, 0, 0, 0)) = {
                let (t, s, d, ti, to, tc, tu) = by_category
                    .get(&cat_key)
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

        // Compute wall-clock duration by merging overlapping intervals from start_time/end_time
        let intervals: Vec<TimeInterval> = cached
            .values()
            .filter(|cr| {
                // Apply same filters
                Self::matches_filter(
                    &cr.language,
                    &cr.agent,
                    &cr.model,
                    &cr.exercise,
                    &ResultFilter {
                        language,
                        agent,
                        model,
                        exercise,
                    },
                ) && (!quick_only || Self::is_quick_bench_exercise(&cr.language, &cr.exercise))
            })
            .filter_map(|cr| {
                let start_ms = parse_rfc3339_ms(&cr.start_time);
                let end_ms = parse_rfc3339_ms(&cr.end_time);
                match (start_ms, end_ms) {
                    (Some(start), Some(end)) if end >= start => Some(TimeInterval { start, end }),
                    _ => None,
                }
            })
            .collect();
        let wall_clock_ms = merge_intervals_and_total_duration(&intervals);
        let wall_clock_seconds = wall_clock_ms as f64 / 1000.0;

        // Convert HashMaps to sorted Vec<StatItem> with duration and tokens
        let mut language_stats: Vec<StatItem> = by_language
            .iter()
            .map(
                |(
                    name,
                    (
                        total,
                        success,
                        duration,
                        input_tokens,
                        output_tokens,
                        cached_tokens,
                        uncached_tokens,
                    ),
                )| {
                    let rate = if *total > 0 {
                        (*success as f64 / *total as f64) * 100.0
                    } else {
                        0.0
                    };
                    let avg_tps = if *duration > 0.0 {
                        *output_tokens as f64 / *duration
                    } else {
                        0.0
                    };
                    let wall_intervals = wall_by_language
                        .get(name)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);
                    let wall_secs =
                        merge_intervals_and_total_duration(wall_intervals) as f64 / 1000.0;
                    StatItem {
                        name: name.clone(),
                        total: *total,
                        success: *success,
                        success_rate_formatted: format!("{:.1}", rate),
                        total_duration: *duration,
                        total_duration_formatted: Self::format_duration(*duration),
                        wall_clock_duration: wall_secs,
                        wall_clock_duration_formatted: Self::format_duration(wall_secs),
                        input_tokens: *input_tokens,
                        output_tokens: *output_tokens,
                        cached_tokens: *cached_tokens,
                        uncached_tokens: *uncached_tokens,
                        agent: None,
                        model: None,
                        avg_tokens_per_sec: if avg_tps > 0.0 { Some(avg_tps) } else { None },
                    }
                },
            )
            .collect();
        language_stats.sort_by_key(|item| std::cmp::Reverse(item.total));

        let mut agent_stats: Vec<StatItem> = by_agent
            .iter()
            .map(
                |(
                    name,
                    (
                        total,
                        success,
                        duration,
                        input_tokens,
                        output_tokens,
                        cached_tokens,
                        uncached_tokens,
                    ),
                )| {
                    let rate = if *total > 0 {
                        (*success as f64 / *total as f64) * 100.0
                    } else {
                        0.0
                    };
                    let avg_tps = if *duration > 0.0 {
                        *output_tokens as f64 / *duration
                    } else {
                        0.0
                    };
                    let wall_intervals =
                        wall_by_agent.get(name).map(|v| v.as_slice()).unwrap_or(&[]);
                    let wall_secs =
                        merge_intervals_and_total_duration(wall_intervals) as f64 / 1000.0;
                    StatItem {
                        name: name.clone(),
                        total: *total,
                        success: *success,
                        success_rate_formatted: format!("{:.1}", rate),
                        total_duration: *duration,
                        total_duration_formatted: Self::format_duration(*duration),
                        wall_clock_duration: wall_secs,
                        wall_clock_duration_formatted: Self::format_duration(wall_secs),
                        input_tokens: *input_tokens,
                        output_tokens: *output_tokens,
                        cached_tokens: *cached_tokens,
                        uncached_tokens: *uncached_tokens,
                        agent: None,
                        model: None,
                        avg_tokens_per_sec: if avg_tps > 0.0 { Some(avg_tps) } else { None },
                    }
                },
            )
            .collect();
        agent_stats.sort_by_key(|item| std::cmp::Reverse(item.total));

        let mut model_stats: Vec<StatItem> = by_model
            .iter()
            .map(
                |(
                    name,
                    (
                        total,
                        success,
                        duration,
                        input_tokens,
                        output_tokens,
                        cached_tokens,
                        uncached_tokens,
                    ),
                )| {
                    let rate = if *total > 0 {
                        (*success as f64 / *total as f64) * 100.0
                    } else {
                        0.0
                    };
                    // Parse "agent - model" into separate fields for URL construction
                    let (agent, model) = name
                        .split_once(" - ")
                        .map(|(a, m)| (Some(a.to_string()), Some(m.to_string())))
                        .unwrap_or((None, None));
                    let avg_tps = if *duration > 0.0 {
                        *output_tokens as f64 / *duration
                    } else {
                        0.0
                    };
                    let wall_intervals =
                        wall_by_model.get(name).map(|v| v.as_slice()).unwrap_or(&[]);
                    let wall_secs =
                        merge_intervals_and_total_duration(wall_intervals) as f64 / 1000.0;

                    StatItem {
                        name: name.clone(),
                        total: *total,
                        success: *success,
                        success_rate_formatted: format!("{:.1}", rate),
                        total_duration: *duration,
                        total_duration_formatted: Self::format_duration(*duration),
                        wall_clock_duration: wall_secs,
                        wall_clock_duration_formatted: Self::format_duration(wall_secs),
                        input_tokens: *input_tokens,
                        output_tokens: *output_tokens,
                        cached_tokens: *cached_tokens,
                        uncached_tokens: *uncached_tokens,
                        agent,
                        model,
                        avg_tokens_per_sec: if avg_tps > 0.0 { Some(avg_tps) } else { None },
                    }
                },
            )
            .collect();
        model_stats.sort_by(|a, b| {
            // Sort by total descending, then by name ascending for consistency
            b.total.cmp(&a.total).then_with(|| a.name.cmp(&b.name))
        });

        let mut category_stats: Vec<StatItem> = by_category
            .iter()
            .map(
                |(
                    name,
                    (
                        total,
                        success,
                        duration,
                        input_tokens,
                        output_tokens,
                        cached_tokens,
                        uncached_tokens,
                    ),
                )| {
                    let rate = if *total > 0 {
                        (*success as f64 / *total as f64) * 100.0
                    } else {
                        0.0
                    };
                    let avg_tps = if *duration > 0.0 {
                        *output_tokens as f64 / *duration
                    } else {
                        0.0
                    };
                    let wall_intervals = wall_by_category
                        .get(name)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);
                    let wall_secs =
                        merge_intervals_and_total_duration(wall_intervals) as f64 / 1000.0;
                    StatItem {
                        name: name.clone(),
                        total: *total,
                        success: *success,
                        success_rate_formatted: format!("{:.1}", rate),
                        total_duration: *duration,
                        total_duration_formatted: Self::format_duration(*duration),
                        wall_clock_duration: wall_secs,
                        wall_clock_duration_formatted: Self::format_duration(wall_secs),
                        input_tokens: *input_tokens,
                        output_tokens: *output_tokens,
                        cached_tokens: *cached_tokens,
                        uncached_tokens: *uncached_tokens,
                        agent: None,
                        model: None,
                        avg_tokens_per_sec: if avg_tps > 0.0 { Some(avg_tps) } else { None },
                    }
                },
            )
            .collect();
        category_stats.sort_by_key(|item| std::cmp::Reverse(item.total));

        let token_display = Self::format_tokens(
            total_uncached_tokens,
            total_cached_tokens,
            total_output_tokens,
        );

        Statistics {
            total_runs,
            total_exercises,
            successful_exercises,
            success_rate,
            success_rate_formatted,
            total_duration,
            total_duration_formatted: Self::format_duration(total_duration),
            wall_clock_duration: wall_clock_seconds,
            wall_clock_duration_formatted: Self::format_duration(wall_clock_seconds),
            total_results: total_runs,
            successful_results: successful_exercises,
            language_stats,
            agent_stats,
            model_stats,
            category_stats,
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
            s.trim_end_matches("ms")
                .parse::<f64>()
                .map(|v| v / 1000.0)
                .ok()
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
}
