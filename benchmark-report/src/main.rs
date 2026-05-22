use benchmark_types::model::{LogEntry, PiLogEntry};
use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Parse benchmark result files and JSONL trace files, reporting token statistics.
#[derive(Parser, Debug)]
#[command(name = "benchmark-report")]
struct Cli {
    /// Path to the benchmark results directory (default: ../benchmark-results)
    #[arg(short, long, default_value = "../benchmark-results")]
    results_dir: PathBuf,

    /// Only include results matching this agent name (e.g., "claude", "pi", "reference")
    #[arg(short, long)]
    agent: Option<String>,

    /// Only include results matching this language
    #[arg(short, long)]
    language: Option<String>,

    /// Only include results matching this model
    #[arg(short, long)]
    model: Option<String>,

    /// Only include results matching this exercise name
    #[arg(short, long)]
    exercise: Option<String>,

    /// Show per-exercise details
    #[arg(short, long)]
    details: bool,

    /// Output as JSON instead of human-readable table
    #[arg(short, long)]
    json: bool,
}

#[derive(Debug, Default)]
struct ResultFile {
    exercise_name: String,
    language: String,
    agent: String,
    model: String,
    success: bool,
    duration: f64,
    attempts: u32,
    exit_code: i32,
    jsonl_path: Option<PathBuf>,
}

#[derive(Debug, Default, Clone)]
struct TraceStats {
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_cache_read: u64,
    total_cache_write: u64,
    total_tokens: u64,
    total_cost: f64,
    message_count: u64,
    tool_use_count: u64,
    thinking_count: u64,
    thinking_tokens: u64,
    model_changes: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let start = Instant::now();

    if !cli.results_dir.exists() {
        eprintln!("Error: Results directory not found: {}", cli.results_dir.display());
        std::process::exit(1);
    }

    let mut result_files = collect_result_files(&cli.results_dir)?;

    if let Some(agent) = &cli.agent {
        result_files.retain(|r| r.agent == *agent);
    }
    if let Some(language) = &cli.language {
        result_files.retain(|r| r.language == *language);
    }
    if let Some(model) = &cli.model {
        result_files.retain(|r| r.model == *model);
    }
    if let Some(exercise) = &cli.exercise {
        result_files.retain(|r| r.exercise_name == *exercise);
    }

    result_files.sort_by(|a, b| {
        a.agent.cmp(&b.agent).then(a.language.cmp(&b.language)).then(a.exercise_name.cmp(&b.exercise_name))
    });

    let mut trace_stats_by_file: HashMap<String, TraceStats> = HashMap::new();
    let mut files_with_traces = 0;
    let mut files_without_traces = 0;
    let mut files_skipped_html = 0;

    // Aggregate token stats directly by model (using result file's model field)
    let mut model_stats: HashMap<String, (TraceStats, usize)> = HashMap::new();

    for rf in &result_files {
        if let Some(trace_path) = &rf.jsonl_path {
            if trace_path.exists() && is_jsonl_file(trace_path) {
                let stats = parse_trace_file(trace_path);
                let key = format!("{}/{}/{}", rf.agent, rf.language, rf.exercise_name);
                trace_stats_by_file.insert(key, stats.clone());

                // Aggregate by model
                let entry = model_stats.entry(rf.model.clone()).or_default();
                entry.0.total_input_tokens += stats.total_input_tokens;
                entry.0.total_output_tokens += stats.total_output_tokens;
                entry.0.total_cache_read += stats.total_cache_read;
                entry.0.total_cache_write += stats.total_cache_write;
                entry.0.total_tokens = entry.0.total_input_tokens + entry.0.total_output_tokens;
                entry.0.message_count += stats.message_count;
                entry.0.tool_use_count += stats.tool_use_count;
                entry.0.thinking_count += stats.thinking_count;
                entry.0.thinking_tokens += stats.thinking_tokens;
                entry.1 += 1; // trace count
                files_with_traces += 1;
            } else {
                if trace_path.exists() { files_skipped_html += 1; } else { files_without_traces += 1; }
            }
        } else {
            files_without_traces += 1;
        }
    }

    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_cache_read = 0u64;
    let mut total_cache_write = 0u64;
    let mut total_tokens = 0u64;
    let mut total_cost = 0.0;
    let mut total_messages = 0u64;
    let mut total_tool_uses = 0u64;
    let mut total_thinking = 0u64;
    let mut total_thinking_tokens = 0u64;

