use serde::{Deserialize, Deserializer};
use std::collections::{HashMap, BTreeSet};
use std::fs::{self, File};
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

// =============================================================================
// Data structures mirroring the Java classes exactly
// =============================================================================

#[derive(Debug, Deserialize, Clone)]
struct SimpleResult {
    #[serde(alias = "exerciseName", alias = "exercise_name")]
    exercise_name: String,
    language: String,
    success: bool,
    #[serde(alias = "exitCode", alias = "exit_code", default)]
    exit_code: i64,
    #[allow(dead_code)]
    output: Option<String>,
    #[serde(alias = "duration_ms", default)]
    duration: f64,
    #[serde(deserialize_with = "deserialize_start_end", default)]
    #[allow(dead_code)]
    start_time: Option<String>,
    #[serde(deserialize_with = "deserialize_start_end", default)]
    #[allow(dead_code)]
    end_time: Option<String>,
    #[allow(dead_code)]
    error_message: Option<String>,
    #[allow(dead_code)]
    trace: Option<String>,
    #[serde(default)]
    model: String,
    #[allow(dead_code)]
    attempts: u32,
    // Token fields (populated from trace files)
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    cached_input_tokens: i64,
    #[serde(default)]
    uncached_input_tokens: i64,
}

impl Default for SimpleResult {
    fn default() -> Self {
        SimpleResult {
            exercise_name: String::new(),
            language: String::new(),
            success: false,
            exit_code: 0,
            output: None,
            duration: 0.0,
            start_time: None,
            end_time: None,
            error_message: None,
            trace: None,
            model: String::new(),
            attempts: 0,
            input_tokens: 0,
            output_tokens: 0,
            cached_input_tokens: 0,
            uncached_input_tokens: 0,
        }
    }
}

#[derive(Debug)]
struct BenchmarkStats {
    benchmark_name: String,
    total_results: i64,
    success_results: i64,
    failed_results: i64,
    total_duration: f64,
    #[allow(dead_code)]
    exit_code: i32,
    input_tokens: i64,
    output_tokens: i64,
    cached_input_tokens: i64,
    uncached_input_tokens: i64,
}

impl BenchmarkStats {
    fn new(name: &str) -> Self {
        BenchmarkStats {
            benchmark_name: name.to_string(),
            total_results: 0,
            success_results: 0,
            failed_results: 0,
            total_duration: 0.0,
            exit_code: 0,
            input_tokens: 0,
            output_tokens: 0,
            cached_input_tokens: 0,
            uncached_input_tokens: 0,
        }
    }
}

// =============================================================================
// Pi trace parser (direct port of calculatePiTokens)
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

fn calculate_pi_tokens(trace_path: &Path) -> (i64, i64, i64, i64) {
    let mut input_tokens = 0i64;
    let mut output_tokens = 0i64;

    let file = match File::open(trace_path) {
        Ok(f) => f,
        Err(_) => return (0, 0, 0, 0),
    };
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(PiLogEntry::Message(msg)) = serde_json::from_str::<PiLogEntry>(trimmed) {
            if let Some(ref data) = msg.message {
                if let Some(ref usage) = data.usage {
                    input_tokens += usage.input.unwrap_or(0);
                    output_tokens += usage.output.unwrap_or(0);
                }
            }
        }
    }

    (input_tokens, output_tokens, 0, 0)
}

// =============================================================================
// Claude trace parser (direct port of calculateClaudeTokens)
// =============================================================================

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

fn calculate_claude_tokens(trace_path: &Path) -> (i64, i64, i64, i64) {
    let mut input_tokens = 0i64;
    let mut output_tokens = 0i64;
    let mut cached_input_tokens = 0i64;
    let mut uncached_input_tokens = 0i64;
    let mut previous_input_tokens: u64 = 0;

    let file = match File::open(trace_path) {
        Ok(f) => f,
        Err(_) => return (0, 0, 0, 0),
    };
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<LogEntry>(trimmed) {
            let message: Option<&Message> = match &entry {
                LogEntry::Assistant(a) => a.message.as_ref(),
                LogEntry::User(u) => u.message.as_ref(),
                LogEntry::Other => continue,
            };

            if let Some(msg) = message {
                if let Some(ref usage) = msg.usage {
                    input_tokens += usage.input_tokens as i64;
                    output_tokens += usage.output_tokens as i64;

                    // Calculate new input tokens (difference from previous)
                    let new_input_tokens = usage.input_tokens.saturating_sub(previous_input_tokens);
                    if new_input_tokens > 0 {
                        uncached_input_tokens += new_input_tokens as i64;
                        cached_input_tokens += (usage.input_tokens - new_input_tokens) as i64;
                    } else {
                        uncached_input_tokens += usage.input_tokens as i64;
                    }
                    previous_input_tokens = usage.input_tokens;
                }
            }
        }
    }

    (input_tokens, output_tokens, cached_input_tokens, uncached_input_tokens)
}

