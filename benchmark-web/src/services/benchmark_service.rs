//! BenchmarkService - mirrors Java BenchmarkService.java
//! Facade service for benchmark operations.
//! Coordinates between SessionManager, BenchmarkExecutor, and QueueProcessor.

use crate::models::queue_item::BenchmarkQueueItem;
use crate::models::session::BenchmarkSession;
use crate::services::queue_processor::QueueProcessor;
use crate::services::result_service::ResultService;
use crate::services::session_manager::SessionManager;
use benchmark_core::exercise_runner::ExerciseRunner;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

/// Facade service for benchmark operations.
/// Coordinates between SessionManager, QueueProcessor, and ResultService.
#[derive(Clone)]
pub struct BenchmarkService {
    session_manager: SessionManager,
    queue_processor: Arc<QueueProcessor>,
    result_service: ResultService,
    exercise_runner: ExerciseRunner,
}

impl BenchmarkService {
    /// Create a new BenchmarkService.
    pub fn new(
        session_manager: SessionManager,
        queue_processor: QueueProcessor,
        result_service: ResultService,
        exercise_runner: ExerciseRunner,
    ) -> Self {
        Self {
            session_manager,
            queue_processor: Arc::new(queue_processor),
            result_service,
            exercise_runner,
        }
    }

    // =============================================================================
    // Session Management
    // =============================================================================

    /// Get a session by ID.
    pub fn get_session(&self, session_id: &str) -> Option<BenchmarkSession> {
        self.session_manager.get_session(session_id)
    }

    /// Take the internal message receiver from a session.
    /// Used by SSE endpoint to connect to the session's message channel.
    /// Broadcast channels support multiple consumers — each call creates a fresh subscriber.
    pub fn take_session_receiver(&self, session_id: &str) -> Option<tokio::sync::broadcast::Receiver<String>> {
        self.session_manager.take_session_receiver(session_id)
    }

    /// Get all sessions.
    pub fn get_all_sessions(&self) -> HashMap<String, BenchmarkSession> {
        self.session_manager.get_all_sessions()
    }

    /// Cancel a running session.
    pub fn cancel_session(&self, session_id: &str) -> bool {
        self.session_manager.cancel_session(session_id)
    }

    /// Get active sessions.
    pub fn get_active_sessions(&self) -> Vec<BenchmarkSession> {
        self.session_manager.get_active_sessions()
    }

    /// Get active session count.
    pub fn get_active_session_count(&self) -> usize {
        self.session_manager.get_active_session_count()
    }

    // =============================================================
    // Queue Management - Delegates to QueueProcessor
    // =============================================================================

    /// Schedule a batch of benchmark runs with optional retry mode.
    pub fn schedule_batch_with_retry(
        &self,
        agent_name: String,
        languages: Vec<String>,
        model: String,  // Required - no default model
        exercise: Option<String>,
        retry: bool,
    ) -> Vec<BenchmarkQueueItem> {
        self.queue_processor.schedule_batch(agent_name, languages, model, exercise, retry)
    }

    /// Cancel a queue item.
    pub async fn cancel_queue_item(&self, item_id: &str) -> bool {
        self.queue_processor.cancel_queue_item(item_id).await
    }

    /// Get all queue items.
    pub fn get_queue_items(&self) -> Vec<BenchmarkQueueItem> {
        self.queue_processor.get_queue_items()
    }

    /// Clear pending items from queue.
    pub fn clear_pending_queue(&self) {
        self.queue_processor.clear_pending_queue();
    }

    /// Clear completed and cancelled items from the queue.
    pub fn clear_completed_and_cancelled(&self) -> usize {
        self.queue_processor.clear_completed_and_cancelled()
    }

    /// Retry a failed queue item.
    pub fn retry_queue_item(&self, item_id: &str) -> Option<BenchmarkQueueItem> {
        self.queue_processor.retry_item(item_id)
    }