    // Totals from model_stats
    for (stats, _) in model_stats.values() {
        total_input += stats.total_input_tokens;
        total_output += stats.total_output_tokens;
        total_cache_read += stats.total_cache_read;
        total_cache_write += stats.total_cache_write;
        total_tokens += stats.total_tokens;
        total_cost += stats.total_cost;
        total_messages += stats.message_count;
        total_tool_uses += stats.tool_use_count;
        total_thinking += stats.thinking_count;
        total_thinking_tokens += stats.thinking_tokens;
    }

    let elapsed = start.elapsed();

    if cli.json {
        print_json_report(&result_files, &trace_stats_by_file, &model_stats,
            total_input, total_output, total_cache_read, total_cache_write,
            total_tokens, total_cost, total_messages, total_tool_uses,
            total_thinking, total_thinking_tokens,
            files_with_traces, files_without_traces, files_skipped_html, &elapsed);
    } else {
        print_human_report(&result_files, &trace_stats_by_file, &model_stats,
            total_input, total_output, total_cache_read, total_cache_write,
            total_tokens, total_cost, total_messages, total_tool_uses,
            total_thinking, total_thinking_tokens,
            files_with_traces, files_without_traces, files_skipped_html,
            &elapsed, cli.details);
    }

    Ok(())
}

fn collect_result_files(results_dir: &Path) -> anyhow::Result<Vec<ResultFile>> {
    let mut result_files = Vec::new();
    let entries = fs::read_dir(results_dir)?;
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() { continue; }
        let file_entries = fs::read_dir(&dir)?;
        for file_entry in file_entries.flatten() {
            let file_path = file_entry.path();
            let filename = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if filename.starts_with("result_") && filename.ends_with(".json") {
                if let Some(rf) = parse_result_filename(&filename, &dir) {
                    // Try multiple trace file naming patterns
                    let jsonl_path = find_trace_file_v2(&dir, &rf);
                    result_files.push(ResultFile { jsonl_path, ..rf });
                }
            }
        }
    }
    Ok(result_files)
}

fn parse_result_filename(filename: &str, dir: &Path) -> Option<ResultFile> {
    let without_ext = filename.strip_suffix(".json")?;
    let parts: Vec<&str> = without_ext.strip_prefix("result_")?.split('_').collect();
    if parts.len() < 3 { return None; }
    let agent = parts[0].to_string();
    let language = parts[1].to_string();
    let exercise_name = parts[2..].join("_");
    let file_path = dir.join(filename);
    let content = fs::read_to_string(&file_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let model = json.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    let duration = json.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let attempts = json.get("attempts").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    let exit_code = json.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
    Some(ResultFile { exercise_name, language, agent, model, success, duration, attempts, exit_code, jsonl_path: None })
}

/// Find trace file with improved heuristics:
/// 1. log_<agent>_<lang>_<exercise>.jsonl (Claude, some Pi runs)
/// 2. trace_<lang>_<exercise>.jsonl (Pi runs — no agent prefix)
/// 3. scan directory for any matching trace_*.jsonl file
fn find_trace_file_v2(dir: &Path, rf: &ResultFile) -> Option<PathBuf> {
    // Pattern 1: log_<agent>_<lang>_<exercise>.jsonl
    let p1 = dir.join(format!("log_{}_{}.jsonl", rf.agent, rf.language));
    if p1.exists() && is_jsonl_file(&p1) { return Some(p1); }

    // Pattern 2: trace_<lang>_<exercise>.jsonl (Pi naming convention — no agent prefix)
    let exercise_stem = rf.exercise_name.replace('-', "-");
    let p2 = dir.join(format!("trace_{}_{}.jsonl", rf.language, exercise_stem));
    if p2.exists() && is_jsonl_file(&p2) { return Some(p2); }

    // Pattern 3: trace_<agent>_<lang>_<exercise>.jsonl (fallback)
    let p3 = dir.join(format!("trace_{}_{}.jsonl", rf.agent, exercise_stem));
    if p3.exists() && is_jsonl_file(&p3) { return Some(p3); }

    // Pattern 4: Scan directory for trace_<lang>_*.jsonl files
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&format!("trace_{}_{}", rf.language, exercise_stem))
                && is_jsonl_file(&entry.path()) {
                return Some(entry.path());
            }
        }
    }

    None
}

