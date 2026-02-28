package com.benchmark.web.service;

import com.benchmark.web.domain.BenchmarkQueue;
import com.benchmark.web.domain.BenchmarkQueueItem;
import com.benchmark.web.domain.BenchmarkSession;
import org.springframework.stereotype.Service;

import java.util.List;

/**
 * Facade service for benchmark operations.
 * Coordinates between SessionManager, BenchmarkExecutor, and QueueProcessor.
 */
@Service
public class BenchmarkService {

    private final SessionManager sessionManager;
    private final BenchmarkExecutor benchmarkExecutor;
    private final QueueProcessor queueProcessor;
    private final ResultService resultService;
    private final com.benchmark.exercise.ExerciseRunner exerciseRunner;

    public BenchmarkService(SessionManager sessionManager, BenchmarkExecutor benchmarkExecutor,
                           QueueProcessor queueProcessor, ResultService resultService,
                           com.benchmark.exercise.ExerciseRunner exerciseRunner) {
        this.sessionManager = sessionManager;
        this.benchmarkExecutor = benchmarkExecutor;
        this.queueProcessor = queueProcessor;
        this.resultService = resultService;
        this.exerciseRunner = exerciseRunner;
    }

    /**
     * Creates a new benchmark session and starts execution asynchronously.
     *
     * @param agentName    The agent to use ("reference" or "claude")
     * @param languages    The programming languages (can be multiple)
     * @param model        The model to use (optional, uses config default if null)
     * @param exerciseName The exercise name, or null for all exercises
     * @return The session ID
     */
    public String startBenchmark(String agentName, String[] languages, String model, String exerciseName) {
        BenchmarkSession session = sessionManager.createSession(agentName, languages, model, exerciseName);

        // Execute asynchronously via BenchmarkExecutor
        benchmarkExecutor.execute(session);

        return session.getId();
    }

    /**
     * Gets a session by ID.
     */
    public BenchmarkSession getSession(String sessionId) {
        return sessionManager.getSession(sessionId);
    }

    /**
     * Gets all sessions.
     */
    public java.util.Map<String, BenchmarkSession> getAllSessions() {
        return sessionManager.getAllSessions();
    }

    /**
     * Cancels a running session.
     */
    public boolean cancelSession(String sessionId) {
        return sessionManager.cancelSession(sessionId);
    }

    /**
     * Removes a completed session.
     */
    public void removeSession(String sessionId) {
        sessionManager.removeSession(sessionId);
    }

    // =============================================================================
    // Queue Management - Delegates to QueueProcessor
    // =============================================================================

    /**
     * Schedule a batch of benchmark runs.
     */
    public List<BenchmarkQueueItem> scheduleBatch(String agentName, String[] languages,
                                                   String model, String exercise) {
        return queueProcessor.scheduleBatch(agentName, languages, model, exercise);
    }

    /**
     * Get the benchmark queue.
     */
    public BenchmarkQueue getQueue() {
        return queueProcessor.getQueue();
    }

    /**
     * Cancel a queue item.
     */
    public boolean cancelQueueItem(String itemId) {
        return queueProcessor.cancelQueueItem(itemId);
    }

    /**
     * Get all queue items.
     */
    public List<BenchmarkQueueItem> getQueueItems() {
        return queueProcessor.getQueueItems();
    }

    /**
     * Clear pending items from queue.
     */
    public void clearPendingQueue() {
        queueProcessor.clearPendingQueue();
    }

    // =============================================================================
    // Result Service Access
    // =============================================================================

    /**
     * Refreshes the result cache.
     */
    public void refreshResultCache() {
        resultService.refreshCache();
    }

    /**
     * Gets the ExerciseRunner for discovering exercises.
     */
    public com.benchmark.exercise.ExerciseRunner getExerciseRunner() {
        return exerciseRunner;
    }
}
