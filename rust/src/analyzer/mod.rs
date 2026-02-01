use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::info;

/// Track path with its modification time for sorting
#[derive(Debug, Clone)]
struct PathTime {
    path: PathBuf,
    file_time: SystemTime,
}

impl PathTime {
    fn new(path: PathBuf) -> Result<Self, std::io::Error> {
        let file_time = fs::metadata(&path)?.modified()?;
        Ok(Self { path, file_time })
    }
}

impl PartialEq for PathTime {
    fn eq(&self, other: &Self) -> bool {
        self.file_time == other.file_time
    }
}

impl Eq for PathTime {}

impl PartialOrd for PathTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PathTime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.file_time.cmp(&other.file_time)
    }
}

/// Token usage from jsonl trace file
#[derive(Debug, Default)]
struct TokenUsage {
    input_tokens: u64,
    output_tokens: u64,
    cached_input_tokens: u64,
    uncached_input_tokens: u64,
}

impl TokenUsage {
    fn add(&mut self, input: u64, output: u64) {
        self.input_tokens += input;
        self.output_tokens += output;
    }
}

/// Log entry structures for parsing jsonl
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LogEntry {
    #[serde(default, rename = "type")]
    _type: Option<String>,
    message: Option<Message>,
}

#[derive(Debug, Deserialize)]
struct Message {
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default, rename = "cache_read_input_tokens")]
    cache_read_input_tokens: Option<u64>,
    #[serde(default, rename = "cache_creation_input_tokens")]
    cache_creation_input_tokens: Option<u64>,
}

/// Simple result structure for benchmark results
#[derive(Debug, Clone, Deserialize)]
pub struct SimpleResult {
    pub model: Option<String>,
    pub language: Option<String>,
    #[serde(rename = "exerciseName")]
    pub exercise_name: Option<String>,
    pub duration: f64,
    pub output: Option<String>,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub uncached_input_tokens: Option<u64>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub error_message: Option<String>,
    pub trace: Option<String>,
}

impl SimpleResult {
    pub fn exercise_name(&self) -> &str {
        self.exercise_name.as_deref().unwrap_or("unknown")
    }

    pub fn model(&self) -> &str {
        self.model.as_deref().unwrap_or("unknown")
    }
}

#[derive(Debug)]
struct BenchmarkStats {
    benchmark_name: String,
    total_results: usize,
    success_results: usize,
    failed_results: usize,
    total_duration: f64,
    input_tokens: u64,
    output_tokens: u64,
    cached_input_tokens: u64,
    uncached_input_tokens: u64,
}

impl BenchmarkStats {
    fn new(name: &str) -> Self {
        Self {
            benchmark_name: name.to_string(),
            total_results: 0,
            success_results: 0,
            failed_results: 0,
            total_duration: 0.0,
            input_tokens: 0,
            output_tokens: 0,
            cached_input_tokens: 0,
            uncached_input_tokens: 0,
        }
    }

    fn completion_percent(&self) -> f64 {
        if self.total_results == 0 {
            0.0
        } else {
            (self.success_results as f64 / self.total_results as f64) * 100.0
        }
    }
}

pub struct BenchmarkAnalyzer;