fn is_jsonl_file(path: &Path) -> bool {
    if let Ok(mut file) = fs::File::open(path) {
        let mut buffer = [0u8; 10];
        if file.read(&mut buffer).is_ok() {
            let content = String::from_utf8_lossy(&buffer);
            let trimmed = content.trim();
            return trimmed.starts_with('{') || trimmed.starts_with('[');
        }
    }
    false
}

/// Determine which parser to use based on the filename.
/// log_*.jsonl → Claude (LogEntry)
/// trace_*.jsonl → Pi (PiLogEntry)
fn is_claude_trace(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    name.starts_with("log_")
}

/// Parse a trace file. Uses filename to determine format:
/// - log_*.jsonl files use Claude LogEntry deserialization with cumulative delta calculation
/// - trace_*.jsonl files use Pi PiLogEntry deserialization with direct values
fn parse_trace_file(path: &Path) -> TraceStats {
    let content = match fs::read_to_string(path) { Ok(c) => c, Err(_) => return TraceStats::default() };

    if is_claude_trace(path) {
        parse_claude_trace(&content)
    } else {
        parse_pi_trace(&content)
    }
}

/// Parse Claude trace file with cumulative token delta calculation.
/// Claude logs report cumulative input_tokens per message, so we compute deltas.
fn parse_claude_trace(content: &str) -> TraceStats {
    #[derive(Default)]
    struct MessageTokens {
        input_tokens: u64,
        output_tokens: u64,
        cache_read: u64,
        cache_write: u64,
        tool_use_count: u64,
        thinking_count: u64,
        thinking_tokens: u64,
        model: Option<String>,
    }

    let mut messages: Vec<MessageTokens> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }

        if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
            match &entry {
                LogEntry::Assistant(a) => {
                    let mut msg = MessageTokens::default();
                    if let Some(msg_data) = &a.message {
                        // Extract model
                        if let Some(model) = &msg_data.model {
                            msg.model = Some(model.clone());
                        }

                        // Extract usage tokens from nested usage object
                        if let Some(usage) = &msg_data.usage {
                            msg.input_tokens = usage.input_tokens;
                            msg.output_tokens = usage.output_tokens;
                            msg.cache_read = usage.cache_read_input_tokens;
                            msg.cache_write = usage.cache_creation_input_tokens;

                            // Also check for direct token fields on the message
                            if let Some(direct) = msg_data.input_tokens_direct {
                                msg.input_tokens = direct;
                            }
                            if let Some(direct) = msg_data.output_tokens_direct {
                                msg.output_tokens = direct;
                            }
                            if let Some(direct) = msg_data.cache_read_direct {
                                msg.cache_read = direct;
                            }
                            if let Some(direct) = msg_data.cache_write_direct {
                                msg.cache_write = direct;
                            }

                            // Extract server_tool_use tokens
                            if let Some(stu) = &usage.server_tool_use {
                                msg.input_tokens += stu.input_tokens;
                                msg.output_tokens += stu.output_tokens;
                            }

                            // Count tool uses and thinking blocks in content
                            if let Some(contents) = &msg_data.content {
                                if let benchmark_types::model::MessageContent::Structured(items) = contents {
                                    for content_item in items {
                                        match content_item {
                                            benchmark_types::model::Content::ToolUse(_) => msg.tool_use_count += 1,
                                            benchmark_types::model::Content::Thinking(thinking) => {
                                                msg.thinking_count += 1;
                                                if let Some(text) = &thinking.thinking {
                                                    msg.thinking_tokens += text.len() as u64 / 4;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }

                        messages.push(msg);
                    }
                }
                LogEntry::User(u) => {
                    let mut msg = MessageTokens::default();
                    if let Some(msg_data) = &u.message {
                        if let Some(usage) = &msg_data.usage {
                            msg.input_tokens = usage.input_tokens;
                            msg.output_tokens = usage.output_tokens;

                            if let Some(direct) = msg_data.input_tokens_direct {
                                msg.input_tokens = direct;
                            }
                            if let Some(direct) = msg_data.output_tokens_direct {
                                msg.output_tokens = direct;
                            }
                        }
                        messages.push(msg);
                    }
                }
                _ => {}
            }
        }
    }

    // Compute deltas between cumulative values
    let mut stats = TraceStats::default();
    let mut prev_input = 0u64;
    let mut prev_output = 0u64;
    let mut prev_cache_read = 0u64;
    let mut prev_cache_write = 0u64;

    for msg in &messages {
        stats.message_count += 1;

        // Delta = current cumulative - previous cumulative
        let delta_input = msg.input_tokens.saturating_sub(prev_input);
        let delta_output = msg.output_tokens.saturating_sub(prev_output);
        let delta_cache_read = msg.cache_read.saturating_sub(prev_cache_read);
        let delta_cache_write = msg.cache_write.saturating_sub(prev_cache_write);

        stats.total_input_tokens += delta_input;
        stats.total_output_tokens += delta_output;
        stats.total_cache_read += delta_cache_read;
        stats.total_cache_write += delta_cache_write;
        stats.tool_use_count += msg.tool_use_count;
        stats.thinking_count += msg.thinking_count;
        stats.thinking_tokens += msg.thinking_tokens;

        // Track model changes
        if let Some(model) = &msg.model {
            if !stats.model_changes.iter().any(|m| m == model.as_str()) {
                stats.model_changes.push(model.clone());
            }
        }

        // Update previous values for next delta calculation
        prev_input = msg.input_tokens;
        prev_output = msg.output_tokens;
        prev_cache_read = msg.cache_read;
        prev_cache_write = msg.cache_write;
    }

    stats.total_tokens = stats.total_input_tokens + stats.total_output_tokens;
    stats
}

/// Parse Pi trace file. Pi logs report direct (non-cumulative) token counts per message.
fn parse_pi_trace(content: &str) -> TraceStats {
    let mut stats = TraceStats::default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }

        // Try Pi format first
        if let Ok(entry) = serde_json::from_str::<PiLogEntry>(line) {
            process_pi_entry(&entry, &mut stats);
            continue;
        }

        // Fall back to raw JSON extraction
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            extract_usage_from_json(&json, &mut stats);
        }
    }

    stats
}

