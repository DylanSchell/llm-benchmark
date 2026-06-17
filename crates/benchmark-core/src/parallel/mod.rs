use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tracing::error;

/// Utility for parallel execution of async tasks with controlled concurrency.
///
/// This is the async (tokio-based) replacement for the Java `ParallelExecutor`.
/// It uses a semaphore-based approach to limit concurrency instead of a fixed
/// thread pool.
pub struct ParallelExecutor;

impl ParallelExecutor {
    /// Executes a list of async tasks in parallel with the specified concurrency limit.
    /// Returns results in the same order as input tasks, skipping failed tasks.
    ///
    /// # Arguments
    /// * `tasks` - List of async tasks (futures).
    /// * `parallelism` - Maximum number of concurrent tasks.
    ///
    /// # Returns
    /// List of successful results (None values filtered out).
    pub async fn execute_parallel<T, F, Fut>(
        tasks: Vec<F>,
        parallelism: usize,
    ) -> Vec<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Option<T>> + Send + 'static,
    {
        if tasks.is_empty() {
            return Vec::new();
        }

        let semaphore = Arc::new(Semaphore::new(parallelism));
        let mut handles = Vec::with_capacity(tasks.len());

        for task in tasks {
            let sem = Arc::clone(&semaphore);
            let handle: JoinHandle<Result<Option<T>, tokio::task::JoinError>> =
                tokio::spawn(async move {
                    let permit = match sem.acquire().await {
                        Ok(permit) => permit,
                        Err(e) => {
                            error!("Semaphore closed: {}", e);
                            return Ok(None);
                        }
                    };
                    let result = task().await;
                    drop(permit);
                    Ok(result)
                });
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(Some(result))) => results.push(result),
                Ok(Ok(None)) => {} // Task returned None, skip it
                Ok(Err(e)) => {
                    error!("Task execution failed: {}", e);
                }
                Err(e) => {
                    error!("Task join failed: {}", e);
                }
            }
        }

        results
    }

    /// Executes tasks and collects results, allowing null (None) values in the result list.
    ///
    /// # Arguments
    /// * `tasks` - List of async tasks (futures).
    /// * `parallelism` - Maximum number of concurrent tasks.
    ///
    /// # Returns
    /// List of all results (including None for failed tasks).
    pub async fn execute_parallel_allow_nulls<T, F, Fut>(
        tasks: Vec<F>,
        parallelism: usize,
    ) -> Vec<Option<T>>
    where
        T: Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Option<T>> + Send + 'static,
    {
        if tasks.is_empty() {
            return Vec::new();
        }

        let semaphore = Arc::new(Semaphore::new(parallelism));
        let mut handles = Vec::with_capacity(tasks.len());

        for task in tasks {
            let sem = Arc::clone(&semaphore);
            let handle: JoinHandle<Result<Option<T>, tokio::task::JoinError>> =
                tokio::spawn(async move {
                    let permit = match sem.acquire().await {
                        Ok(permit) => permit,
                        Err(e) => {
                            error!("Semaphore closed: {}", e);
                            return Ok(None);
                        }
                    };
                    let result = task().await;
                    drop(permit);
                    Ok(result)
                });
            handles.push(handle);
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => {
                    error!("Task execution failed: {}", e);
                    results.push(None);
                }
                Err(e) => {
                    error!("Task join failed: {}", e);
                    results.push(None);
                }
            }
        }

        results
    }

    /// Executes a list of tasks sequentially (for debugging or when parallelism = 1).
    pub async fn execute_sequential<T, F, Fut>(tasks: Vec<F>) -> Vec<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Option<T>> + Send + 'static,
    {
        let mut results = Vec::new();
        for task in tasks {
            match task().await {
                Some(result) => results.push(result),
                None => {}
            }
        }
        results
    }
}

/// Semaphore-based rate limiter for controlling throughput.
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
}

impl RateLimiter {
    /// Creates a new rate limiter with the given concurrency limit.
    pub fn new(limit: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(limit)),
        }
    }

    /// Acquires a permit. Returns a guard that releases the permit when dropped.
    pub async fn acquire(&self) -> Result<tokio::sync::SemaphorePermit<'_>, tokio::sync::AcquireError> {
        self.semaphore.acquire().await
    }

    /// Returns the remaining permits.
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}
