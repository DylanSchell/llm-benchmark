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

/**
 * Processes benchmark queue items.
 * Handles queue management and sequential processing of queued benchmark runs.
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
    private volatile boolean processingItem = false;

    public QueueProcessor(SessionManager sessionManager, ResultService resultService,
                         ExerciseRunner exerciseRunner, Config config, ExecutorService executor,
                         BenchmarkExecutor benchmarkExecutor) {
        this.sessionManager = sessionManager;
        this.resultService = resultService;
        this.exerciseRunner = exerciseRunner;
        this.config = config;
        this.executor = executor;
        this.benchmarkExecutor = benchmarkExecutor;

        // Start queue worker
        startQueueWorker();
    }

    /**
     * Starts the queue worker that processes items sequentially.
     */
    @Async
    public void startQueueWorker() {
        executor.execute(() -> {
            logger.info("Queue worker started");
            while (!Thread.currentThread().isInterrupted()) {
                try {
                    processNextItem();
                    Thread.sleep(1000); // Check every second
                } catch (InterruptedException e) {
                    logger.info("Queue worker interrupted, shutting down");
                    Thread.currentThread().interrupt();
                    break;
                } catch (Exception e) {
                    logger.error("Error in queue worker: {}", e.getMessage(), e);
                    processingItem = false;
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
     * Process the next item in the queue.
     */
    private void processNextItem() {
        if (processingItem) {
            return; // Already processing
        }

        BenchmarkQueueItem item = queue.pollNext();
        if (item == null) {
            return; // No items pending
        }

        processingItem = true;
        logger.info("Processing queue item: {} - {}/{}", 
                item.getId(), item.getLanguage(), item.getExercise());

        try {
            processQueueItem(item);
        } finally {
            processingItem = false;
        }
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
     * Check if currently processing a queue item.
     */
    public boolean isProcessingItem() {
        return processingItem;
    }
}
