package io.schell.llm.benchmark.web.service;

import io.schell.llm.benchmark.BenchmarkRunner;
import io.schell.llm.benchmark.config.Config;
import io.schell.llm.benchmark.config.QuickBenchConfig;
import io.schell.llm.benchmark.exercise.ExerciseRunner;
import io.schell.llm.benchmark.util.StringUtil;
import io.schell.llm.benchmark.web.domain.BenchmarkQueue;
import io.schell.llm.benchmark.web.domain.BenchmarkQueueItem;
import io.schell.llm.benchmark.web.domain.BenchmarkSession;
import io.schell.llm.benchmark.web.domain.RunStatus;
import jakarta.annotation.PreDestroy;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.scheduling.annotation.Async;
import org.springframework.stereotype.Component;

import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Semaphore;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Processes benchmark queue items.
 * Handles queue management and concurrent processing of queued benchmark runs.
 * Respects the parallelism setting from config.yaml.
 */
@Component
public class QueueProcessor {
    private static final Logger logger = LoggerFactory.getLogger(QueueProcessor.class);

    private final BenchmarkQueue queue = new BenchmarkQueue();
    private final SessionManager sessionManager;
    private final ResultService resultService;
    private final ExerciseRunner exerciseRunner;
    private final Config config;
    private final ExecutorService executor;
    private final BenchmarkExecutor benchmarkExecutor;
    private final BenchmarkRunner benchmarkRunner;
    private final Semaphore workerSemaphore;
    private final AtomicInteger activeWorkers = new AtomicInteger(0);
    private final AtomicBoolean shutdownRequested = new AtomicBoolean(false);
    private volatile Thread queueWorkerThread;

    public QueueProcessor(SessionManager sessionManager, ResultService resultService,
                         ExerciseRunner exerciseRunner, Config config, ExecutorService executor,
                         BenchmarkExecutor benchmarkExecutor, BenchmarkRunner benchmarkRunner) {
        this.sessionManager = sessionManager;
        this.resultService = resultService;
        this.exerciseRunner = exerciseRunner;
        this.config = config;
        this.executor = executor;
        this.benchmarkExecutor = benchmarkExecutor;
        this.benchmarkRunner = benchmarkRunner;
        this.workerSemaphore = new Semaphore(config.getParallelism());

        // Start queue worker
        startQueueWorker();
    }

    /**
     * Gracefully shut down the queue processor.
     */
    @PreDestroy
    public void shutdown() {
        logger.info("Shutting down queue processor...");
        shutdownRequested.set(true);
        if (queueWorkerThread != null) {
            queueWorkerThread.interrupt();
        }
    }

    /**
     * Starts the queue worker that processes items concurrently up to parallelism limit.
     */
    @Async
    public void startQueueWorker() {
        queueWorkerThread = Thread.currentThread();
        executor.execute(() -> {
            logger.info("Queue worker started (parallelism={})", config.getParallelism());
            Thread workerThread = Thread.currentThread();
            while (!workerThread.isInterrupted() && !shutdownRequested.get()) {
                try {
                    tryStartNextItem();
                    // Use a shorter sleep to respond faster to shutdown
                    for (int i = 0; i < 10 && !workerThread.isInterrupted() && !shutdownRequested.get(); i++) {
                        Thread.sleep(50);
                    }
                } catch (InterruptedException e) {
                    logger.info("Queue worker interrupted, shutting down");
                    workerThread.interrupt();
                    break;
                } catch (Exception e) {
                    logger.error("Error in queue worker: {}", e.getMessage(), e);
                    try {
                        // Sleep in smaller increments to be more responsive to shutdown
                        for (int i = 0; i < 100 && !workerThread.isInterrupted() && !shutdownRequested.get(); i++) {
                            Thread.sleep(50);
                        }
                    } catch (InterruptedException ie) {
                        workerThread.interrupt();
                        break;
                    }
                }
            }
            logger.info("Queue worker shut down");
        });
    }