// =============================================================================
// File metadata for sorting by modification time
// =============================================================================

struct PathTime {
    path: PathBuf,
    file_time: SystemTime,
}

impl PartialEq for PathTime {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.file_time == other.file_time
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

// =============================================================================
// Main logic (direct port of processResultsDirectory + generateReport)
// =============================================================================

fn is_result_file(name: &str) -> bool {
    if !name.ends_with(".json") {
        return false;
    }
    // Must start with result_pi_, result_claude_, or result_reference_
    let n = name;
    if n.starts_with("result_pi_") || n.starts_with("result_claude") || n.starts_with("result_reference") {
        // Exclude timestamped files: result_claude_YYYYMMDD_HHMMSS.json
        if !regex::Regex::new(r"result_claude_\d{8}_\d{6}\.json").unwrap().is_match(n) {
            return true;
        }
    }
    false
}

fn process_results_directory(
    dir: &Path,
    all_results: &mut Vec<SimpleResult>,
    all_exercises: &mut BTreeSet<String>,
    stats_by_benchmark: &mut HashMap<String, BenchmarkStats>,
    results_by_benchmark: &mut HashMap<String, Vec<SimpleResult>>,
    results_by_exercise: &mut HashMap<String, Vec<SimpleResult>>,
) {
    // Collect result files
    let mut result_files: Vec<PathTime> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let fname = e.file_name().to_string_lossy().into_owned();
                is_result_file(&fname)
            })
            .filter_map(|e| {
                let path = e.path();
                let file_time = fs::metadata(&path).ok()?.modified().ok()?;
                Some(PathTime { path, file_time })
            })
            .collect(),
        Err(_) => return,
    };
    result_files.sort();

    // Collect trace files
    let mut trace_files: Vec<PathTime> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
            .filter_map(|e| {
                let path = e.path();
                let file_time = fs::metadata(&path).ok()?.modified().ok()?;
                Some(PathTime { path, file_time })
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    trace_files.sort();

    if result_files.is_empty() {
        return;
    }

    let benchmark_name = dir.file_name().unwrap().to_string_lossy().to_string();
    let mut stats = BenchmarkStats::new(&benchmark_name);

    for pt in &result_files {
        let result_file = &pt.path;
        let content = match fs::read_to_string(result_file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: Could not read {}: {}", result_file.display(), e);
                continue;
            }
        };

        let mut simple: SimpleResult = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Warning: Could not parse {}: {}", result_file.display(), e);
                continue;
            }
        };

        // model is set to benchmarkName (the directory name)
        simple.model = benchmark_name.clone();

        let exercise_key = format!("{}_{}", simple.exercise_name, simple.language);
        all_exercises.insert(exercise_key.clone());
        all_results.push(simple.clone());  // Keep original (zero tokens) in all_results for per-exercise stats

        stats.total_results += 1;
        if simple.success {
            stats.success_results += 1;
        } else {
            stats.failed_results += 1;
        }
        stats.total_duration += simple.duration;
        stats.exit_code = simple.exit_code as i32;

        // If exit code != 0 but success was true, flip the outcome
        if simple.exit_code != 0 && simple.success {
            stats.success_results -= 1;
            stats.failed_results += 1;
        }

        // Process trace files that were modified after this result file
        let result_time = match fs::metadata(result_file).and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };

        // Clone for trace processing — this copy gets updated with token counts
        let mut simple_with_tokens = simple.clone();

        while !trace_files.is_empty() && result_time.cmp(&trace_files[0].file_time) == std::cmp::Ordering::Greater {
            let trace_path = trace_files.remove(0).path;
            let usage = if simple_with_tokens.model.starts_with("pi") {
                calculate_pi_tokens(&trace_path)
            } else {
                calculate_claude_tokens(&trace_path)
            };

            stats.input_tokens += usage.0;
            stats.output_tokens += usage.1;
            stats.cached_input_tokens += usage.2;
            stats.uncached_input_tokens += usage.3;

            simple_with_tokens.input_tokens += usage.0;
            simple_with_tokens.output_tokens += usage.1;
            simple_with_tokens.cached_input_tokens += usage.2;
            simple_with_tokens.uncached_input_tokens += usage.3;
        }

        // Push results with updated token counts (after trace processing)
        let sw = simple_with_tokens;
        results_by_benchmark.entry(benchmark_name.clone()).or_default().push(sw.clone());
        results_by_exercise.entry(exercise_key).or_default().push(sw);
    }

    if stats.total_results > 0 {
        stats_by_benchmark.insert(benchmark_name, stats);
    }
}

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