fn process_pi_entry(entry: &PiLogEntry, stats: &mut TraceStats) {
    match entry {
        PiLogEntry::Message(msg) => {
            if let Some(data) = &msg.message {
                stats.message_count += 1;
                if let Some(model) = &data.model {
                    if !stats.model_changes.iter().any(|m| m == model.as_str()) {
                        stats.model_changes.push(model.clone());
                    }
                }
                if let Some(usage) = &data.usage {
                    stats.total_input_tokens += usage.total_input();
                    stats.total_output_tokens += usage.total_output();
                    stats.total_cache_read += usage.cache_read.unwrap_or(0);
                    stats.total_cache_write += usage.cache_write.unwrap_or(0);
                    if let Some(total) = usage.total_tokens {
                        stats.total_tokens = total;
                    } else {
                        stats.total_tokens = stats.total_input_tokens + stats.total_output_tokens;
                    }
                    if let Some(cost) = &usage.cost {
                        stats.total_cost += cost.total.unwrap_or(0.0);
                    }
                }
                if let Some(contents) = &data.content {
                    for content in contents {
                        match content {
                            benchmark_types::model::PiContentItem::ToolCall(_) => stats.tool_use_count += 1,
                            benchmark_types::model::PiContentItem::Thinking(thinking) => {
                                stats.thinking_count += 1;
                                if let Some(text) = &thinking.thinking {
                                    stats.thinking_tokens += text.len() as u64 / 4;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        PiLogEntry::ModelChange(mc) => {
            if let Some(model) = &mc.model_id {
                if !stats.model_changes.iter().any(|m| m == model.as_str()) {
                    stats.model_changes.push(model.clone());
                }
            }
        }
        _ => {}
    }
}

fn extract_usage_from_json(json: &serde_json::Value, stats: &mut TraceStats) {
    if let Some(usage) = json.get("usage") {
        if let Some(input) = usage.get("input_tokens").or_else(|| usage.get("input")) {
            if let Some(v) = input.as_u64() { stats.total_input_tokens += v; }
        }
        if let Some(output) = usage.get("output_tokens").or_else(|| usage.get("output")) {
            if let Some(v) = output.as_u64() { stats.total_output_tokens += v; }
        }
        if let Some(cache_read) = usage.get("cache_read_input_tokens") {
            if let Some(v) = cache_read.as_u64() { stats.total_cache_read += v; }
        }
        if let Some(cache_write) = usage.get("cache_creation_input_tokens") {
            if let Some(v) = cache_write.as_u64() { stats.total_cache_write += v; }
        }
    }
    if let Some(model) = json.get("model").and_then(|v| v.as_str()) {
        if !stats.model_changes.iter().any(|m| m == model) {
            stats.model_changes.push(model.to_string());
        }
    }
    if let Some(content) = json.get("content") {
        if let Some(arr) = content.as_array() {
            for item in arr {
                if let Some(type_) = item.get("type").and_then(|v| v.as_str()) {
                    match type_ {
                        "tool_use" => stats.tool_use_count += 1,
                        "thinking" => stats.thinking_count += 1,
                        _ => {}
                    }
                }
            }
        }
    }
}

fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 { format!("{:.1}s", seconds) }
    else if seconds < 3600.0 { format!("{:.1}m", seconds / 60.0) }
    else { format!("{:.1}h", seconds / 3600.0) }
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    if len <= 3 { return s; }
    let mut result = String::with_capacity(len + len / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 { result.push(','); }
        result.push(c);
    }
    result
}

fn print_human_report(
    result_files: &[ResultFile], trace_stats: &HashMap<String, TraceStats>, model_stats: &HashMap<String, (TraceStats, usize)>,
    total_input: u64, total_output: u64, total_cache_read: u64, total_cache_write: u64,
    total_tokens: u64, total_cost: f64, total_messages: u64, total_tool_uses: u64,
    total_thinking: u64, total_thinking_tokens: u64,
    files_with_traces: usize, files_without_traces: usize, files_skipped_html: usize,
    elapsed: &std::time::Duration, details: bool,
) {
    let success_count = result_files.iter().filter(|r| r.success).count();
    let fail_count = result_files.len() - success_count;
    let total_duration: f64 = result_files.iter().map(|r| r.duration).sum();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    BENCHMARK REPORT                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    println!("\n📊 SUMMARY");
    println!("  Results analyzed:    {}", result_files.len());
    println!("  Successful:          {} ({:.1}%)", success_count, if result_files.is_empty() { 0.0 } else { success_count as f64 * 100.0 / result_files.len() as f64 });
    println!("  Failed:              {}", fail_count);
    println!("  Total duration:      {}", format_duration(total_duration));
    println!("  Parse time:          {:.1}s", elapsed.as_secs_f64());

    let mut by_agent: HashMap<String, Vec<&ResultFile>> = HashMap::new();
    for rf in result_files { by_agent.entry(rf.agent.clone()).or_default().push(rf); }
    println!("\n🤖 BY AGENT");
    for (agent, files) in &by_agent {
        let success = files.iter().filter(|r| r.success).count();
        let duration: f64 = files.iter().map(|r| r.duration).sum();
        println!("  {}:", agent);
        println!("    Results: {} ({} success, {} failed)", files.len(), success, files.len() - success);
        println!("    Duration: {}", format_duration(duration));
    }

    let mut by_language: HashMap<String, Vec<&ResultFile>> = HashMap::new();
    for rf in result_files { by_language.entry(rf.language.clone()).or_default().push(rf); }
    println!("\n💻 BY LANGUAGE");
    for (lang, files) in &by_language {
        let success = files.iter().filter(|r| r.success).count();
        println!("  {}: {} results ({} success)", lang, files.len(), success);
    }

    println!("\n🔤 TOKEN STATISTICS (from {} traces)", files_with_traces);
    println!("  Input tokens:        {:>15}", format_number(total_input));
    println!("  Output tokens:       {:>15}", format_number(total_output));
    println!("  Cache read tokens:   {:>15}", format_number(total_cache_read));
    println!("  Cache write tokens:  {:>15}", format_number(total_cache_write));
    println!("  Total tokens:        {:>15}", format_number(total_tokens));
    println!("  Total cost:          ${:>14.4}", total_cost);
    println!("  Messages processed:  {:>15}", format_number(total_messages));
    println!("  Tool uses:           {:>15}", format_number(total_tool_uses));
    println!("  Thinking blocks:     {:>15}", format_number(total_thinking));
    println!("  Thinking tokens:     {:>15}", format_number(total_thinking_tokens));
    println!("  Trace coverage:      {} / {} ({}%)", files_with_traces, result_files.len(), if result_files.is_empty() { 0 } else { files_with_traces * 100 / result_files.len() });
    if files_skipped_html > 0 { println!("  Skipped (HTML):      {}", files_skipped_html); }

    // Build per-model stats from model_stats (direct aggregation)
    let mut by_model: HashMap<String, (usize, u64, u64, f64)> = HashMap::new();
    for (model, (stats, trace_count)) in model_stats {
        by_model.insert(model.clone(), (*trace_count, stats.total_input_tokens + stats.total_output_tokens, stats.tool_use_count, stats.total_cost));
    }

    if !by_model.is_empty() {
        println!("\n📈 BY MODEL");
        println!("  {:<40} {:>8} {:>12} {:>10} {:>10}", "Model", "Traces", "Tokens", "Tools", "Cost");
        println!("  {}", "-".repeat(82));
        let mut models: Vec<_> = by_model.iter().collect();
        models.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
        for (model, (count, tokens, tools, cost)) in models {
            println!("  {:<40} {:>8} {:>12} {:>10} ${:>9.4}", model, format_number(*count as u64), format_number(*tokens), format_number(*tools as u64), cost);
        }
    }

    if details && !trace_stats.is_empty() {
        println!("\n📋 PER-EXERCISE DETAILS (top 50)");
        println!("  {:<30} {:<12} {:<10} {:>12} {:>8} {:>8}", "Exercise", "Language", "Agent", "Tokens", "Tools", "Cost");
        println!("  {}", "-".repeat(82));
        let mut keys: Vec<_> = trace_stats.keys().collect();
        keys.sort();
        for key in keys.iter().take(50) {
            let stats = &trace_stats[*key];
            let parts: Vec<&str> = key.split('/').collect();
            if parts.len() >= 3 {
                let total_t = stats.total_input_tokens + stats.total_output_tokens;
                println!("  {:<30} {:<12} {:<10} {:>12} {:>8} ${:>8.4}", parts[2], parts[1], parts[0], format_number(total_t), format_number(stats.tool_use_count), stats.total_cost);
            }
        }
    }

    if files_without_traces > 0 {
        println!("\n⚠️  {} result files had no corresponding JSONL trace file", files_without_traces);
    }
    println!("\n");
}

fn print_json_report(
    result_files: &[ResultFile], trace_stats: &HashMap<String, TraceStats>, _model_stats: &HashMap<String, (TraceStats, usize)>,
    total_input: u64, total_output: u64, total_cache_read: u64, total_cache_write: u64,
    total_tokens: u64, total_cost: f64, total_messages: u64, total_tool_uses: u64,
    total_thinking: u64, total_thinking_tokens: u64,
    files_with_traces: usize, files_without_traces: usize, files_skipped_html: usize,
    _elapsed: &std::time::Duration,
) {
    let success_count = result_files.iter().filter(|r| r.success).count();
    let total_duration: f64 = result_files.iter().map(|r| r.duration).sum();
    let report = serde_json::json!({
        "summary": { "total_results": result_files.len(), "successful": success_count, "failed": result_files.len() - success_count,
            "total_duration_seconds": total_duration, "files_with_traces": files_with_traces, "files_without_traces": files_without_traces, "files_skipped_html": files_skipped_html },
        "token_statistics": { "input_tokens": total_input, "output_tokens": total_output, "cache_read_tokens": total_cache_read,
            "cache_write_tokens": total_cache_write, "total_tokens": total_tokens, "total_cost": total_cost,
            "messages_processed": total_messages, "tool_uses": total_tool_uses, "thinking_blocks": total_thinking, "thinking_tokens": total_thinking_tokens },
        "exercises": result_files.iter().map(|rf| {
            let key = format!("{}/{}/{}", rf.agent, rf.language, rf.exercise_name);
            let stats = trace_stats.get(&key);
            serde_json::json!({ "agent": rf.agent, "language": rf.language, "exercise": rf.exercise_name, "model": rf.model,
                "success": rf.success, "duration": rf.duration, "attempts": rf.attempts, "exit_code": rf.exit_code,
                "tokens": stats.map(|s| s.total_input_tokens + s.total_output_tokens),
                "tool_uses": stats.map(|s| s.tool_use_count), "cost": stats.map(|s| s.total_cost) })
        }).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
