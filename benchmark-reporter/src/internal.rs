//! Internal report generation implementation.

use benchmark_types::exercise::ExerciseResult;
use serde::Deserialize;
use std::collections::{HashMap, BTreeSet};
use std::fs::{self, File};
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug)]
struct BenchmarkStats {
    benchmark_name: String,
    total_results: i64,
    success_results: i64,
    failed_results: i64,
    total_duration: f64,
    #[allow(dead_code)]
    exit_code: i32,
    input_tokens: u64,
    output_tokens: u64,
    cached_input_tokens: u64,
    uncached_input_tokens: u64,
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

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum PiLogEntry { #[serde(rename = "message")] Message(PiMessage), #[serde(other)] Other }
#[derive(Debug, Deserialize)]
struct PiMessage { message: Option<PiMessageData> }
#[derive(Debug, Deserialize)]
struct PiMessageData { usage: Option<PiUsage> }
#[derive(Debug, Deserialize)]
struct PiUsage { input: Option<i64>, output: Option<i64> }

fn calculate_pi_tokens(trace_path: &Path) -> (i64, i64, i64, i64) {
    let mut input = 0i64;
    let mut output = 0i64;
    if let Ok(file) = File::open(trace_path) {
        for line in BufReader::new(file).lines().flatten() {
            if let Ok(PiLogEntry::Message(msg)) = serde_json::from_str::<PiLogEntry>(line.trim()) {
                if let Some(ref data) = msg.message {
                    if let Some(ref usage) = data.usage {
                        input += usage.input.unwrap_or(0);
                        output += usage.output.unwrap_or(0);
                    }
                }
            }
        }
    }
    (input, output, 0, 0)
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum LogEntry { #[serde(rename = "assistant")] Assistant(AssistantEntry), #[serde(rename = "user")] User(UserEntry), #[serde(other)] Other }
#[derive(Debug, Deserialize)]
struct AssistantEntry { message: Option<Message> }
#[derive(Debug, Deserialize)]
struct UserEntry { message: Option<Message> }
#[derive(Debug, Deserialize)]
struct Message { usage: Option<Usage> }
#[derive(Debug, Deserialize)]
struct Usage { input_tokens: u64, output_tokens: u64 }

fn calculate_claude_tokens(trace_path: &Path) -> (i64, i64, i64, i64) {
    let mut input = 0i64;
    let mut output = 0i64;
    let mut cached = 0i64;
    let mut uncached = 0i64;
    let mut prev_input: u64 = 0;
    if let Ok(file) = File::open(trace_path) {
        for line in BufReader::new(file).lines().flatten() {
            if let Ok(entry) = serde_json::from_str::<LogEntry>(line.trim()) {
                let msg = match &entry { LogEntry::Assistant(a) => a.message.as_ref(), LogEntry::User(u) => u.message.as_ref(), _ => continue };
                if let Some(m) = msg {
                    if let Some(ref usage) = m.usage {
                        input += usage.input_tokens as i64;
                        output += usage.output_tokens as i64;
                        let new = usage.input_tokens.saturating_sub(prev_input);
                        if new > 0 { uncached += new as i64; cached += (usage.input_tokens - new) as i64; } else { uncached += usage.input_tokens as i64; }
                        prev_input = usage.input_tokens;
                    }
                }
            }
        }
    }
    (input, output, cached, uncached)
}

struct PathTime { path: PathBuf, file_time: SystemTime }
impl PartialEq for PathTime { fn eq(&self, other: &Self) -> bool { self.path == other.path && self.file_time == other.file_time } }
impl Eq for PathTime {}
impl PartialOrd for PathTime { fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) } }
impl Ord for PathTime { fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.file_time.cmp(&other.file_time) } }

fn is_result_file(name: &str) -> bool {
    if !name.ends_with(".json") { return false; }
    let n = name;
    if n.starts_with("result_pi_") || n.starts_with("result_claude") || n.starts_with("result_reference") {
        if !regex::Regex::new(r"result_claude_\d{8}_\d{6}\.json").unwrap().is_match(n) { return true; }
    }
    false
}

fn format_number(num: i64) -> String {
    let abs = num.unsigned_abs();
    if abs >= 1_000_000_000 { format!("{:.1}G", num as f64 / 1_000_000_000.0) }
    else if abs >= 1_000_000 { format!("{:.1}M", num as f64 / 1_000_000.0) }
    else if abs >= 1_000 { format!("{:.1}K", num as f64 / 1_000.0) }
    else { num.to_string() }
}

fn format_duration(total_seconds: f64) -> String {
    if total_seconds == 0.0 { return "0s".to_string(); }
    let days = (total_seconds / 86400.0) as i64;
    let hours = ((total_seconds % 86400.0) / 3600.0) as i64;
    let minutes = ((total_seconds % 3600.0) / 60.0) as i64;
    let seconds = (total_seconds % 60.0) as i64;
    let mut parts = Vec::new();
    if days > 0 { parts.push(format!("{}d", days)); }
    if hours > 0 { parts.push(format!("{}h", hours)); }
    if minutes > 0 { parts.push(format!("{}m", minutes)); }
    parts.push(format!("{}s", seconds));
    parts.join(" ")
}

fn format_tokens(input: u64, cached: u64, output: u64) -> String {
    format!(
        "{} / {} / {}",
        format_number(input as i64),
        format_number(cached as i64),
        format_number(output as i64)
    )
}

fn sort_by_count_and_percentage(a: &BenchmarkStats, b: &BenchmarkStats) -> std::cmp::Ordering {
    let cmp = b.total_results.cmp(&a.total_results);
    if cmp != std::cmp::Ordering::Equal { return cmp; }
    if a.total_results == 0 && b.total_results == 0 { return std::cmp::Ordering::Equal; }
    let comp_a = (a.success_results as f64 * 100.0) / a.total_results as f64;
    let comp_b = (b.success_results as f64 * 100.0) / b.total_results as f64;
    let cmp = comp_b.partial_cmp(&comp_a).unwrap_or(std::cmp::Ordering::Equal);
    if cmp != std::cmp::Ordering::Equal { return cmp; }
    a.total_duration.partial_cmp(&b.total_duration).unwrap_or(std::cmp::Ordering::Equal)
}

fn sort_by_percentage(a: &BenchmarkStats, b: &BenchmarkStats) -> std::cmp::Ordering {
    if a.total_results == 0 && b.total_results == 0 { return std::cmp::Ordering::Equal; }
    let comp_a = (a.success_results as f64 * 100.0) / a.total_results as f64;
    let comp_b = (b.success_results as f64 * 100.0) / b.total_results as f64;
    let cmp = comp_b.partial_cmp(&comp_a).unwrap_or(std::cmp::Ordering::Equal);
    if cmp != std::cmp::Ordering::Equal { return cmp; }
    a.total_duration.partial_cmp(&b.total_duration).unwrap_or(std::cmp::Ordering::Equal)
}

fn dump_sorted_stats(sorted_stats: &[BenchmarkStats], markdown: &mut String) {
    for stats in sorted_stats {
        if stats.total_results == 0 { continue; }
        let completion_percent = (stats.success_results as f64 * 100.0) / stats.total_results as f64;
        let duration_str = format_duration(stats.total_duration);
        let display_name = stats.benchmark_name.replace('.', "_").replace(':', "-");
        markdown.push_str(&format!("| [{}](#{}) | {} | {} | {} | {:.1}% | {} | {} |\n", display_name, display_name, stats.total_results, stats.success_results, stats.failed_results, completion_percent, duration_str, format_tokens(stats.uncached_input_tokens, stats.cached_input_tokens, stats.output_tokens)));
    }
}

fn process_results_directory(
    dir: &Path,
    all_results: &mut Vec<ExerciseResult>,
    all_exercises: &mut BTreeSet<String>,
    stats_by_benchmark: &mut HashMap<String, BenchmarkStats>,
    results_by_benchmark: &mut HashMap<String, Vec<ExerciseResult>>,
    results_by_exercise: &mut HashMap<String, Vec<ExerciseResult>>,
) {
    let mut result_files: Vec<PathTime> = match fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).filter(|e| is_result_file(&e.file_name().to_string_lossy())).filter_map(|e| { let path = e.path(); let file_time = fs::metadata(&path).ok()?.modified().ok()?; Some(PathTime { path, file_time }) }).collect(),
        Err(_) => return,
    };
    result_files.sort();
    let mut trace_files: Vec<PathTime> = match fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl")).filter_map(|e| { let path = e.path(); let file_time = fs::metadata(&path).ok()?.modified().ok()?; Some(PathTime { path, file_time }) }).collect(),
        Err(_) => Vec::new(),
    };
    trace_files.sort();
    if result_files.is_empty() { return; }

    let benchmark_name = dir.file_name().unwrap().to_string_lossy().to_string();
    let mut stats = BenchmarkStats::new(&benchmark_name);

    for pt in &result_files {
        let result_file = &pt.path;
        let content = match fs::read_to_string(result_file) { Ok(c) => c, Err(_) => continue };
        let mut result: ExerciseResult = match serde_json::from_str(&content) {
            Ok(r) => r,
            Err(_) => continue,
        };
        result.model = benchmark_name.clone();
        let exercise_key = format!("{}_{}", result.exercise_name, result.language);
        all_exercises.insert(exercise_key.clone());
        all_results.push(result.clone());

        stats.total_results += 1;
        if result.success {
            stats.success_results += 1;
        } else {
            stats.failed_results += 1;
        }
        // Convert duration from seconds (f64) to ms (u64) for display
        let duration_secs = result.duration_ms as f64 / 1000.0;
        stats.total_duration += duration_secs;
        stats.exit_code = result.exit_code.unwrap_or(0);
        if stats.exit_code != 0 && result.success {
            stats.success_results -= 1;
            stats.failed_results += 1;
        }

        let result_time = match fs::metadata(result_file).and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let mut result_with_tokens = result.clone();

        while !trace_files.is_empty()
            && result_time.cmp(&trace_files[0].file_time) == std::cmp::Ordering::Greater
        {
            let trace_path = trace_files.remove(0).path;
            let usage = if result_with_tokens.model.starts_with("pi") {
                calculate_pi_tokens(&trace_path)
            } else {
                calculate_claude_tokens(&trace_path)
            };
            stats.input_tokens += usage.0 as u64;
            stats.output_tokens += usage.1 as u64;
            stats.cached_input_tokens += usage.2 as u64;
            stats.uncached_input_tokens += usage.3 as u64;
            result_with_tokens.input_tokens += usage.0 as u64;
            result_with_tokens.output_tokens += usage.1 as u64;
            result_with_tokens.cached_input_tokens += usage.2 as u64;
            result_with_tokens.uncached_input_tokens += usage.3 as u64;
        }

        results_by_benchmark
            .entry(benchmark_name.clone())
            .or_default()
            .push(result_with_tokens.clone());
        results_by_exercise.entry(exercise_key).or_default().push(result_with_tokens);
    }
    if stats.total_results > 0 {
        stats_by_benchmark.insert(benchmark_name, stats);
    }
}

