package io.schell.llm.benchmark.web.service;

import io.schell.llm.benchmark.BenchmarkRunner;
import io.schell.llm.benchmark.agent.AgentFactory;
import io.schell.llm.benchmark.agent.ReferenceAgent;
import io.schell.llm.benchmark.config.Config;
import io.schell.llm.benchmark.docker.DockerClient;
import io.schell.llm.benchmark.exercise.ExerciseResult;
import io.schell.llm.benchmark.exception.BenchmarkExecutionException;
import io.schell.llm.benchmark.util.StringUtil;
import io.schell.llm.benchmark.web.domain.BenchmarkSession;
import io.schell.llm.benchmark.web.domain.RunStatus;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.util.List;

/**
 * Executes benchmark runs.
 * Handles the actual execution of exercises and agents.
 */
@Component
public class BenchmarkExecutor {
    private static final Logger logger = LoggerFactory.getLogger(BenchmarkExecutor.class);

    private final BenchmarkRunner benchmarkRunner;
    private final DockerClient dockerClient;
    private final Config config;

    public BenchmarkExecutor(BenchmarkRunner benchmarkRunner, DockerClient dockerClient, Config config) {
        this.benchmarkRunner = benchmarkRunner;
        this.dockerClient = dockerClient;
        this.config = config;
    }

    /**
     * Executes a benchmark run for the given session.
     *
     * @param session The benchmark session
     */
    public void execute(BenchmarkSession session) {
        try {
            session.setStatus(RunStatus.RUNNING);

            String[] languages = session.getLanguages();
            String agentName = session.getAgentName();
            String model = session.getModel();
            String exerciseName = session.getExerciseName();

            benchmarkRunner.getExerciseRunner().setRunParams(agentName, StringUtil.toNonNull(model), languages);

            ReferenceAgent agent = createAgent(agentName);

            // Set up output consumer for live streaming to web UI
            agent.setOutputConsumer(session::emitOutput);

            if (exerciseName != null && !exerciseName.isEmpty()) {
                executeSingleExercise(session, agent, languages, StringUtil.toNonNull(model));
            } else {
                executeAllExercises(session, agent, languages, StringUtil.toNonNull(model));
            }

            session.completeOutput();

        } catch (Exception e) {
            logger.error("Benchmark execution failed: {}", e.getMessage(), e);
            session.setStatus(RunStatus.FAILED);
            session.setErrorMessage(e.getMessage());
            session.emitOutput("Error: " + e.getMessage());
            session.completeOutput();
        }
    }

    /**
     * Executes a single exercise across all selected languages.
     */
    private void executeSingleExercise(BenchmarkSession session, ReferenceAgent agent, 
                                       String[] languages, String effectiveModel) {
        String exerciseName = session.getExerciseName();

        for (String language : languages) {
            // Check for cancellation
            if (session.getStatus() == RunStatus.CANCELLED) {
                session.emitOutput("Benchmark cancelled");
                return;
            }

            session.emitOutput("Running exercise: " + exerciseName + " for language: " + language);
            
            try {
                ExerciseResult result = benchmarkRunner.runReferenceExercise(agent, language, 
                        exerciseName, effectiveModel, languages);
                session.emitOutput(result.getOutput());

                // Save result to file (pass retry flag for attempts/timing behavior)
                benchmarkRunner.saveResult(result, session.getAgentName(), effectiveModel, 
                        language, languages, session.isRetry());

                session.incrementCompletedExercises();

                if (!result.isSuccess()) {
                    session.setStatus(RunStatus.FAILED);
                    String errorMsg = "Exercise failed for language " + language + ": " + 
                            result.getErrorMessage();
                    session.setErrorMessage(errorMsg);
                    session.emitOutput(errorMsg);
                    return;
                }
            } catch (Exception e) {
                throw new BenchmarkExecutionException(exerciseName, language, e);
            }
        }

        session.setStatus(RunStatus.COMPLETED);
        session.emitOutput("All exercises completed successfully!");
    }

    /**
     * Executes all exercises for all selected languages.
     */
    private void executeAllExercises(BenchmarkSession session, ReferenceAgent agent, 
                                     String[] languages, String effectiveModel) {
        int totalExercises = 0;
        int successfulExercises = 0;

        for (String language : languages) {
            // Check for cancellation
            if (session.getStatus() == RunStatus.CANCELLED) {
                session.emitOutput("Benchmark cancelled");
                return;
            }

            session.emitOutput("Running all exercises for language: " + language);
            
            try {
                List<ExerciseResult> results = benchmarkRunner.runAllReferenceExercises(
                        agent, language, session.getAgentName(), effectiveModel, languages);
                totalExercises += results.size();

                long languageSuccessful = results.stream().filter(ExerciseResult::isSuccess).count();
                successfulExercises += (int) languageSuccessful;

                long failed = results.size() - languageSuccessful;
                if (failed > 0) {
                    session.setStatus(RunStatus.FAILED);
                    session.emitOutput(failed + " exercises failed for language " + 
                            language + " out of " + results.size());
                } else {
                    session.emitOutput("All exercises completed successfully for language: " + language);
                }
            } catch (Exception e) {
                throw new BenchmarkExecutionException("all exercises", language, e);
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

    /**
     * Creates an agent instance based on name.
     */
    private ReferenceAgent createAgent(String agentName) {
        try {
            return AgentFactory.createAgent(agentName, dockerClient);
        } catch (IllegalArgumentException e) {
            logger.error("Failed to create agent: {}", e.getMessage());
            throw new RuntimeException("Failed to create agent: " + agentName, e);
        }
    }
}
