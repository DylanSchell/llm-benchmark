package com.benchmark.web.service;

import com.benchmark.config.Config;
import com.benchmark.exercise.ExerciseRunner;
import com.benchmark.web.domain.BenchmarkQueue;
import com.benchmark.web.domain.BenchmarkQueueItem;
import com.benchmark.web.domain.BenchmarkSession;
import com.benchmark.web.domain.RunStatus;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.scheduling.annotation.Async;
import org.springframework.stereotype.Component;

import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Semaphore;
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
    private final Semaphore workerSemaphore;
    private final AtomicInteger activeWorkers = new AtomicInteger(0);

    public QueueProcessor(SessionManager sessionManager, ResultService resultService,
                         ExerciseRunner exerciseRunner, Config config, ExecutorService executor,
                         BenchmarkExecutor benchmarkExecutor) {
        this.sessionManager = sessionManager;
        this.resultService = resultService;
        this.exerciseRunner = exerciseRunner;
        this.config = config;
        this.executor = executor;
        this.benchmarkExecutor = benchmarkExecutor;
        this.workerSemaphore = new Semaphore(config.getParallelism());

        // Start queue worker
        startQueueWorker();
    }

    /**
     * Starts the queue worker that processes items concurrently up to parallelism limit.
     */
    @Async
    public void startQueueWorker() {
        executor.execute(() -> {
            logger.info("Queue worker started (parallelism={})", config.getParallelism());
            while (!Thread.currentThread().isInterrupted()) {
                try {
                    tryStartNextItem();
                    Thread.sleep(500); // Check twice per second
                } catch (InterruptedException e) {
                    logger.info("Queue worker interrupted, shutting down");
                    Thread.currentThread().interrupt();
                    break;
                } catch (Exception e) {
                    logger.error("Error in queue worker: {}", e.getMessage(), e);
                    try {
                        Thread.sleep(5000); // Wait 5 seconds before retrying
                    } catch (InterruptedException ie) {
                        Thread.currentThread().interrupt();
                        break;
                    }
                }
            }
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
                    item.getExercise()
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
     *
     * @param agentName   The agent to use
     * @param model       The model to use (can be null)
     * @param languages   Array of languages
     * @param exercise    Exercise name, or null for all exercises per language
     * @return List of queue items created
     */
    public List<BenchmarkQueueItem> scheduleBatch(String agentName, String[] languages,
                                                   String model, String exercise) {
        // Compute target directory once for the entire batch
        String effectiveModel = (model != null && !model.isEmpty()) ? model : null;

        List<BenchmarkQueueItem> items = new java.util.ArrayList<>();

        if (exercise != null && !exercise.isEmpty()) {
            // Single exercise specified - create one item per language for that exercise
            for (String language : languages) {
                String targetDir = config.getOutput().getResultsDir(agentName, effectiveModel, 
                        new String[]{language});
                BenchmarkQueueItem item = new BenchmarkQueueItem(targetDir, agentName, 
                        model, language, exercise);
                items.add(item);
            }
        } else {
            // No specific exercise - expand to individual items for each language/exercise combination
            for (String language : languages) {
                try {
                    List<String> exercises = exerciseRunner.getExercisesForLanguage(language);
                    String targetDir = config.getOutput().getResultsDir(agentName, effectiveModel, 
                            new String[]{language});
                    for (String exerciseName : exercises) {
                        BenchmarkQueueItem item = new BenchmarkQueueItem(targetDir, agentName, 
                                model, language, exerciseName);
                        items.add(item);
                    }
                    logger.info("Scheduled {} individual items for language: {}", 
                            exercises.size(), language);
                } catch (Exception e) {
                    logger.error("Failed to get exercises for language {}: {}", language, e.getMessage());
                }
            }
        }

        queue.addAll(items);
        logger.info("Scheduled {} queue items total", items.size());
        return items;
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
}