pub fn run_report(base_dir: &Path, output: &str) -> anyhow::Result<()> {
    let mut all_results: Vec<ExerciseResult> = Vec::new();
    let mut all_exercises: BTreeSet<String> = BTreeSet::new();
    let mut stats_by_benchmark: HashMap<String, BenchmarkStats> = HashMap::new();
    let mut results_by_exercise: HashMap<String, Vec<ExerciseResult>> = HashMap::new();
    let mut results_by_benchmark: HashMap<String, Vec<ExerciseResult>> = HashMap::new();

    for entry in walkdir::WalkDir::new(base_dir) {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        if entry.file_type().is_dir() {
            process_results_directory(entry.path(), &mut all_results, &mut all_exercises, &mut stats_by_benchmark, &mut results_by_benchmark, &mut results_by_exercise);
        }
    }

    let mut sorted_stats: Vec<BenchmarkStats> = stats_by_benchmark.into_values().collect();
    sorted_stats.sort_by(sort_by_count_and_percentage);

    let mut markdown = String::new();
    markdown.push_str("# Benchmark Results Summary\n\n");
    markdown.push_str("| Benchmark | Total Results | Success | Failed | Completion % | Total Duration | Input / Cached / Output |\n");
    markdown.push_str("|-----------|---------------|---------|--------|---------------|----------------|---------------------------|\n");
    dump_sorted_stats(&sorted_stats, &mut markdown);

    markdown.push_str("\n# Success rates per exercise\n\n");
    markdown.push_str("| Exercise | Total Results | Success | Failed | Completion % | Total Duration | Input / Cached / Output |\n");
    markdown.push_str("|----------|---------------|---------|--------|---------------|----------------|---------------------------|\n");

    let mut exercise_stats: Vec<BenchmarkStats> = Vec::new();
    for exercise_name in &all_exercises {
        let mut stats = BenchmarkStats::new(exercise_name);
        for result in &all_results {
            let key = format!("{}_{}", result.exercise_name, result.language);
            if key == *exercise_name {
                stats.total_results += 1;
                if result.success {
                    stats.success_results += 1;
                } else {
                    stats.failed_results += 1;
                }
                stats.total_duration += result.duration_ms as f64 / 1000.0;
                stats.input_tokens += result.input_tokens;
                stats.output_tokens += result.output_tokens;
                stats.cached_input_tokens += result.cached_input_tokens;
                stats.uncached_input_tokens += result.uncached_input_tokens;
            }
        }
        exercise_stats.push(stats);
    }
    exercise_stats.sort_by(sort_by_percentage);
    dump_sorted_stats(&exercise_stats, &mut markdown);

    for (benchmark_name, results) in &results_by_benchmark {
        markdown.push_str("\n");
        let display_name = benchmark_name.replace('.', "_").replace(':', "-");
        markdown.push_str(&format!("# {}\n\n", display_name));
        markdown.push_str("| Exercise | Success | Duration | Input / Cached / Output |\n|----------|---------|----------|---------------------------|\n");
        for result in results {
            let exercise_display = result.exercise_name.replace('.', "_") + "_" + &result.language;
            // Convert ms to seconds for timeout check
            let duration_secs = result.duration_ms as f64 / 1000.0;
            let success_indicator = if result.success {
                "✅"
            } else if duration_secs >= 7199.0 {
                "⏰"
            } else {
                "❌"
            };
            markdown.push_str(&format!(
                "| [{}](#{}) | {} | {} | {} |\n",
                exercise_display,
                exercise_display,
                success_indicator,
                format_duration(duration_secs),
                format_tokens(
                    result.uncached_input_tokens,
                    result.cached_input_tokens,
                    result.output_tokens
                )
            ));
        }
    }

    for (exercise, results) in &results_by_exercise {
        markdown.push_str("\n");
        let display_name = exercise.replace('.', "_");
        markdown.push_str(&format!("# {}\n\n", display_name));
        markdown.push_str("| Model | Success | Duration | Input / Cached / Output |\n|-------|---------|----------|---------------------------|\n");
        let mut sorted_results = results.clone();
        // Sort by duration_ms (already in ms)
        sorted_results.sort_by(|a, b| a.duration_ms.cmp(&b.duration_ms));
        for result in sorted_results {
            let model_display = result.model.replace('.', "_");
            let success_indicator = if result.success { "✅" } else { "❌" };
            let duration_secs = result.duration_ms as f64 / 1000.0;
            markdown.push_str(&format!(
                "| [{}](#{}) | {} | {} | {} |\n",
                model_display,
                model_display,
                success_indicator,
                format_duration(duration_secs),
                format_tokens(
                    result.uncached_input_tokens,
                    result.cached_input_tokens,
                    result.output_tokens
                )
            ));
        }
    }

    markdown.push_str("\n*Generated by BenchmarkResultAnalyzer*\n");
    fs::write(output, &markdown)?;
    println!("Results written to {}", output);
    println!();
    print!("{}", markdown);
    Ok(())
}