    /**
     * Try to start processing the next available queue item.
     * Only starts a new item if we have capacity (active workers < parallelism).
     */
    private void tryStartNextItem() {
        // Check if we have capacity
        if (activeWorkers.get() >= config.getParallelism()) {
            return; // At max capacity
        }

        // Try to acquire a permit (non-blocking check via semaphore)
        if (!workerSemaphore.tryAcquire()) {
            return; // No permits available
        }

        BenchmarkQueueItem item = queue.pollNext();
        if (item == null) {
            workerSemaphore.release(); // Release permit since no item to process
            return;
        }

        // We have an item and a permit - start processing
        activeWorkers.incrementAndGet();
        logger.info("Starting queue item: {} - {}/{} (active workers: {}/{})", 
                item.getId(), item.getLanguage(), item.getExercise(),
                activeWorkers.get(), config.getParallelism());

        // Process the item asynchronously
        executor.execute(() -> {
            try {
                processQueueItem(item);
            } finally {
                activeWorkers.decrementAndGet();
                workerSemaphore.release();
                logger.info("Completed queue item: {} (active workers: {})", 
                        item.getId(), activeWorkers.get());
            }
        });
    }

    /**
     * Process a single queue item.
     */
    private void processQueueItem(BenchmarkQueueItem item) {
        String sessionId = null;
        try {
            // Create session for this queue item
            BenchmarkSession session = sessionManager.createSession(
                    item.getAgentName(),
                    new String[]{item.getLanguage()},
                    item.getModel(),
                    item.getExercise(),
                    item.isRetry()
            );
            sessionId = session.getId();
            item.setSessionId(sessionId);

            logger.info("Starting benchmark execution for session: {}", sessionId);
            
            // Start the benchmark execution
            benchmarkExecutor.execute(session);

            // Wait for session to complete
            while (session != null && 
                   (session.getStatus() == RunStatus.PENDING ||
                    session.getStatus() == RunStatus.RUNNING)) {
                Thread.sleep(500);
                session = sessionManager.getSession(sessionId); // Refresh session status
            }

            if (session != null) {
                if (session.getStatus() == RunStatus.COMPLETED) {
                    queue.completeCurrent();
                    logger.info("Queue item completed: {}", item.getId());
                } else if (session.getStatus() == RunStatus.FAILED ||
                           session.getStatus() == RunStatus.CANCELLED) {
                    queue.failCurrent();
                    logger.warn("Queue item failed: {}", item.getId());
                }
            } else {
                queue.failCurrent();
            }

        } catch (Exception e) {
            logger.error("Queue item failed: {}", e.getMessage(), e);
            if (sessionId != null) {
                BenchmarkSession session = sessionManager.getSession(sessionId);
                if (session != null && session.getStatus() == RunStatus.RUNNING) {
                    session.setStatus(RunStatus.FAILED);
                    session.setErrorMessage(e.getMessage());
                }
            }
            queue.failCurrent();
        }
    }

    /**
     * Schedule a batch of benchmark runs.
     * Creates individual queue items for each language/exercise combination.
     * Never uses "all" exercises - always expands to individual exercise items.
     * Skips exercises that have already been completed successfully (unless retry mode).
     *
     * @param agentName   The agent to use
     * @param model       The model to use (can be null)
     * @param languages   Array of languages
     * @param exercise    Exercise name, or null for all exercises per language,
     *                    or "__quick__" for quick bench mode
     * @return List of queue items created
     */
    public List<BenchmarkQueueItem> scheduleBatch(String agentName, String[] languages,
                                                   String model, String exercise) {
        return scheduleBatch(agentName, languages, model, exercise, false);
    }

