package com.benchmark.web.service;

import com.benchmark.BenchmarkRunner;
import com.benchmark.agent.AgentFactory;
import com.benchmark.agent.ReferenceAgent;
import com.benchmark.config.Config;
import com.benchmark.docker.DockerClient;
import com.benchmark.exercise.ExerciseResult;
import com.benchmark.web.domain.BenchmarkQueue;
import com.benchmark.web.domain.BenchmarkQueueItem;
import com.benchmark.web.domain.BenchmarkSession;
import com.benchmark.web.domain.RunStatus;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.function.Consumer;

/**
 * Service for managing benchmark runs and sessions.
 * Wraps the existing BenchmarkRunner and ExerciseRunner for web usage.
 */
@Service
public class BenchmarkService {
    private static final Logger logger = LoggerFactory.getLogger(BenchmarkService.class);

    private final Config config;
    private final DockerClient dockerClient;
    private final BenchmarkRunner benchmarkRunner;
    private final ResultService resultService;
    private final Map<String, BenchmarkSession> sessions;
    private final ExecutorService executor;
    private final BenchmarkQueue queue;
    private volatile boolean processingItem = false;

    public BenchmarkService(Config config, DockerClient dockerClient, BenchmarkRunner benchmarkRunner,
                            ResultService resultService, ExecutorService executor) {
        this.config = config;
        this.dockerClient = dockerClient;
        this.benchmarkRunner = benchmarkRunner;
        this.resultService = resultService;
        this.sessions = new ConcurrentHashMap<>();
        this.executor = executor;
        this.queue = new BenchmarkQueue();

        // Start queue worker
        startQueueWorker();
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
        String sessionId = UUID.randomUUID().toString();
        BenchmarkSession session = new BenchmarkSession(sessionId, agentName, languages, model, exerciseName);
        sessions.put(sessionId, session);

        logger.info("Starting benchmark session: {} for {}/{} (model: {})", sessionId, String.join(",", languages),
                exerciseName != null ? exerciseName : "all", model);

        // Run asynchronously
        CompletableFuture.runAsync(() -> runBenchmark(session, agentName, languages, model, exerciseName), executor);

        return sessionId;
    }

    /**
     * Runs the benchmark synchronously.
     */
    private void runBenchmark(BenchmarkSession session, String agentName, String[] languages, String model, String exerciseName) {
        try {
            session.setStatus(RunStatus.RUNNING);

            // Set run parameters for result directory computation
            // Treat empty string as null for proper directory naming
            String effectiveModel = (model != null && !model.isEmpty()) ? model : null;
            benchmarkRunner.getExerciseRunner().setRunParams(agentName, effectiveModel, languages);

            ReferenceAgent agent = createAgent(agentName);

            // Set up output consumer for live streaming to web UI
            agent.setOutputConsumer(line -> session.emitOutput(line));

            if (exerciseName != null && !exerciseName.isEmpty()) {
                // Single exercise across all selected languages
                int totalExercises = 0;
                int successfulExercises = 0;

                for (String language : languages) {
                    // Check for cancellation
                    if (session.getStatus() == RunStatus.CANCELLED) {
                        session.emitOutput("Benchmark cancelled");
                        return;
                    }

                    // Skip if result already exists and was successful
                    if (benchmarkRunner.resultFileSuccess(exerciseName, agentName, effectiveModel, language, languages)) {
                        session.emitOutput("Skipping exercise: " + exerciseName + " for language: " + language + " (already completed successfully)");
                        totalExercises++;
                        successfulExercises++;
                        continue;
                    }

                    session.emitOutput("Running exercise: " + exerciseName + " for language: " + language);
                    ExerciseResult result = benchmarkRunner.runReferenceExercise(agent, language, exerciseName, effectiveModel, languages);
                    session.emitOutput(result.getOutput());

                    // Save result to file (saveResult also saves trace if available)
                    benchmarkRunner.saveResult(result, agentName, effectiveModel, language, languages);

                    totalExercises++;
                    session.incrementCompletedExercises();

                    if (result.isSuccess()) {
                        successfulExercises++;
                    } else {
                        session.setStatus(RunStatus.FAILED);
                        session.setErrorMessage("Exercise failed for language " + language + ": " + result.getErrorMessage());
                        session.emitOutput("Exercise failed for " + language + ": " + result.getErrorMessage());
                    }
                }

                if (session.getStatus() != RunStatus.FAILED) {
                    session.setStatus(RunStatus.COMPLETED);
                    session.emitOutput("All exercises completed successfully!");
                }
            } else {
                // All exercises for all selected languages
                int totalExercises = 0;
                int successfulExercises = 0;

                for (String language : languages) {
                    // Check for cancellation
                    if (session.getStatus() == RunStatus.CANCELLED) {
                        session.emitOutput("Benchmark cancelled");
                        return;
                    }

                    session.emitOutput("Running all exercises for language: " + language);
                    List<ExerciseResult> results = benchmarkRunner.runAllReferenceExercises(agent, language, agentName, effectiveModel, languages);
                    totalExercises += results.size();

                    long languageSuccessful = results.stream().filter(ExerciseResult::isSuccess).count();
                    successfulExercises += (int) languageSuccessful;

                    long failed = results.size() - languageSuccessful;
                    if (failed > 0) {
                        session.setStatus(RunStatus.FAILED);
                        session.emitOutput(failed + " exercises failed for language " + language + " out of " + results.size());
                    } else {
                        session.emitOutput("All exercises completed successfully for language: " + language);
                    }
                }

                session.setTotalExercises(totalExercises);
                session.setCompletedExercises(successfulExercises);

                if (session.getStatus() != RunStatus.FAILED) {
                    session.setStatus(RunStatus.COMPLETED);
                    session.emitOutput("All exercises in all languages completed successfully!");
                } else {
                    session.setErrorMessage("Some exercises failed");
                }
            }

            session.completeOutput();

            // Refresh result cache after benchmark completes
            resultService.refreshCache();

        } catch (Exception e) {
            logger.error("Benchmark failed: {}", e.getMessage(), e);
            session.setStatus(RunStatus.FAILED);
            session.setErrorMessage(e.getMessage());
            session.emitOutput("Error: " + e.getMessage());
            session.completeOutput();
        }
    }

