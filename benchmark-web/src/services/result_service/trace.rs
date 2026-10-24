//! Trace-file parsing: tokens, turns and tool calls per agent format.
//!
//! The agent CLIs write JSONL traces while an exercise runs, and the dashboard needs
//! four numbers out of each run. Every agent has its own event format, so each has its
//! own model here; the shapes deliberately mirror `benchmark-reporter`, which parses the
//! same files for the markdown report.

use serde::Deserialize;
use std::path::Path;

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
    #[serde(rename = "cacheRead")]
    cache_read: Option<i64>,
    #[serde(rename = "cacheWrite")]
    cache_write: Option<i64>,
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
    let mut output = 0u64;
    let mut cached = 0u64;
    let mut uncached = 0u64;

    // A whole-file read keeps the loop free of per-line io::Result: a failed read
    // leaves the loop body unentered, which is what a failed open did.
    if let Ok(trace) = std::fs::read_to_string(trace_path) {
        for line in trace.lines() {
            if let Ok(PiLogEntry::Message(msg)) = serde_json::from_str::<PiLogEntry>(line.trim()) {
                if let Some(ref data) = msg.message {
                    if let Some(ref usage) = data.usage {
                        let msg_input = usage.input.unwrap_or(0) as u64;
                        let msg_cache_read = usage.cache_read.unwrap_or(0) as u64;
                        let msg_cache_write = usage.cache_write.unwrap_or(0) as u64;
                        output += usage.output.unwrap_or(0) as u64;

                        // cached = cache_read, uncached = input + cache_write (newly written to cache)
                        cached += msg_cache_read;
                        uncached += msg_input + msg_cache_write;
                    }
                }
            }
        }
    }

    // Total input = uncached + cached
    let input = uncached + cached;
    (input, output, cached, uncached)
}

/// Calculate tokens from a Claude agent trace file.
/// Returns (input, output, cached, uncached)
fn calculate_claude_tokens(trace_path: &Path) -> (u64, u64, u64, u64) {
    let mut input = 0u64;
    let mut output = 0u64;
    let mut cached = 0u64;
    let mut uncached = 0u64;
    let mut prev_input: u64 = 0;

    // A whole-file read keeps the loop free of per-line io::Result: a failed read
    // leaves the loop body unentered, which is what a failed open did.
    if let Ok(trace) = std::fs::read_to_string(trace_path) {
        for line in trace.lines() {
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
pub(super) fn parse_tokens_from_trace(trace_path: &Path, agent: &str) -> (u64, u64, u64, u64) {
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

/// Calculate input/output character counts from a trace file.
///
/// Token counts are model-tokenizer-dependent (each model counts its own tokens),
/// so they aren't comparable across models. Character counts derived from the
/// raw message text are tokenizer-independent and give a fair measure of the
/// actual text volume consumed and produced.
///
/// Works for both Pi and Claude trace formats, which share the same shape:
/// `{"type":"message","message":{"role":"...","content":[{"type":"text","text":"..."}]}}`.
///
/// Returns (input_chars, output_chars).
pub(super) fn parse_chars_from_trace(trace_path: &Path) -> (u64, u64) {
    if !trace_path.exists() {
        return (0, 0);
    }
    let mut input_chars: u64 = 0;
    let mut output_chars: u64 = 0;
    // A whole-file read keeps the loop free of per-line io::Result: a failed read
    // leaves the loop body unentered, which is what a failed open did.
    if let Ok(trace) = std::fs::read_to_string(trace_path) {
        for line in trace.lines() {
            let v: serde_json::Value = match serde_json::from_str(line.trim()) {
                Ok(v) => v,
                _ => continue,
            };
            // Skip entries without a nested message (e.g. session, model_change,
            // custom_message, thinking_level_change).
            let Some(msg) = v.get("message") else {
                continue;
            };
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let Some(content) = msg.get("content").and_then(|c| c.as_array()) else {
                continue;
            };
            for block in content {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    let n = block
                        .get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.chars().count() as u64)
                        .unwrap_or(0);
                    if role == "assistant" {
                        output_chars += n;
                    } else if role == "user" {
                        input_chars += n;
                    }
                }
            }
        }
    }
    (input_chars, output_chars)
}

/// Parse turn count and tool call count from a trace file.
/// Returns (turn_count, tool_call_count).
/// A "turn" is each assistant message. A "tool call" is each tool invocation
/// (for Pi traces: `toolCall` content blocks; for Claude traces: `tool_use` blocks).
pub(super) fn parse_trace_counts(trace_path: &Path, agent: &str) -> (u64, u64) {
    if !trace_path.exists() {
        return (0, 0);
    }
    if agent.starts_with("pi") {
        count_pi_turns_and_tools(trace_path)
    } else {
        count_claude_turns_and_tools(trace_path)
    }
}

/// Count turns and tool calls from a Pi agent trace file (JSONL format).
fn count_pi_turns_and_tools(trace_path: &Path) -> (u64, u64) {
    let mut turns = 0u64;
    let mut tool_calls = 0u64;
    // A whole-file read keeps the loop free of per-line io::Result: a failed read
    // leaves the loop body unentered, which is what a failed open did.
    if let Ok(trace) = std::fs::read_to_string(trace_path) {
        for line in trace.lines() {
            let v: serde_json::Value = match serde_json::from_str(line.trim()) {
                Ok(v) => v,
                _ => continue,
            };
            // Pi trace: {"type":"message","message":{"role":"assistant","content":[...]}}
            if v.get("type").and_then(|t| t.as_str()) == Some("message") {
                if let Some(msg) = v.get("message") {
                    if msg.get("role").and_then(|r| r.as_str()) == Some("assistant") {
                        turns += 1;
                        if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                            for block in content {
                                if block.get("type").and_then(|t| t.as_str()) == Some("toolCall")
                                    || block.get("type").and_then(|t| t.as_str())
                                        == Some("tool_use")
                                {
                                    tool_calls += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    (turns, tool_calls)
}

/// Count turns and tool calls from a Claude agent trace file (JSONL format).
fn count_claude_turns_and_tools(trace_path: &Path) -> (u64, u64) {
    let mut turns = 0u64;
    let mut tool_calls = 0u64;
    // A whole-file read keeps the loop free of per-line io::Result: a failed read
    // leaves the loop body unentered, which is what a failed open did.
    if let Ok(trace) = std::fs::read_to_string(trace_path) {
        for line in trace.lines() {
            if let Ok(LogEntry::Assistant(a)) = serde_json::from_str::<LogEntry>(line.trim()) {
                if a.message.is_some() {
                    turns += 1;
                }
            }
            // Second pass: Claude assistant entries can have tool_use in their content.
            // The current LogEntry doesn't expose content blocks,
            // so for Claude we count tool_use from the raw JSON.
            let v: serde_json::Value = match serde_json::from_str(line.trim()) {
                Ok(v) => v,
                _ => continue,
            };
            if v.get("type").and_then(|t| t.as_str()) == Some("assistant") {
                if let Some(msg) = v.get("message") {
                    if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                        for block in content {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                tool_calls += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    (turns, tool_calls)
}