impl BenchmarkAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Analyze results in a directory and generate report
    pub fn analyze(&self, results_dir: &str, output_path: &str) -> Result<(), std::io::Error> {
        let base_dir = PathBuf::from(results_dir);

        let mut all_results: Vec<SimpleResult> = Vec::new();
        let mut all_exercises: HashSet<String> = HashSet::new();
        let mut stats_by_benchmark: HashMap<String, BenchmarkStats> = HashMap::new();
        let mut results_by_exercise: HashMap<String, Vec<SimpleResult>> = HashMap::new();
        let mut results_by_benchmark: HashMap<String, Vec<SimpleResult>> = HashMap::new();

        self.archive_claude_projects(&base_dir)?;

        self.process_results_directory(
            &base_dir,
            &mut all_results,
            &mut all_exercises,
            &mut stats_by_benchmark,
            &mut results_by_exercise,
            &mut results_by_benchmark,
        )?;

        self.generate_report(
            &all_results,
            &all_exercises,
            &stats_by_benchmark,
            &results_by_benchmark,
            &results_by_exercise,
            output_path,
        )?;

        Ok(())
    }

    /// Copy all jsonl trace files to ~/.claude/projects/benchmark/ for ccusage
    fn archive_claude_projects(&self, base_dir: &Path) -> Result<(), std::io::Error> {
        let target_dir = match home::home_dir() {
            Some(home) => home.join(".claude/projects/benchmark"),
            None => {
                tracing::warn!("Could not find home directory, skipping archive");
                return Ok(());
            }
        };

        if let Err(e) = fs::create_dir_all(&target_dir) {
            tracing::warn!("Could not create target directory {:?}: {}", target_dir, e);
            return Ok(());
        }

        let entries = fs::read_dir(base_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.archive_claude_projects(&path)?;
            } else if path.to_string_lossy().ends_with("jsonl") {
                let filename = path.file_name().unwrap().to_string_lossy();
                let target = target_dir.join(&*filename);

                if !target.exists() {
                    if let Err(e) = fs::copy(&path, &target) {
                        tracing::warn!("Could not copy {}: {}", path.display(), e);
                    } else {
                        tracing::info!("Archived: {}", filename);
                    }
                }
            }
        }

        Ok(())
    }

    fn process_results_directory(
        &self,
        dir: &Path,
        all_results: &mut Vec<SimpleResult>,
        all_exercises: &mut HashSet<String>,
        stats_by_benchmark: &mut HashMap<String, BenchmarkStats>,
        results_by_exercise: &mut HashMap<String, Vec<SimpleResult>>,
        results_by_benchmark: &mut HashMap<String, Vec<SimpleResult>>,
    ) -> Result<(), std::io::Error> {
        let entries = fs::read_dir(dir)?;

        let mut result_files: Vec<PathBuf> = Vec::new();
        let mut trace_files: Vec<PathBuf> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.process_results_directory(
                    &path,
                    all_results,
                    all_exercises,
                    stats_by_benchmark,
                    results_by_exercise,
                    results_by_benchmark,
                )?;
            } else if path.to_string_lossy().ends_with(".json") {
                let filename = path.file_name().unwrap().to_string_lossy();
                if filename.starts_with("result_claude")
                /* && !filename.contains("result_claude_") */
                {
                    result_files.push(path);
                }
            } else if path.to_string_lossy().ends_with("jsonl") {
                trace_files.push(path);
            }
        }

        // Sort by modification time
        let mut result_times: Vec<(PathBuf, SystemTime)> = result_files
            .into_iter()
            .map(|p| {
                let time = fs::metadata(&p)
                    .and_then(|m| m.modified())
                    .unwrap_or_else(|_| SystemTime::UNIX_EPOCH);
                (p, time)
            })
            .collect();
        result_times.sort_by_key(|(_, t)| *t);

        let mut trace_times: Vec<PathTime> = trace_files
            .into_iter()
            .filter_map(|p| PathTime::new(p).ok())
            .collect();
        trace_times.sort();

        if !result_times.is_empty() {
            let benchmark_name = dir.file_name().unwrap().to_string_lossy().to_string();
            let stats = BenchmarkStats::new(&benchmark_name);
            stats_by_benchmark.insert(benchmark_name.clone(), stats);
            for (result_file, result_time) in &result_times {
                match self.parse_result_file(result_file) {
                    Ok(mut simple) => {
                        simple.model = Some(benchmark_name.clone());
                        all_results.push(simple.clone());
                        all_exercises.insert(simple.exercise_name().to_string());

                        results_by_benchmark
                            .entry(benchmark_name.clone())
                            .or_insert_with(Vec::new)
                            .push(simple.clone());

                        results_by_exercise
                            .entry(simple.exercise_name().to_string())
                            .or_insert_with(Vec::new)
                            .push(simple.clone());

                        if let Some(stats) = stats_by_benchmark.get_mut(&benchmark_name) {
                            stats.total_results += 1;
                            if simple.success {
                                stats.success_results += 1;
                            } else {
                                stats.failed_results += 1;
                            }
                            stats.total_duration += simple.duration;
                        }

                        // Process trace files that are older than this result file
                        while !trace_times.is_empty() {
                            let next_trace = &trace_times[0];
                            if result_time.cmp(&next_trace.file_time) == std::cmp::Ordering::Greater
                            {
                                let trace_path = trace_times.remove(0).path;
                                match self.calculate_tokens(&trace_path) {
                                    Ok(usage) => {
                                        if let Some(stats) =
                                            stats_by_benchmark.get_mut(&benchmark_name)
                                        {
                                            stats.input_tokens += usage.input_tokens;
                                            stats.output_tokens += usage.output_tokens;
                                            stats.cached_input_tokens += usage.cached_input_tokens;
                                            stats.uncached_input_tokens +=
                                                usage.uncached_input_tokens;
                                        }
                                        if let Some(simple) = all_results.iter_mut().find(|r| {
                                            r.model == Some(benchmark_name.clone())
                                                && r.exercise_name == simple.exercise_name
                                                && r.duration == simple.duration
                                        }) {
                                            simple.input_tokens = Some(
                                                simple.input_tokens.unwrap_or(0)
                                                    + usage.input_tokens,
                                            );
                                            simple.output_tokens = Some(
                                                simple.output_tokens.unwrap_or(0)
                                                    + usage.output_tokens,
                                            );
                                            simple.cached_input_tokens = Some(
                                                simple.cached_input_tokens.unwrap_or(0)
                                                    + usage.cached_input_tokens,
                                            );
                                            simple.uncached_input_tokens = Some(
                                                simple.uncached_input_tokens.unwrap_or(0)
                                                    + usage.uncached_input_tokens,
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Could not read trace file {:?}: {}",
                                            trace_path,
                                            e
                                        );
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Could not read {:?}: {}", result_file, e);
                    }
                }
            }

            // if stats.total_results > 0 {
            //     stats_by_benchmark.insert(benchmark_name, stats);
            // }
        }

        Ok(())
    }

    fn parse_result_file(&self, path: &Path) -> Result<SimpleResult, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    fn calculate_tokens(&self, path: &Path) -> Result<TokenUsage, std::io::Error> {
        let mut usage = TokenUsage::default();
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut previous_input_tokens: u64 = 0;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            // Parse as LogEntry with nested message.usage
            if let Ok(entry) = serde_json::from_str::<LogEntry>(&line) {
                if let Some(message) = entry.message {
                    if let Some(msg_usage) = message.usage {
                        if let Some(input) = msg_usage.input_tokens {
                            usage.input_tokens += input;
                            let new_input = input.saturating_sub(previous_input_tokens);
                            if new_input > 0 {
                                usage.uncached_input_tokens += new_input;
                                usage.cached_input_tokens += input.saturating_sub(new_input);
                            } else {
                                usage.uncached_input_tokens += input;
                            }
                            previous_input_tokens = input;
                        }
                        if let Some(output) = msg_usage.output_tokens {
                            usage.output_tokens += output;
                        }
                        // Add cache tokens directly from the trace
                        if let Some(cache_read) = msg_usage.cache_read_input_tokens {
                            usage.cached_input_tokens += cache_read;
                        }
                        if let Some(cache_create) = msg_usage.cache_creation_input_tokens {
                            usage.cached_input_tokens += cache_create;
                        }
                    }
                }
            }
        }

        Ok(usage)
    }

    fn generate_report(
        &self,
        all_results: &[SimpleResult],
        all_exercises: &HashSet<String>,
        stats_by_benchmark: &HashMap<String, BenchmarkStats>,
        results_by_benchmark: &HashMap<String, Vec<SimpleResult>>,
        results_by_exercise: &HashMap<String, Vec<SimpleResult>>,
        output_path: &str,
    ) -> Result<(), std::io::Error> {
        let mut markdown = String::new();

        // Summary header
        markdown.push_str("# Benchmark Results Summary\n\n");
        markdown.push_str("| Benchmark | Total Results | Success | Failed | Completion % | Total Duration | Tokens |\n");
        markdown.push_str("|-----------|---------------|---------|--------|---------------|----------------|--------|\n");

        // Sort benchmarks by completion percentage
        let mut sorted_benchmarks: Vec<_> = stats_by_benchmark.values().collect();
        sorted_benchmarks.sort_by(|a, b| {
            let cmp = b
                .completion_percent()
                .partial_cmp(&a.completion_percent())
                .unwrap();
            if cmp != std::cmp::Ordering::Equal {
                return cmp;
            }
            b.total_duration.partial_cmp(&a.total_duration).unwrap()
        });

        for stats in &sorted_benchmarks {
            markdown.push_str(&format!(
                "| [{}](#{}) | {} | {} | {} | {:.1}% | {} | {} |\n",
                stats.benchmark_name.replace('.', "_"),
                stats.benchmark_name.replace('.', "_"),
                stats.total_results,
                stats.success_results,
                stats.failed_results,
                stats.completion_percent(),
                Self::format_duration(stats.total_duration),
                Self::format_tokens(
                    stats.uncached_input_tokens,
                    stats.cached_input_tokens,
                    stats.output_tokens
                )
            ));
        }

        // Success rates per exercise
        markdown.push_str("\n# Success rates per exercise\n\n");
        markdown.push_str("| Exercise | Total Results | Success | Failed | Completion % | Total Duration | Tokens |\n");
        markdown.push_str("|----------|---------------|---------|--------|---------------|----------------|--------|\n");

        let mut exercise_stats: Vec<BenchmarkStats> = all_exercises
            .iter()
            .map(|name| {
                let mut stats = BenchmarkStats::new(name);
                for result in all_results {
                    if result.exercise_name() == name {
                        stats.total_results += 1;
                        if result.success {
                            stats.success_results += 1;
                        } else {
                            stats.failed_results += 1;
                        }
                        stats.total_duration += result.duration;
                        stats.input_tokens += result.input_tokens.unwrap_or(0);
                        stats.output_tokens += result.output_tokens.unwrap_or(0);
                        stats.cached_input_tokens += result.cached_input_tokens.unwrap_or(0);
                        stats.uncached_input_tokens += result.uncached_input_tokens.unwrap_or(0);
                    }
                }
                stats
            })
            .collect();

        exercise_stats.sort_by(|a, b| {
            let cmp = b
                .completion_percent()
                .partial_cmp(&a.completion_percent())
                .unwrap();
            if cmp != std::cmp::Ordering::Equal {
                return cmp;
            }
            b.total_duration.partial_cmp(&a.total_duration).unwrap()
        });

        for stats in &exercise_stats {
            markdown.push_str(&format!(
                "| [{}](#{}) | {} | {} | {} | {:.1}% | {} | {} |\n",
                stats.benchmark_name.replace('.', "_"),
                stats.benchmark_name.replace('.', "_"),
                stats.total_results,
                stats.success_results,
                stats.failed_results,
                stats.completion_percent(),
                Self::format_duration(stats.total_duration),
                Self::format_tokens(
                    stats.uncached_input_tokens,
                    stats.cached_input_tokens,
                    stats.output_tokens
                )
            ));
        }

        // Breakdown by benchmark
        for (benchmark_name, results) in results_by_benchmark {
            markdown.push_str(&format!("\n# {}\n\n", benchmark_name.replace('.', "_")));
            markdown.push_str("| Exercise | Success | Duration | Tokens |\n");
            markdown.push_str("|----------|---------|----------|--------|\n");

            for result in results {
                markdown.push_str(&format!(
                    "| [{}](#{}) | {} | {} | {} |\n",
                    result.exercise_name().replace('.', "_"),
                    result.exercise_name().replace('.', "_"),
                    if result.success {
                        "✅"
                    } else if result.duration >= 7199.0 {
                        "⏰"
                    } else {
                        "❌"
                    },
                    Self::format_duration(result.duration),
                    Self::format_tokens(
                        result.uncached_input_tokens.unwrap_or(0),
                        result.cached_input_tokens.unwrap_or(0),
                        result.output_tokens.unwrap_or(0)
                    )
                ));
            }
        }

        // Breakdown by exercise
        for (exercise, results) in results_by_exercise {
            let mut sorted_results = results.clone();
            sorted_results.sort_by(|a, b| a.duration.partial_cmp(&b.duration).unwrap());

            markdown.push_str(&format!("\n# {}\n\n", exercise.replace('.', "_")));
            markdown.push_str("| Model | Success | Duration | Tokens |\n");
            markdown.push_str("|-------|---------|----------|--------|\n");

            for result in &sorted_results {
                markdown.push_str(&format!(
                    "| [{}](#{}) | {} | {} | {} |\n",
                    result.model().replace('.', "_"),
                    result.model().replace('.', "_"),
                    if result.success {
                        "✅"
                    } else if Self::is_likely_timeout(result.duration) {
                        "⏰"
                    } else {
                        "❌"
                    },
                    Self::format_duration(result.duration),
                    Self::format_tokens(
                        result.uncached_input_tokens.unwrap_or(0),
                        result.cached_input_tokens.unwrap_or(0),
                        result.output_tokens.unwrap_or(0)
                    )
                ));
            }
        }

        markdown.push_str("\n*Generated by Benchmark Analyzer*\n");

        // Write to file
        let mut file = File::create(output_path)?;
        file.write_all(markdown.as_bytes())?;

        info!("Results written to {}", output_path);

        Ok(())
    }

    fn is_likely_timeout(duration: f64) -> bool {
        duration >= 7199.0
            || (duration > 1799.0 && duration < 1801.0)
            || (duration > 1199.0 && duration < 1201.0)
    }

    fn format_tokens(uncached: u64, cached: u64, output: u64) -> String {
        format!(
            "{} / {} / {}",
            Self::format_number(uncached),
            Self::format_number(cached),
            Self::format_number(output)
        )
    }

    fn format_number(num: u64) -> String {
        if num >= 1_000_000_000 {
            format!("{:.1}G", num as f64 / 1_000_000_000.0)
        } else if num >= 1_000_000 {
            format!("{:.1}M", num as f64 / 1_000_000.0)
        } else if num >= 1_000 {
            format!("{:.1}K", num as f64 / 1_000.0)
        } else {
            num.to_string()
        }
    }

    fn format_duration(total_seconds: f64) -> String {
        if total_seconds == 0.0 {
            return "0s".to_string();
        }

        let days = (total_seconds / 86400.0) as i32;
        let hours = ((total_seconds % 86400.0) / 3600.0) as i32;
        let minutes = ((total_seconds % 3600.0) / 60.0) as i32;
        let seconds = (total_seconds % 60.0) as i32;

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
}