    /**
     * Schedule a batch of benchmark runs with optional retry mode.
     *
     * @param agentName   The agent to use
     * @param model       The model to use (can be null)
     * @param languages   Array of languages
     * @param exercise    Exercise name, or null for all exercises per language,
     *                    or "__quick__" for quick bench mode
     * @param retry       If true, do not skip already-successful exercises
     * @return List of queue items created
     */
    public List<BenchmarkQueueItem> scheduleBatch(String agentName, String[] languages,
                                                   String model, String exercise, boolean retry) {
        String effectiveModel = StringUtil.toNonNull(model);

        List<BenchmarkQueueItem> items = new java.util.ArrayList<>();
        int skippedCount = 0;

        if ("__quick__".equals(exercise)) {
            // Quick bench mode - use curated list of fast exercises (< 60s)
            for (String language : languages) {
                List<String> quickExercises = QuickBenchConfig.getExercisesForLanguage(language);
                if (quickExercises.isEmpty()) {
                    logger.warn("No quick-bench exercises defined for language: {}", language);
                    continue;
                }
                for (String exerciseName : quickExercises) {
                    if (!retry && benchmarkRunner.resultFileSuccess(exerciseName, agentName, effectiveModel, language, languages)) {
                        logger.debug("Skipping quick-bench exercise: {} for language: {} (already completed successfully)", 
                                exerciseName, language);
                        skippedCount++;
                        continue;
                    }

                    String targetDir = config.getOutput().getResultsDir(agentName, effectiveModel,
                            new String[]{language});
                    BenchmarkQueueItem item = new BenchmarkQueueItem(targetDir, agentName,
                            model, language, exerciseName, retry);
                    items.add(item);
                }
            }
        } else if (exercise != null && !exercise.isEmpty()) {
            // Single exercise specified - create one item per language for that exercise
            for (String language : languages) {
                if (!retry && benchmarkRunner.resultFileSuccess(exercise, agentName, effectiveModel, language, languages)) {
                    logger.info("Skipping exercise: {} for language: {} (already completed successfully)", 
                            exercise, language);
                    skippedCount++;
                    continue;
                }
                
                String targetDir = config.getOutput().getResultsDir(agentName, effectiveModel, 
                        new String[]{language});
                BenchmarkQueueItem item = new BenchmarkQueueItem(targetDir, agentName, 
                        model, language, exercise, retry);
                items.add(item);
            }
        } else {
            // No specific exercise - expand to individual items for each language/exercise combination
            for (String language : languages) {
                try {
                    List<String> exercises = exerciseRunner.getExercisesForLanguage(language);
                    for (String exerciseName : exercises) {
                        if (!retry && benchmarkRunner.resultFileSuccess(exerciseName, agentName, effectiveModel, language, languages)) {
                            logger.debug("Skipping exercise: {} for language: {} (already completed successfully)", 
                                    exerciseName, language);
                            skippedCount++;
                            continue;
                        }
                        
                        String targetDir = config.getOutput().getResultsDir(agentName, effectiveModel, 
                                new String[]{language});
                        BenchmarkQueueItem item = new BenchmarkQueueItem(targetDir, agentName, 
                                model, language, exerciseName, retry);
                        items.add(item);
                    }
                } catch (Exception e) {
                    logger.error("Failed to get exercises for language {}: {}", language, e.getMessage());
                }
            }
        }

        queue.addAll(items);
        logger.info("Scheduled {} queue items total (skipped {} already successful)", items.size(), skippedCount);
        return items;
    }

    /**
     * Counts how many exercises for a language have already been completed successfully.
     */
    private int countSkippedForLanguage(List<String> exercises, String agentName, String model, String language) {
        int count = 0;
        for (String exerciseName : exercises) {
            if (benchmarkRunner.resultFileSuccess(exerciseName, agentName, model, language, new String[]{language})) {
                count++;
            }
        }
        return count;
    }

    /**
     * Get the benchmark queue.
     */
    public BenchmarkQueue getQueue() {
        return queue;
    }

    /**
     * Cancel a queue item.
     */
    public boolean cancelQueueItem(String itemId) {
        return queue.cancelItem(itemId);
    }

    /**
     * Get all queue items (pending, running, completed).
     */
    public List<BenchmarkQueueItem> getQueueItems() {
        return queue.getAllItems();
    }

    /**
     * Clear pending items from queue.
     */
    public void clearPendingQueue() {
        queue.clearPending();
    }

    /**
     * Clear completed and cancelled items from the queue.
     * @return Number of items removed
     */
    public int clearCompletedAndCancelled() {
        return queue.clearTerminalItems();
    }

    /**
     * Check if currently processing any queue items.
     */
    public boolean isProcessingItem() {
        return activeWorkers.get() > 0;
    }

    /**
     * Get the number of currently active workers.
     */
    public int getActiveWorkerCount() {
        return activeWorkers.get();
    }

    /**
     * Get the configured parallelism limit.
     */
    public int getParallelismLimit() {
        return config.getParallelism();
    }

    /**
     * Retry a failed queue item by re-adding it to the pending queue.
     * @param itemId The ID of the failed item to retry
     * @return The new queue item, or null if not found
     */
    public BenchmarkQueueItem retryItem(String itemId) {
        return queue.retryItem(itemId);
    }
}
