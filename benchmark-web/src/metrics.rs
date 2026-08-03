//! Prometheus-style metrics endpoint for benchmark-web.
//!
//! Exposes benchmark execution counters, durations, queue depth,
//! and active worker counts in Prometheus text format.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use benchmark_types::util::recover_poisoned;


/// Thread-safe metrics registry.
#[derive(Clone, Default)]
pub struct Metrics {
    /// exercises_total{agent, language, status} — incremented on each exercise completion.
    exercises: Arc<Mutex<Vec<ExerciseMetric>>>,
    /// queue_depth — current number of pending items (set externally).
    queue_depth: Arc<AtomicU64>,
    /// active_workers — current number of active workers (set externally).
    active_workers: Arc<AtomicU64>,
    /// sessions_total — lifetime session counter.
    sessions_total: Arc<AtomicU64>,
}

#[derive(Debug, Clone)]
struct ExerciseMetric {
    agent: String,
    language: String,
    status: String, // "success" or "failure"
    count: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a completed exercise result.
    pub fn record_exercise(&self, agent: &str, language: &str, success: bool) {
        let status = if success { "success" } else { "failure" };
        let mut exercises = recover_poisoned(self.exercises.lock());
        for metric in exercises.iter_mut() {
            if metric.agent == agent && metric.language == language && metric.status == status {
                metric.count += 1;
                return;
            }
        }
        exercises.push(ExerciseMetric {
            agent: agent.to_string(),
            language: language.to_string(),
            status: status.to_string(),
            count: 1,
        });
    }

    /// Set the current queue depth.
    pub fn set_queue_depth(&self, depth: u64) {
        self.queue_depth.store(depth, Ordering::SeqCst);
    }

    /// Set the current active worker count.
    pub fn set_active_workers(&self, count: u64) {
        self.active_workers.store(count, Ordering::SeqCst);
    }

    /// Increment total sessions.
    pub fn inc_sessions(&self) {
        self.sessions_total.fetch_add(1, Ordering::SeqCst);
    }

    /// Render all metrics in Prometheus text format.
    pub fn render(&self) -> String {
        let mut out = String::new();

        // Exercise counter
        let exercises = recover_poisoned(self.exercises.lock());
        for metric in exercises.iter() {
            let total = metric.count;
            // For Prometheus, emit one total + per-status breakdown
            out.push_str(&format!(
                "benchmark_exercises_total{{agent=\"{}\",language=\"{}\",status=\"{}\"}} {}\n",
                metric.agent, metric.language, metric.status, total
            ));
        }

        // Gauge metrics
        out.push_str(&format!(
            "benchmark_queue_depth {}\n",
            self.queue_depth.load(Ordering::SeqCst)
        ));
        out.push_str(&format!(
            "benchmark_active_workers {}\n",
            self.active_workers.load(Ordering::SeqCst)
        ));
        out.push_str(&format!(
            "benchmark_sessions_total {}\n",
            self.sessions_total.load(Ordering::SeqCst)
        ));

        out
    }
}
