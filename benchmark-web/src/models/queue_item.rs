//! BenchmarkQueueItem - mirrors Java BenchmarkQueueItem.java
//! Represents a single item in the benchmark queue.

use crate::models::status::RunStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of a queue item (separate from RunStatus for the queue).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueItemStatus {
    PENDING,
    RUNNING,
    COMPLETED,
    FAILED,
    CANCELLED,
}

impl QueueItemStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::COMPLETED | Self::FAILED | Self::CANCELLED)
    }


}

impl From<RunStatus> for QueueItemStatus {
    fn from(status: RunStatus) -> Self {
        match status {
            RunStatus::PENDING => Self::PENDING,
            RunStatus::RUNNING => Self::RUNNING,
            RunStatus::COMPLETED => Self::COMPLETED,
            RunStatus::FAILED => Self::FAILED,
            RunStatus::CANCELLED => Self::CANCELLED,
        }
    }
}

impl std::fmt::Display for QueueItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PENDING => write!(f, "PENDING"),
            Self::RUNNING => write!(f, "RUNNING"),
            Self::COMPLETED => write!(f, "COMPLETED"),
            Self::FAILED => write!(f, "FAILED"),
            Self::CANCELLED => write!(f, "CANCELLED"),
        }
    }
}

/// A single item in the benchmark queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkQueueItem {
    pub id: String,
    pub agent_name: String,
    pub model: String,  // Required - no default model
    /// Pi thinking level: off, minimal, low, medium, high, xhigh (optional)
    pub thinking_level: Option<String>,
    pub language: String,
    pub exercise: String,
    /// Benchmark family (default `Polyglot`). AoC runs carry `Aoc2015`.
    pub category: benchmark_types::Category,
    /// AoC seed user (drives input generation). Only used for `Aoc2015` runs.
    #[serde(default)]
    pub aoc_user: Option<String>,
    pub retry: bool,
    pub status: QueueItemStatus,
    pub session_id: Option<String>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl BenchmarkQueueItem {
    /// Create a new queue item.
    pub fn new(
        agent_name: String,
        model: String,  // Required - no default model
        thinking_level: Option<String>,
        language: String,
        exercise: String,
        retry: bool,
    ) -> Self {
        Self::with_category(agent_name, model, thinking_level, language, exercise, retry, benchmark_types::Category::Polyglot)
    }

    /// Create a queue item with an explicit category.
    pub fn with_category(
        agent_name: String,
        model: String,
        thinking_level: Option<String>,
        language: String,
        exercise: String,
        retry: bool,
        category: benchmark_types::Category,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent_name,
            model,
            thinking_level,
            language,
            exercise,
            category,
            aoc_user: None,
            retry,
            status: QueueItemStatus::PENDING,
            session_id: None,
            scheduled_at: Some(Utc::now()),
            started_at: None,
            finished_at: None,
        }
    }

    /// Cancel a pending item.
    pub fn cancel(&mut self) {
        if self.status == QueueItemStatus::PENDING {
            self.status = QueueItemStatus::CANCELLED;
            self.finished_at = Some(Utc::now());
        }
    }

    /// Retry: reset to pending and return a clone.
    pub fn retry(&mut self) -> Self {
        let cloned = self.clone();
        self.status = QueueItemStatus::PENDING;
        self.session_id = None;
        self.started_at = None;
        self.finished_at = None;
        cloned
    }

    /// The benchmark category as a lowercase string (for serialization/UI).
    pub fn category_str(&self) -> String {
        self.category.to_string()
    }

    /// Set the AoC seed user on this item.
    pub fn with_aoc_user(mut self, user: impl Into<String>) -> Self {
        self.aoc_user = Some(user.into());
        self
    }


}