    /// Get the number of currently active workers.
    pub async fn get_active_worker_count(&self) -> usize {
        self.queue_processor.get_active_worker_count().await
    }

    /// Get the configured parallelism limit.
    pub fn get_parallelism_limit(&self) -> usize {
        self.queue_processor.get_parallelism_limit()
    }

    /// Get the queue processor (for starting the worker).
    pub fn get_queue_processor(&self) -> Arc<QueueProcessor> {
        Arc::clone(&self.queue_processor)
    }

    // =============================================================================
    // Result Service Access
    // =============================================================================

    /// Refreshes the result cache.
    pub fn refresh_result_cache(&self) {
        self.result_service.refresh_cache();
    }

    /// Get the ExerciseRunner for discovering exercises.
    pub fn get_exercise_runner(&self) -> &ExerciseRunner {
        &self.exercise_runner
    }

    /// Get all models.
    pub fn get_models(&self) -> Vec<String> {
        self.result_service.get_models()
    }

    /// Get all unique languages.
    pub fn get_languages(&self) -> Vec<String> {
        self.result_service.get_languages()
    }

    /// Get all exercises, optionally filtered by language.
    pub fn get_exercises(&self, language: Option<&str>) -> Vec<String> {
        self.result_service.get_exercises(language)
    }

    /// List individual results with filtering.
    pub fn list_individual_results(
        &self,
        language: Option<&str>,
        agent: Option<&str>,
        model: Option<&str>,
        exercise: Option<&str>,
        quick_only: bool,
    ) -> Vec<crate::services::result_service::IndividualResult> {
        self.result_service.list_individual_results(language, agent, model, exercise, quick_only)
    }

    /// Get a result by its cache key.
    pub fn get_result_by_key(&self, key: &str) -> Option<HashMap<String, String>> {
        self.result_service.get_result_by_key(key)
    }

    /// Get aggregate statistics.
    pub fn get_statistics(
        &self,
        language: Option<&str>,
        agent: Option<&str>,
        model: Option<&str>,
        exercise: Option<&str>,
        quick_only: bool,
    ) -> crate::services::result_service::Statistics {
        self.result_service.get_statistics(language, agent, model, exercise, quick_only)
    }

    /// Get loading status of the result cache.
    pub fn get_loading_status(&self) -> crate::services::result_service::LoadingStatus {
        self.result_service.get_loading_status()
    }

    /// Get trace content for a result.
    pub fn get_trace_content(&self, key: &str) -> anyhow::Result<Option<String>> {
        self.result_service.get_trace_content(key)
    }

    /// Calculate composite scores for results.
    pub fn calculate_scores(
        &self,
        language: Option<&str>,
        agent: Option<&str>,
        model: Option<&str>,
        exercise: Option<&str>,
        quick_only: bool,
    ) -> Vec<crate::services::result_service::ScoredResult> {
        self.result_service.calculate_scores(language, agent, model, exercise, quick_only)
    }

    /// Get aggregated model scores.
    pub fn get_model_scores(
        &self,
        language: Option<&str>,
        agent: Option<&str>,
        quick_only: bool,
    ) -> Vec<crate::services::result_service::ModelScore> {
        self.result_service.get_model_scores(language, agent, quick_only)
    }

    // =============================================================================
    // Model Management
    // =============================================================================

    /// Fetch available models from the inference endpoint.
    pub async fn fetch_models(&self) -> anyhow::Result<Vec<String>> {
        self.exercise_runner.fetch_models().await
    }

    // =============================================================================
    // Shutdown
    // =============================================================================

    /// Gracefully shut down all services.
    pub async fn shutdown(&self) {
        info!("Shutting down benchmark service...");
        self.session_manager.shutdown();
        self.queue_processor.shutdown().await;
    }

    /// Clean up all Docker containers (orphaned containers from crashed runs).
    pub async fn cleanup_containers(&self) {
        info!("Cleaning up Docker containers...");
        self.exercise_runner.cleanup_all_containers().await;
    }
}
