use chrono::DateTime;
use serde::{Deserialize, Serialize, Serializer};
use std::path::Path;

/// Represents a time interval [start, end) in milliseconds.
#[derive(Debug, Clone, Copy)]
pub struct TimeInterval {
    pub start: u64,
    pub end: u64,
}

impl TimeInterval {
    pub fn duration(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

/// Merges overlapping/adjacent intervals and returns total wall-clock duration.
///
/// Example: [0,100], [50,150], [200,300] → merged [0,150]+[200,300] = 200ms
/// This is the correct way to sum execution times when tasks run in parallel.
pub fn merge_intervals_and_total_duration(intervals: &[TimeInterval]) -> u64 {
    if intervals.is_empty() {
        return 0;
    }

    let mut sorted = intervals.to_vec();
    sorted.sort_by_key(|i| i.start);

    let mut total_duration: u64 = 0;
    let mut current_start = sorted[0].start;
    let mut current_end = sorted[0].end;

    for interval in &sorted[1..] {
        if interval.start <= current_end {
            // Overlapping or adjacent — extend
            current_end = current_end.max(interval.end);
        } else {
            // Disjoint — close current, start new
            total_duration += current_end.saturating_sub(current_start);
            current_start = interval.start;
            current_end = interval.end;
        }
    }

    total_duration += current_end.saturating_sub(current_start);
    total_duration
}

/// Parses an RFC3339 timestamp to Unix epoch milliseconds.
pub fn parse_rfc3339_ms(ts: &str) -> Option<u64> {
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp_millis() as u64)
}

/// Computes wall-clock statistics for a set of results by merging overlapping intervals.
pub fn compute_wall_clock_stats(results: &[AgentResult]) -> WallClockStats {
    if results.is_empty() {
        return WallClockStats::default();
    }

    let intervals: Vec<TimeInterval> = results
        .iter()
        .filter_map(|r| {
            let start_ms = parse_rfc3339_ms(&r.start_time);
            let end_ms = parse_rfc3339_ms(&r.end_time);
            match (start_ms, end_ms) {
                (Some(start), Some(end)) if end >= start => Some(TimeInterval { start, end }),
                _ => None,
            }
        })
        .collect();

    let total_duration = merge_intervals_and_total_duration(&intervals);

    let wall_start = intervals.iter().map(|i| i.start).min().unwrap_or(0);
    let wall_end = intervals.iter().map(|i| i.end).max().unwrap_or(0);
    let wall_clock_span = wall_end.saturating_sub(wall_start);

    let sum_individual: u64 = intervals.iter().map(|i| i.duration()).sum();

    WallClockStats {
        wall_clock_span,
        total_duration,
        sum_individual_durations: sum_individual,
        parallelism_gain: if wall_clock_span > 0 {
            (1.0 - (total_duration as f64 / wall_clock_span as f64)).max(0.0)
        } else {
            0.0
        },
    }
}

/// Statistics about wall-clock execution time vs individual durations.
#[derive(Debug, Clone, Default)]
pub struct WallClockStats {
    /// Earliest start to latest end across all results.
    pub wall_clock_span: u64,
    /// Sum of merged (non-overlapping) intervals — true work time.
    pub total_duration: u64,
    /// Sum of all individual durations (overcounts when parallel).
    pub sum_individual_durations: u64,
    /// Fraction of time saved by parallelism (0.0 = no overlap, ~1.0 = fully parallel).
    pub parallelism_gain: f64,
}

fn serialize_duration_seconds<S>(duration_ms: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_f64(*duration_ms as f64 / 1000.0)
}

#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    async fn run_exercise(
        &self,
        exercise: &crate::exercise::Exercise,
        host_exercise_dir: &Path,
        model: &str,
        thinking_level: Option<&str>,
        results_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>>;

    /// Returns the agent's name (e.g., "reference", "claude", "pi").
    fn get_name(&self) -> &str;
}

/// Agent result from running an exercise
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    #[serde(rename = "exerciseName")]
    pub exercise_name: String,
    pub language: String,
    pub success: bool,
    pub exit_code: i32,
    pub output: String,
    #[serde(rename = "duration", serialize_with = "serialize_duration_seconds")]
    pub duration_ms: u64,
    #[serde(rename = "startTime")]
    pub start_time: String,
    #[serde(rename = "endTime")]
    pub end_time: String,
    #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(rename = "containerId")]
    pub container_id: String,
}

impl AgentResult {
    /// Parses start_time to Unix epoch milliseconds.
    pub fn start_ms(&self) -> Option<u64> {
        parse_rfc3339_ms(&self.start_time)
    }

    /// Parses end_time to Unix epoch milliseconds.
    pub fn end_ms(&self) -> Option<u64> {
        parse_rfc3339_ms(&self.end_time)
    }
}

impl AgentResult {
    pub fn builder() -> AgentResultBuilder {
        AgentResultBuilder::new()
    }
}

#[derive(Default)]
pub struct AgentResultBuilder {
    exercise_name: Option<String>,
    language: Option<String>,
    success: bool,
    exit_code: i32,
    output: String,
    duration_ms: u64,
    start_time: Option<String>,
    end_time: Option<String>,
    error_message: Option<String>,
    container_id: Option<String>,
}

impl AgentResultBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn exercise_name(mut self, exercise_name: String) -> Self {
        self.exercise_name = Some(exercise_name);
        self
    }

    pub fn language(mut self, language: String) -> Self {
        self.language = Some(language);
        self
    }

    pub fn success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }

    pub fn exit_code(mut self, exit_code: i32) -> Self {
        self.exit_code = exit_code;
        self
    }

    pub fn output(mut self, output: String) -> Self {
        self.output = output;
        self
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn start_time(mut self, start_time: String) -> Self {
        self.start_time = Some(start_time);
        self
    }

    pub fn end_time(mut self, end_time: String) -> Self {
        self.end_time = Some(end_time);
        self
    }

    pub fn error_message(mut self, error_message: Option<String>) -> Self {
        self.error_message = error_message;
        self
    }



    pub fn container_id(mut self, container_id: String) -> Self {
        self.container_id = Some(container_id);
        self
    }

    pub fn build(self) -> AgentResult {
        let now = chrono::Utc::now();
        AgentResult {
            exercise_name: self.exercise_name.unwrap_or_default(),
            language: self.language.unwrap_or_default(),
            success: self.success,
            exit_code: self.exit_code,
            output: self.output,
            duration_ms: self.duration_ms,
            start_time: self.start_time.unwrap_or_else(|| now.to_rfc3339()),
            end_time: self.end_time.unwrap_or_else(|| now.to_rfc3339()),
            error_message: self.error_message,
            container_id: self.container_id.unwrap_or_default(),
        }
    }
}