    /**
     * Creates an agent instance based on name.
     * Uses AgentFactory instead of reflection for better type safety and testability.
     */
    private ReferenceAgent createAgent(String agentName) {
        try {
            return AgentFactory.createAgent(agentName, dockerClient);
        } catch (IllegalArgumentException e) {
            logger.error("Failed to create agent: {}", e.getMessage());
            throw new RuntimeException("Failed to create agent: " + agentName, e);
        }
    }

    /**
     * Gets a session by ID.
     */
    public BenchmarkSession getSession(String sessionId) {
        return sessions.get(sessionId);
    }

    /**
     * Gets all sessions.
     */
    public Map<String, BenchmarkSession> getAllSessions() {
        return new ConcurrentHashMap<>(sessions);
    }

    /**
     * Cancels a running session.
     */
    public boolean cancelSession(String sessionId) {
        BenchmarkSession session = sessions.get(sessionId);
        if (session != null && session.getStatus() == RunStatus.RUNNING) {
            session.setStatus(RunStatus.CANCELLED);
            session.emitOutput("Cancelled by user");
            session.completeOutput();
            return true;
        }
        return false;
    }

    /**
     * Removes a completed session.
     */
    public void removeSession(String sessionId) {
        sessions.remove(sessionId);
    }

    // =============================================================================
    // Queue Management
    // =============================================================================

    /**
     * Starts the queue worker that processes items sequentially.
     */
    private void startQueueWorker() {
        CompletableFuture.runAsync(() -> {
            while (true) {
                try {
                    // Check for next item in queue - only if not already processing
                    if (!processingItem) {
                        BenchmarkQueueItem item = queue.pollNext();
                        if (item != null) {
                            processingItem = true;
                            logger.info("Processing queue item: {} - {}/{}",
                                    item.getId(), item.getLanguage(), item.getExercise());
                            processQueueItem(item);
                            processingItem = false;
                        } else {
                            // No items pending, wait before checking again
                            Thread.sleep(1000);
                        }
                    } else {
                        // Wait for current item to complete before checking for more
                        Thread.sleep(500);
                    }
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    break;
                } catch (Exception e) {
                    logger.error("Error processing queue item: {}", e.getMessage(), e);
                    processingItem = false;
                }
            }
        }, executor);
    }

    /**
     * Process a single queue item.
     */
    private void processQueueItem(BenchmarkQueueItem item) {
        String sessionId = null;
        try {
            // Create session for this queue item
            sessionId = startBenchmark(
                    item.getAgentName(),
                    new String[]{item.getLanguage()},
                    item.getModel(),
                    item.getExercise()
            );
            item.setSessionId(sessionId);

            // Wait for session to complete
            BenchmarkSession session = sessions.get(sessionId);
            while (session != null &&
                   (session.getStatus() == RunStatus.PENDING ||
                    session.getStatus() == RunStatus.RUNNING)) {
                Thread.sleep(500);
            }

            if (session != null) {
                if (session.getStatus() == RunStatus.COMPLETED) {
                    queue.completeCurrent();
                } else if (session.getStatus() == RunStatus.FAILED ||
                           session.getStatus() == RunStatus.CANCELLED) {
                    queue.failCurrent();
                }
            } else {
                queue.failCurrent();
            }

        } catch (Exception e) {
            logger.error("Queue item failed: {}", e.getMessage(), e);
            if (sessionId != null) {
                BenchmarkSession session = sessions.get(sessionId);
                if (session != null && session.getStatus() == RunStatus.RUNNING) {
                    session.setStatus(RunStatus.FAILED);
                    session.setErrorMessage(e.getMessage());
                }
            }
            queue.failCurrent();
        } finally {
            processingItem = false;
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
                String targetDir = config.getOutput().getResultsDir(agentName, effectiveModel, new String[]{language});
                BenchmarkQueueItem item = new BenchmarkQueueItem(targetDir, agentName, model, language, exercise);
                items.add(item);
            }
        } else {
            // No specific exercise - expand to individual items for each language/exercise combination
            // Never create bulk "all exercises" items - always schedule individually
            for (String language : languages) {
                try {
                    List<String> exercises = benchmarkRunner.getExerciseRunner().getExercisesForLanguage(language);
                    String targetDir = config.getOutput().getResultsDir(agentName, effectiveModel, new String[]{language});
                    for (String exerciseName : exercises) {
                        BenchmarkQueueItem item = new BenchmarkQueueItem(targetDir, agentName, model, language, exerciseName);
                        items.add(item);
                    }
                    logger.info("Scheduled {} individual items for language: {}", exercises.size(), language);
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
     * Get the ExerciseRunner for discovering exercises.
     */
    public com.benchmark.exercise.ExerciseRunner getExerciseRunner() {
        return benchmarkRunner.getExerciseRunner();
    }
}