/// Deserialize start/end time that can be either a string or a number.
fn deserialize_start_end<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde_json::Value;
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.map(|v| match v {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }))
}

fn format_duration(total_seconds: f64) -> String {
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

fn format_tokens(uncached: i64, cached: i64, output: i64) -> String {
    format!("{} / {} / {}", format_number(uncached), format_number(cached), format_number(output))
}

fn sort_by_count_and_percentage(a: &BenchmarkStats, b: &BenchmarkStats) -> std::cmp::Ordering {
    // Sort by total results descending
    let cmp = b.total_results.cmp(&a.total_results);
    if cmp != std::cmp::Ordering::Equal {
        return cmp;
    }
    // Then by completion percentage descending (guard against zero)
    if a.total_results == 0 && b.total_results == 0 {
        return std::cmp::Ordering::Equal;
    }
    let comp_a = (a.success_results as f64 * 100.0) / a.total_results as f64;
    let comp_b = (b.success_results as f64 * 100.0) / b.total_results as f64;
    let cmp = comp_b.partial_cmp(&comp_a).unwrap_or(std::cmp::Ordering::Equal);
    if cmp != std::cmp::Ordering::Equal {
        return cmp;
    }
    // Then by total duration ascending
    a.total_duration.partial_cmp(&b.total_duration).unwrap_or(std::cmp::Ordering::Equal)
}

fn sort_by_percentage(a: &BenchmarkStats, b: &BenchmarkStats) -> std::cmp::Ordering {
    if a.total_results == 0 && b.total_results == 0 {
        return std::cmp::Ordering::Equal;
    }
    let comp_a = (a.success_results as f64 * 100.0) / a.total_results as f64;
    let comp_b = (b.success_results as f64 * 100.0) / b.total_results as f64;
    let cmp = comp_b.partial_cmp(&comp_a).unwrap_or(std::cmp::Ordering::Equal);
    if cmp != std::cmp::Ordering::Equal {
        return cmp;
    }
    a.total_duration.partial_cmp(&b.total_duration).unwrap_or(std::cmp::Ordering::Equal)
}

fn dump_sorted_stats(sorted_stats: &[BenchmarkStats], markdown: &mut String) {
    for stats in sorted_stats {
        if stats.total_results == 0 {
            continue;
        }
        let completion_percent = (stats.success_results as f64 * 100.0) / stats.total_results as f64;
        let duration_str = format_duration(stats.total_duration);
        let display_name = stats.benchmark_name.replace('.', "_").replace(':', "-");
        markdown.push_str(&format!(
            "| [{}](#{}) | {} | {} | {} | {:.1}% | {} | {} |\n",
            display_name,
            display_name,
            stats.total_results,
            stats.success_results,
            stats.failed_results,
            completion_percent,
            duration_str,
            format_tokens(stats.uncached_input_tokens, stats.cached_input_tokens, stats.output_tokens)
        ));
    }
}

fn main() -> anyhow::Result<()> {
    let base_dir = PathBuf::from("../benchmark-results");

    if !base_dir.exists() {
        eprintln!("Error: Results directory not found: {}", base_dir.display());
        std::process::exit(1);
    }

    let mut all_results: Vec<SimpleResult> = Vec::new();
    let mut all_exercises: BTreeSet<String> = BTreeSet::new();
    let mut stats_by_benchmark: HashMap<String, BenchmarkStats> = HashMap::new();
    let mut results_by_exercise: HashMap<String, Vec<SimpleResult>> = HashMap::new();
    let mut results_by_benchmark: HashMap<String, Vec<SimpleResult>> = HashMap::new();

    // Walk the directory tree and process each subdirectory
    for entry in walkdir::WalkDir::new(&base_dir) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.file_type().is_dir() {
            process_results_directory(
                entry.path(),
                &mut all_results,
                &mut all_exercises,
                &mut stats_by_benchmark,
                &mut results_by_benchmark,
                &mut results_by_exercise,
            );
        }
    }


    // Sort benchmarks by count and percentage
    let mut sorted_stats: Vec<BenchmarkStats> = stats_by_benchmark.into_values().collect();
    sorted_stats.sort_by(sort_by_count_and_percentage);

    // Generate markdown
    let mut markdown = String::new();

    // === Section 1: Benchmark Summary ===
    markdown.push_str("# Benchmark Results Summary\n\n");
    markdown.push_str("| Benchmark | Total Results | Success | Failed | Completion % | Total Duration | Tokens |\n");
    markdown.push_str("|-----------|---------------|---------|--------|---------------|----------------|--------|\n");
    dump_sorted_stats(&sorted_stats, &mut markdown);

    // === Section 2: Success rates per exercise ===
    markdown.push_str("\n");
    markdown.push_str("# Success rates per exercise\n\n");
    markdown.push_str("| Exercise | Total Results | Success | Failed | Completion % | Total Duration | Tokens |\n");
    markdown.push_str("|----------|---------------|---------|--------|---------------|----------------|--------|\n");

    let mut exercise_stats: Vec<BenchmarkStats> = Vec::new();
    for exercise_name in &all_exercises {
        let mut stats = BenchmarkStats::new(exercise_name);
        for simple_result in &all_results {
            let key = format!("{}_{}", simple_result.exercise_name, simple_result.language);
            if key == *exercise_name {
                stats.total_results += 1;
                if simple_result.success {
                    stats.success_results += 1;
                } else {
                    stats.failed_results += 1;
                }
                stats.total_duration += simple_result.duration;
                stats.input_tokens += simple_result.input_tokens;
                stats.output_tokens += simple_result.output_tokens;
                stats.cached_input_tokens += simple_result.cached_input_tokens;
                stats.uncached_input_tokens += simple_result.uncached_input_tokens;
            }
        }
        exercise_stats.push(stats);
    }

    exercise_stats.sort_by(sort_by_percentage);
    dump_sorted_stats(&exercise_stats, &mut markdown);

    // === Section 3: Breakdown per benchmark (model configuration) ===
    for (benchmark_name, results) in &results_by_benchmark {
        markdown.push_str("\n");
        let display_name = benchmark_name.replace('.', "_").replace(':', "-");
        markdown.push_str(&format!("# {}\n\n", display_name));
        markdown.push_str("| Exercise | Success | Duration | Tokens |\n");
        markdown.push_str("|----------|---------|----------|--------|\n");

        for simple_result in results {
            let exercise_display = simple_result.exercise_name.replace('.', "_") + "_" + &simple_result.language;
            let success_indicator = if simple_result.success {
                "✅"
            } else if simple_result.duration >= 7199.0 {
                "⏰"
            } else {
                "❌"
            };
            markdown.push_str(&format!(
                "| [{}](#{}) | {} | {} | {} |\n",
                exercise_display,
                exercise_display,
                success_indicator,
                format_duration(simple_result.duration),
                format_tokens(simple_result.uncached_input_tokens, simple_result.cached_input_tokens, simple_result.output_tokens)
            ));
        }
        markdown.push_str("\n");
    }

    // === Section 4: Breakdown per exercise (how long each model took) ===
    for (exercise, results) in &results_by_exercise {
        markdown.push_str("\n");
        let display_name = exercise.replace('.', "_");
        markdown.push_str(&format!("# {}\n\n", display_name));
        markdown.push_str("| Model | Success | Duration | Tokens |\n");
        markdown.push_str("|-------|---------|----------|--------|\n");

        // Sort by duration ascending
        let mut sorted_results = results.clone();
        sorted_results.sort_by(|a, b| a.duration.partial_cmp(&b.duration).unwrap_or(std::cmp::Ordering::Equal));

        for simple_result in sorted_results {
            let model_display = simple_result.model.replace('.', "_");
            let success_indicator = if simple_result.success { "✅" } else { "❌" };
            markdown.push_str(&format!(
                "| [{}](#{}) | {} | {} | {} |\n",
                model_display,
                model_display,
                success_indicator,
                format_duration(simple_result.duration),
                format_tokens(simple_result.uncached_input_tokens, simple_result.cached_input_tokens, simple_result.output_tokens)
            ));
        }
        markdown.push_str("\n");
    }

    // === Footer ===
    markdown.push_str("\n*Generated by BenchmarkResultAnalyzer*\n");

    // Write to results.md (same as Java version)
    fs::write("results.md", &markdown)?;
    println!("Results written to results.md");
    println!();
    print!("{}", markdown);

    Ok(())
}
