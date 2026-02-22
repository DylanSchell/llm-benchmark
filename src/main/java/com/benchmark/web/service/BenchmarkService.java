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
     * @param language     The programming language
     * @param exerciseName The exercise name, or null for all exercises
     * @return The session ID
     */
    public String startBenchmark(String agentName, String language, String exerciseName) {
        String sessionId = UUID.randomUUID().toString();
        BenchmarkSession session = new BenchmarkSession(sessionId, agentName, language, exerciseName);
        sessions.put(sessionId, session);

        logger.info("Starting benchmark session: {} for {}/{}", sessionId, language,
                exerciseName != null ? exerciseName : "all");

        // Run asynchronously
        CompletableFuture.runAsync(() -> runBenchmark(session, agentName, language, exerciseName), executor);

        return sessionId;
    }

    /**
     * Runs the benchmark synchronously.
     */
    private void runBenchmark(BenchmarkSession session, String agentName, String language, String exerciseName) {
        try {
            session.setStatus(RunStatus.RUNNING);
            ReferenceAgent agent = createAgent(agentName);

            if (exerciseName != null && !exerciseName.isEmpty()) {
                // Single exercise
                session.setTotalExercises(1);
                session.emitOutput("Running exercise: " + exerciseName);
                ExerciseResult result = benchmarkRunner.runReferenceExercise(agent, language, exerciseName);
                session.emitOutput(result.getOutput());
                session.incrementCompletedExercises();

                if (result.isSuccess()) {
                    session.setStatus(RunStatus.COMPLETED);
                    session.emitOutput("Exercise completed successfully!");
                } else {
                    session.setStatus(RunStatus.FAILED);
                    session.setErrorMessage(result.getErrorMessage());
                    session.emitOutput("Exercise failed: " + result.getErrorMessage());
                }
            } else {
                // All exercises for language
                session.emitOutput("Running all exercises for language: " + language);
                List<ExerciseResult> results = benchmarkRunner.runAllReferenceExercises(agent, language, agentName);
                session.setTotalExercises(results.size());
                session.setCompletedExercises((int) results.stream().filter(ExerciseResult::isSuccess).count());

                long failed = results.stream().filter(r -> !r.isSuccess()).count();
                if (failed == 0) {
                    session.setStatus(RunStatus.COMPLETED);
                    session.emitOutput("All exercises completed successfully!");
                } else {
                    session.setStatus(RunStatus.FAILED);
                    session.setErrorMessage(failed + " exercises failed");
                    session.emitOutput(failed + " exercises failed out of " + results.size());
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
