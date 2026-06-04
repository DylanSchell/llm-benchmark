//! QueueProcessor - mirrors Java QueueProcessor.java
//! Processes benchmark queue items with parallelism control.

use crate::models::queue::BenchmarkQueue;
use crate::models::queue_item::BenchmarkQueueItem;
use crate::models::status::RunStatus;
use crate::services::benchmark_executor::BenchmarkExecutor;
use crate::services::result_service::ResultService;
use crate::services::session_manager::SessionManager;
use anyhow::Result;
use benchmark_core::exercise_runner::ExerciseRunner;
use benchmark_types::config::QuickBenchConfig;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{info, warn, error, debug};

/// Configuration for the queue processor.
#[derive(Debug)]
pub struct QueueConfig {
    /// Maximum number of concurrent workers.
    pub parallelism: usize,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            parallelism: 2,
        }
    }
}

/// Processes benchmark queue items.
/// Handles queue management and concurrent processing of queued benchmark runs.
pub struct QueueProcessor {
    queue: BenchmarkQueue,
    session_manager: SessionManager,
    result_service: ResultService,
    benchmark_executor: Arc<BenchmarkExecutor>,
    exercise_runner: ExerciseRunner,
    config: QueueConfig,
    worker_semaphore: Arc<Semaphore>,
    active_workers: Arc<Mutex<usize>>,
    shutdown_requested: Arc<Mutex<bool>>,
    queue_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl QueueProcessor {
    /// Create a new QueueProcessor.
    pub fn new(
        session_manager: SessionManager,
        result_service: ResultService,
        benchmark_executor: Arc<BenchmarkExecutor>,
        exercise_runner: ExerciseRunner,
        config: QueueConfig,
    ) -> Self {
        let worker_semaphore = Arc::new(Semaphore::new(config.parallelism));
        let active_workers = Arc::new(Mutex::new(0));
        let shutdown_requested = Arc::new(Mutex::new(false));
        let queue_task = Arc::new(Mutex::new(None::<tokio::task::JoinHandle<()>>));

        Self {
            queue: BenchmarkQueue::new(),
            session_manager,
            result_service,
            benchmark_executor,
            exercise_runner,
            config,
            worker_semaphore,
            active_workers,
            shutdown_requested,
            queue_task,
        }
    }

    /// Schedule a batch of benchmark runs.
    /// Never uses "all" exercises - always expands to individual exercise items.
    /// Skips exercises that have already been completed successfully (unless retry mode).
    /// Also, skip exercises that are already pending/running in the queue (unless retry mode).
    pub fn schedule_batch(
        &self,
        agent_name: String,
        languages: Vec<String>,
        model: String,  // Required - no default model
        thinking_level: Option<String>,
        exercise: Option<String>,
        retry: bool,
    ) -> Vec<BenchmarkQueueItem> {
        let mut items = Vec::new();
        let effective_model = &model;

        // Get existing queue items to check for duplicates
        let existing_items = self.queue.get_pending_items();
        let existing_keys: std::collections::HashSet<(String, String, String)> = existing_items
            .iter()
            .map(|item| (item.agent_name.clone(), item.language.clone(), item.exercise.clone()))
            .collect();

        match exercise.as_deref() {
            Some("__slow__") => {
                // Slow bench mode - all exercises EXCEPT the quick-bench ones
                for language in &languages {
                    let quick_set = QuickBenchConfig::get_quick_exercises_set(language);
                    let all_exercises = self.exercise_runner.get_exercises_for_language(language);
                    let slow_exercises: Vec<String> = all_exercises
                        .into_iter()
                        .filter(|e| !quick_set.contains(e))
                        .collect();
                    if slow_exercises.is_empty() {
                        info!("No slow-bench exercises for language: {} (all are quick-bench)", language);
                        continue;
                    }
                    for exercise_name in slow_exercises {
                        if !retry && self.result_exists(&exercise_name, &agent_name, &effective_model, language) {
                            debug!(
                                "Skipping slow-bench exercise: {} for language: {} (already completed successfully)",
                                exercise_name, language
                            );
                            continue;
                        }

                        let key = (agent_name.clone(), language.clone(), exercise_name.clone());
                        if !retry && existing_keys.contains(&key) {
                            debug!(
                                "Skipping slow-bench exercise: {} for language: {} (already in queue)",
                                exercise_name, language
                            );
                            continue;
                        }

                        let item = BenchmarkQueueItem::new(
                            agent_name.clone(),
                            model.clone(),
                            thinking_level.clone(),
                            language.clone(),
                            exercise_name.clone(),
                            retry,
                        );
                        items.push(item);
                    }
                }
            }
            Some("__quick__") => {
                // Quick bench mode - use curated list of fast exercises
                for language in &languages {
                    let quick_exercises = QuickBenchConfig::get_exercises_for_language(language);
                    if quick_exercises.is_empty() {
                        warn!("No quick-bench exercises defined for language: {}", language);
                        continue;
                    }
                    for exercise_name in quick_exercises {
                        // Skip if already completed successfully (unless retry)
                        if !retry && self.result_exists(&exercise_name, &agent_name, &effective_model, language) {
                            debug!(
                                "Skipping quick-bench exercise: {} for language: {} (already completed successfully)",
                                exercise_name, language
                            );
                            continue;
                        }

                        // Skip if already in queue (unless retry)
                        let key = (agent_name.clone(), language.clone(), exercise_name.clone());
                        if !retry && existing_keys.contains(&key) {
                            debug!(
                                "Skipping quick-bench exercise: {} for language: {} (already in queue)",
                                exercise_name, language
                            );
                            continue;
                        }

                        let item = BenchmarkQueueItem::new(
                            agent_name.clone(),
                            model.clone(),
                            thinking_level.clone(),
                            language.clone(),
                            exercise_name.clone(),
                            retry,
                        );
                        items.push(item);
                    }
                }
            }
            Some(exercise_name) => {
                // Single exercise specified - create one item per language for that exercise
                if languages.is_empty() {
                    tracing::error!("ERROR: No languages provided for single exercise mode!");
                    return items;
                }
                
                // CRITICAL DEBUG: Log exactly what we received
                tracing::info!("SINGLE EXERCISE RECEIVED: exercise='{}', languages={:?} (count={})", 
                    exercise_name, languages, languages.len());
                
                // Sanity check: if we have more than expected languages, log a warning
                if languages.len() > 1 {
                    tracing::warn!("WARNING: Expected 1 language for single exercise mode, got {}", languages.len());
                }
                
                // Create exactly ONE item per language in the list
                for language in &languages {
                    if !retry && self.result_exists(exercise_name, &agent_name, &effective_model, language) {
                        info!(
                            "Skipping exercise: {} for language: {} (already completed successfully)",
                            exercise_name, language
                        );
                        continue;
                    }

                    let item = BenchmarkQueueItem::new(
                        agent_name.clone(),
                        model.clone(),
                        thinking_level.clone(),
                        language.clone(),
                        exercise_name.to_string(),
                        retry,
                    );
                    items.push(item);
                }
            }
            None => {
                // No specific exercise - expand to individual items for each language/exercise combination
                for language in &languages {
                    let exercises = self.exercise_runner.get_exercises_for_language(language);
                    for exercise_name in exercises {
                        if !retry && self.result_exists(&exercise_name, &agent_name, &effective_model, language) {
                            debug!(
                                "Skipping exercise: {} for language: {} (already completed successfully)",
                                exercise_name, language
                            );
                            continue;
                        }

                        let item = BenchmarkQueueItem::new(
                            agent_name.clone(),
                            model.clone(),
                            thinking_level.clone(),
                            language.clone(),
                            exercise_name,
                            retry,
                        );
                        items.push(item);
                    }
                }
            }
        }

        self.queue.add_all(items.clone());
        info!(
            "Scheduled {} queue items total",
            items.len()
        );
        items
    }

    /// Check if a result file already exists for this exercise and was successful.
    fn result_exists(&self, exercise: &str, agent: &str, model: &str, language: &str) -> bool {
        // The ResultService knows where to find results
        self.result_service.result_file_success(exercise, agent, model, language, self.result_service.results_dir())
    }



    /// Start the queue worker.
    pub async fn start_queue_worker(&self) -> bool {
        // Check if already started
        let already_started = {
            let task = self.queue_task.lock().await;
            task.is_some()
        };
        if already_started {
            return false; // Already started
        }

        let queue = self.queue.clone();
        let session_manager = self.session_manager.clone();
        let benchmark_executor = self.benchmark_executor.clone();
        let worker_semaphore = Arc::clone(&self.worker_semaphore);
        let active_workers = Arc::clone(&self.active_workers);
        let shutdown_requested = Arc::clone(&self.shutdown_requested);
        let parallelism = self.config.parallelism;

        info!("Queue worker started (parallelism={})", parallelism);

        let handle = tokio::spawn(async move {
            loop {
                // Check shutdown
                {
                    let requested = shutdown_requested.lock().await;
                    if *requested {
                        info!("Queue worker shutting down");
                        break;
                    }
                }

                // Check capacity
                {
                    let workers = active_workers.lock().await;
                    if *workers >= parallelism {
                        drop(workers);
                        // Wait for a notification that capacity may have freed up
                        queue.wait_for_item().await;
                        continue;
                    }
                }

                // Try to acquire a permit
                let _permit = match worker_semaphore.clone().try_acquire_owned() {
                    Ok(permit) => {
                        // Forget the owned permit so we don't release it here.
                        // The worker task will release it on completion via add_permits(1).
                        permit.forget();
                    }
                    Err(_) => {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                };

                // Get next item — block until something is available
                let item = loop {
                    // Check shutdown again while waiting
                    {
                        let requested = shutdown_requested.lock().await;
                        if *requested {
                            worker_semaphore.add_permits(1);
                            return;
                        }
                    }

                    if let Some(item) = queue.poll_next() {
                        break item;
                    }
                    // Nothing in queue — wait for a notification
                    queue.wait_for_item().await;
                };
                let item_id = item.id.clone();

                // Increment active workers
                {
                    let mut workers = active_workers.lock().await;
                    *workers += 1;
                    drop(workers);
                }

                let worker_count = *active_workers.lock().await;
                info!(
                    "Starting queue item: {} - {}/{} (active workers: {}/{})",
                    item_id,
                    item.language,
                    item.exercise,
                    worker_count,
                    parallelism
                );

                // Process the item
                let queue = queue.clone();
                let session_manager = session_manager.clone();
                let benchmark_executor = benchmark_executor.clone();
                let worker_semaphore = Arc::clone(&worker_semaphore);
                let active_workers = Arc::clone(&active_workers);

                tokio::spawn(async move {
                    if let Err(e) = Self::process_queue_item(
                        &queue,
                        &session_manager,
                        &benchmark_executor,
                        item,
                    )
                    .await
                    {
                        error!("Queue item processing failed: {}", e);
                    }

                    // Decrement active workers and release permit
                    {
                        let mut workers = active_workers.lock().await;
                        *workers -= 1;
                        drop(workers);
                    }
                    worker_semaphore.add_permits(1);

                    // Wake the main loop — capacity has freed up
                    queue.notify_capacity();

                    let worker_count = *active_workers.lock().await;
                    info!(
                        "Completed queue item: {} (active workers: {})",
                        item_id,
                        worker_count
                    );
                });
            }
        });

        {
            let mut task = self.queue_task.lock().await;
            *task = Some(handle);
        }
        true
    }

    /// Process a single queue item.
    async fn process_queue_item(
        queue: &BenchmarkQueue,
        session_manager: &SessionManager,
        benchmark_executor: &BenchmarkExecutor,
        item: BenchmarkQueueItem,
    ) -> Result<()> {
        let session = session_manager.create_session(
            item.agent_name.clone(),
            vec![item.language.clone()],
            item.model.clone(),
            item.thinking_level.clone(),
            Some(item.exercise.clone()),
            item.retry,
            3600_000, // 1 hour timeout
        );

        let mut session_clone = session.clone();
        let session_id = session.id.clone();

        // Mark queue item as running and link the session ID
        queue.set_session_id(&item.id, session_id.clone());

        info!("Starting benchmark execution for session: {}", session.id);

        // Execute the benchmark (session is passed mutably for output consumer setup)
        let result = benchmark_executor.execute(&mut session_clone, Some(session_manager)).await;

        // Wait for session to complete (no timeout - benchmarks can take hours)
        let session_id = session.id.clone();

        loop {
            let current_session = session_manager.get_session(&session_id);
            match current_session {
                Some(s) if s.status == RunStatus::COMPLETED => {
                    queue.complete_current(&item.id);
                    info!("Queue item completed: {}", item.id);
                    break;
                }
                Some(s) if s.status == RunStatus::FAILED || s.status == RunStatus::CANCELLED => {
                    queue.fail_current(&item.id);
                    warn!("Queue item failed: {}", item.id);
                    break;
                }
                Some(_) => {
                    // Session is still running, wait a bit and check again
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                None => {
                    queue.fail_current(&item.id);
                    error!("Session not found: {}", session_id);
                    break;
                }
            }
        }

        if let Err(e) = result {
            warn!("Benchmark execution error: {}", e);
        }

        Ok(())
    }

    /// Cancel a queue item.
    pub async fn cancel_queue_item(&self, item_id: &str) -> bool {
        self.queue.cancel_item(item_id)
    }

    /// Get all queue items.
    pub fn get_queue_items(&self) -> Vec<BenchmarkQueueItem> {
        self.queue.get_all_items()
    }

    /// Clear pending items from queue.
    pub fn clear_pending_queue(&self) {
        self.queue.clear_pending();
    }

    /// Clear completed and cancelled items from the queue.
    pub fn clear_completed_and_cancelled(&self) -> usize {
        self.queue.clear_terminal_items()
    }

    /// Get the number of currently active workers.
    pub async fn get_active_worker_count(&self) -> usize {
        *self.active_workers.lock().await
    }

    /// Get the configured parallelism limit.
    pub fn get_parallelism_limit(&self) -> usize {
        self.config.parallelism
    }

    /// Retry a failed queue item.
    pub fn retry_item(&self, item_id: &str) -> Option<BenchmarkQueueItem> {
        self.queue.retry_item(item_id)
    }

    /// Gracefully shut down the queue processor.
    pub async fn shutdown(&self) {
        info!("Shutting down queue processor...");
        let mut shutdown = self.shutdown_requested.lock().await;
        *shutdown = true;
        drop(shutdown);

        let mut task = self.queue_task.lock().await;
        if let Some(handle) = task.take() {
            handle.abort();
        }
    }
}
