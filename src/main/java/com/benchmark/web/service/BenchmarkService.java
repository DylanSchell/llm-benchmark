package com.benchmark.web.service;

import com.benchmark.BenchmarkRunner;
import com.benchmark.agent.ReferenceAgent;
import com.benchmark.config.Config;
import com.benchmark.docker.DockerClient;
import com.benchmark.exercise.ExerciseResult;
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

    public BenchmarkService(Config config, DockerClient dockerClient, BenchmarkRunner benchmarkRunner,
                            ResultService resultService, ExecutorService executor) {
        this.config = config;
        this.dockerClient = dockerClient;
        this.benchmarkRunner = benchmarkRunner;
        this.resultService = resultService;
        this.sessions = new ConcurrentHashMap<>();
        this.executor = executor;
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
                session.setTotalExercises(languages.length);
                for (String language : languages) {
                    session.emitOutput("Running exercise: " + exerciseName + " for language: " + language);
                    ExerciseResult result = benchmarkRunner.runReferenceExercise(agent, language, exerciseName, effectiveModel, languages);
                    session.emitOutput(result.getOutput());

                    // Save result to file (saveResult also saves trace if available)
                    benchmarkRunner.saveResult(result, agentName, effectiveModel, languages);

                    session.incrementCompletedExercises();

                    if (!result.isSuccess()) {
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
     */
    private ReferenceAgent createAgent(String agentName) {
        if ("claude".equals(agentName)) {
            try {
                // Use reflection to create ClaudeAgent
                Class<?> claudeAgentClass = Class.forName("com.benchmark.agent.ClaudeAgent");
                var constructor = claudeAgentClass.getConstructor(DockerClient.class);
                return (ReferenceAgent) constructor.newInstance(dockerClient);
            } catch (Exception e) {
                logger.error("Failed to create Claude agent: {}", e.getMessage());
                throw new RuntimeException("Failed to create Claude agent", e);
            }
        }
        return new ReferenceAgent(dockerClient);
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
}
