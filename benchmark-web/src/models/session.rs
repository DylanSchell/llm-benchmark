//! BenchmarkSession - mirrors Java BenchmarkSession.java
//! Represents a running benchmark with SSE streaming support.

use crate::models::status::RunStatus;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use uuid::Uuid;

/// A benchmark session with SSE output streaming.
/// Uses broadcast channels so multiple SSE clients can all receive live output,
/// and accumulated_output is always kept in sync for server-side rendering.
#[derive(Debug, Serialize)]
pub struct BenchmarkSession {
    pub id: String,
    pub agent_name: String,
    pub languages: Vec<String>,
    pub model: String,
    pub exercise_name: Option<String>,
    pub retry: bool,
    pub status: RunStatus,
    pub completed_exercises: u32,
    pub total_exercises: i32,
    pub progress: f64,
    /// Shared accumulated output — updated by the output consumer so
    /// server-side template rendering always has the latest data.
    #[serde(skip)]
    pub(crate) accumulated_output: Arc<Mutex<Vec<String>>>,
    pub error_message: Option<String>,
    pub timeout_ms: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    /// Broadcast sender — one producer (the agent), many consumers (SSE streams).
    /// Created at session construction time so output is never lost.
    #[serde(skip)]
    pub(crate) msg_tx: broadcast::Sender<String>,
}

impl BenchmarkSession {
    /// Create a new benchmark session.
    /// Uses a broadcast channel (1024 capacity) so multiple SSE clients
    /// can all receive live output, and accumulated_output is always in sync.
    pub fn new(
        agent_name: String,
        languages: Vec<String>,
        model: String,
        exercise_name: Option<String>,
        retry: bool,
        timeout_ms: u64,
    ) -> Self {
        let (msg_tx, _rx) = broadcast::channel::<String>(1024);
        Self {
            id: Uuid::new_v4().to_string(),
            agent_name,
            languages,
            model,
            exercise_name,
            retry,
            status: RunStatus::PENDING,
            completed_exercises: 0,
            total_exercises: 0,
            progress: 0.0,
            accumulated_output: Arc::new(Mutex::new(Vec::new())),
            error_message: None,
            timeout_ms,
            started_at: None,
            finished_at: None,
            msg_tx,
        }
    }
}

impl Clone for BenchmarkSession {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            agent_name: self.agent_name.clone(),
            languages: self.languages.clone(),
            model: self.model.clone(),
            exercise_name: self.exercise_name.clone(),
            retry: self.retry,
            status: self.status.clone(),
            completed_exercises: self.completed_exercises,
            total_exercises: self.total_exercises,
            progress: self.progress,
            accumulated_output: Arc::clone(&self.accumulated_output),
            error_message: self.error_message.clone(),
            timeout_ms: self.timeout_ms,
            started_at: self.started_at,
            finished_at: self.finished_at,
            msg_tx: self.msg_tx.clone(),
        }
    }
}

impl BenchmarkSession {
    /// Set up an SSE subscriber for this session.
    /// Returns a fresh receiver that will get all future messages.
    /// Any messages already in accumulated_output are sent first as a snapshot,
    /// so late-joining clients see what happened before they connected.
    pub fn setup_sse(&self) -> broadcast::Receiver<String> {
        let rx = self.msg_tx.subscribe();

        // Send any accumulated output that was captured before this client connected.
        // This ensures a late-joining user sees the full history, not just live updates.
        let output = self.accumulated_output.lock().unwrap().clone();
        for msg in output {
            // Ignore send errors — if the receiver buffer is full, the message
            // will arrive later via live streaming.
            let _ = self.msg_tx.send(msg);
        }

        rx
    }

    /// Emit output: appends to accumulated_output and broadcasts to all SSE subscribers.
    pub fn emit_output(&mut self, message: &str) {
        if let Ok(mut out) = self.accumulated_output.lock() {
            out.push(message.to_string());
        }
        // Ignore send errors — no subscribers means nobody is listening.
        let _ = self.msg_tx.send(message.to_string());
    }

    /// Mark session as running.
    pub fn start(&mut self) {
        self.status = RunStatus::RUNNING;
        self.started_at = Some(Utc::now());
    }

    /// Mark session as completed.
    pub fn complete(&mut self) {
        self.status = RunStatus::COMPLETED;
        self.finished_at = Some(Utc::now());
        self.progress = if self.total_exercises > 0 {
            (self.completed_exercises as f64 / self.total_exercises as f64) * 100.0
        } else {
            100.0
        };
    }

    /// Cancel the session.
    pub fn cancel(&mut self) {
        self.status = RunStatus::CANCELLED;
        self.finished_at = Some(Utc::now());
        self.emit_output("Cancelled by user");
    }

    /// Force complete (for shutdown).
    pub fn force_complete(&mut self) {
        self.cancel();
    }

    /// Increment completed exercise count.
    pub fn increment_completed(&mut self) {
        self.completed_exercises += 1;
        if self.total_exercises > 0 {
            self.progress = (self.completed_exercises as f64 / self.total_exercises as f64) * 100.0;
        }
    }

    /// Set total exercise count.
    pub fn set_total_exercises(&mut self, total: i32) {
        self.total_exercises = total;
    }

    /// Set error message.
    pub fn set_error_message(&mut self, error: &str) {
        self.error_message = Some(error.to_string());
    }

    /// Get the language (first language, for API compatibility).
    pub fn language(&self) -> String {
        self.languages.first().cloned().unwrap_or_default()
    }

    /// Get progress as a display string.
    pub fn progress_display(&self) -> String {
        format!("{:.1}%", self.progress)
    }

    /// Creates an output consumer closure that broadcasts to all SSE subscribers
    /// AND updates accumulated_output so server-side rendering always has data.
    pub fn make_output_consumer(&self) -> Box<dyn Fn(&str) + Send + Sync> {
        let tx = self.msg_tx.clone();
        let accumulated = Arc::clone(&self.accumulated_output);
        Box::new(move |message: &str| {
            // Update accumulated output so server-side rendering works for any visitor
            if let Ok(mut out) = accumulated.lock() {
                out.push(message.to_string());
            }
            // Broadcast to all SSE subscribers (multiple users)
            let _ = tx.send(message.to_string());
        })
    }

    /// Get accumulated output as a concatenated string.
    /// Mirrors Java BenchmarkSession.getAccumulatedOutput().
    pub fn get_accumulated_output(&self) -> String {
        self.accumulated_output.lock().map(|v| v.join("")).unwrap_or_default()
    }

}
