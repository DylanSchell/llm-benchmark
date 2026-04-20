package com.benchmark;

import com.benchmark.agent.ReferenceAgent;
import com.benchmark.config.Config;
import com.benchmark.config.ConfigLoader;
import com.benchmark.docker.DockerClient;
import com.benchmark.exercise.ExerciseResult;
import com.benchmark.exercise.ExerciseRunner;
import com.benchmark.persistence.ResultPersister;
import com.benchmark.util.Languages;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Main entry point for the benchmark runner.
 * Provides convenient methods for running exercises.
 */
public class BenchmarkRunner {
    private static final Logger logger = LoggerFactory.getLogger(BenchmarkRunner.class);

    private final Config config;
    private final DockerClient dockerClient;
    private final ResultPersister resultPersister;
    private ExerciseRunner exerciseRunner;



    public BenchmarkRunner(Path configPath) throws Exception {
        this(ConfigLoader.load(configPath), new DockerClient(ConfigLoader.load(configPath).getDocker()));
    }

    /**
     * Constructor for Spring DI - accepts Config and DockerClient directly.
     * Note: ExerciseRunner is created lazily to avoid circular dependency.
     */
    public BenchmarkRunner(Config config, DockerClient dockerClient) throws Exception {
        this.config = config;
        this.dockerClient = dockerClient;
        this.resultPersister = new ResultPersister(config.getOutput());
        this.exerciseRunner = null; // Will be set via setter or created lazily
    }

    /**
     * Sets the ExerciseRunner - used by Spring for proper wiring.
     */
    public void setExerciseRunner(ExerciseRunner exerciseRunner) {
        this.exerciseRunner = exerciseRunner;
    }

    /**
     * Gets or creates the ExerciseRunner lazily.
     */
    public ExerciseRunner getExerciseRunner() {
        if (exerciseRunner == null) {
            exerciseRunner = new ExerciseRunner(config, dockerClient, this);
        }
        return exerciseRunner;
    }

    /**
     * Sets run parameters for result directory computation.
     * Delegates to ExerciseRunner for storage and use.
     */
    public void setRunParams(String agentName, String model, String[] languages) {
        getExerciseRunner().setRunParams(agentName, model, languages);
    }

    /**
     * Gets the current run agent name from ExerciseRunner.
     */
    public String getRunAgentName() {
        return exerciseRunner != null ? exerciseRunner.getRunAgentName() : null;
    }

    /**
     * Gets the current run model from ExerciseRunner.
     */
    public String getRunModel() {
        return exerciseRunner != null ? exerciseRunner.getRunModel() : null;
    }

    /**
     * Gets the current run languages from ExerciseRunner.
     */
    public String[] getRunLanguages() {
        return exerciseRunner != null ? exerciseRunner.getRunLanguages() : new String[]{};
    }

    /**
     * Gets the ResultPersister for result persistence operations.
     */
    public ResultPersister getResultPersister() {
        return resultPersister;
    }

    /**
     * Run a single exercise using the reference agent.
     * This copies the reference implementation and runs tests to validate the exercise.
     *
     * @param language     Programming language
     * @param exerciseName Exercise name
     * @return Result of the exercise execution
     */
    public ExerciseResult runReferenceExercise(ReferenceAgent agent, String language, String exerciseName) {
        return runReferenceExercise(agent, language, exerciseName, null, null);
    }

    /**
     * Run a single exercise using the reference agent.
     *
     * @param agent        Agent instance
     * @param language     Programming language
     * @param exerciseName Exercise name
     * @param model        Model name (for results directory)
     * @param languages    Array of languages (for results directory)
     * @return Result of the exercise execution
     */
    public ExerciseResult runReferenceExercise(ReferenceAgent agent, String language, String exerciseName, String model, String[] languages) {
        // Treat empty strings as null for proper directory naming
        String effectiveModel = (model != null && !model.isEmpty()) ? model : null;
        getExerciseRunner().setRunParams(agent.getName(), effectiveModel, languages);
        return getExerciseRunner().runReferenceExercise(agent, model, language, exerciseName);
    }

    /**
     * Run all exercises for a language using the reference agent.
     *
     * @param languages comma separated list of programming language exercises to process
     * @return List of results for all exercises
     */
    public List<ExerciseResult> runAllReferenceExercises(ReferenceAgent agent, String languages, String agentName) {

        return runAllReferenceExercises(agent, languages, agentName, config.getModel(), null);
    }

    /**
     * Run all exercises for a language using the reference agent.
     *
     * @param languages comma separated list of programming language exercises to process
     * @param agentName Name of the agent (for result directory naming)
     * @param model     Model name (for result directory naming)
     * @param languagesArray Array of languages (for result directory naming)
     * @return List of results for all exercises
     */
    public List<ExerciseResult> runAllReferenceExercises(ReferenceAgent agent, String languages, String agentName, String model, String[] languagesArray) {
        List<ExerciseResult> result = new ArrayList<>();
        String[] split = languages.split(",");
        for (String language : split) {
            String trimmedLanguage = language.trim().toLowerCase();
            if (Languages.isSupported(trimmedLanguage)) {
                // Set run parameters for result directory computation
                // Treat empty strings as null for proper directory naming
                String effectiveModel = (model != null && !model.isEmpty()) ? model : null;
                getExerciseRunner().setRunParams(agentName, effectiveModel, languagesArray);
                List<ExerciseResult> languageResults = getExerciseRunner().runAllReferenceExercises(agent, model, language.trim(), agentName, languagesArray);
                result.addAll(languageResults);
            }
        }
        return result;
    }


    /**
     * Get configured parallelism value.
     */
    public int getParallelism() {
        return config.getParallelism();
    }

    public boolean isDockerAvailable() {
        return dockerClient.isAvailable();
    }

    /**
     * Saves results to the configured results directory.
     * Delegates to ResultPersister.
     *
     * @param results   List of exercise results to save
     * @param agentName Name of the agent used (for filename)
     * @param language  Language of exercises run
     * @return Path to the saved results file, or null if save failed
     */
    public Path saveResults(List<ExerciseResult> results, String agentName, String language) {
        return resultPersister.saveResults(results, agentName, new String[]{language});
    }

    /**
     * Saves results to the configured results directory.
     * Delegates to ResultPersister.
     *
     * @param results   List of exercise results to save
     * @param agentName Name of the agent used (for subdirectory naming)
     * @param model     Model name (for subdirectory naming)
     * @param languages Array of languages (for subdirectory naming)
     * @return Path to the saved results file, or null if save failed
     */
    public Path saveResults(List<ExerciseResult> results, String agentName, String model, String[] languages) {
        return resultPersister.saveResults(results, agentName, model, languages);
    }

    /**
     * Saves a single exercise result to the results directory.
     * Delegates to ResultPersister.
     *
     * @param result    Exercise result to save
     * @param agentName Name of the agent used
     * @return Path to the saved result file, or null if save failed
     */
    public Path saveResult(ExerciseResult result, String agentName, String language) {
        return resultPersister.saveResult(result, agentName, language);
    }

    /**
     * Saves a single exercise result with model information.
     * Delegates to ResultPersister.
     *
     * @param result    Exercise result to save
     * @param agentName Name of the agent used
     * @param model     Model name (for subdirectory naming)
     * @param language  Programming language
     * @param languages Array of languages (for subdirectory naming)
     * @return Path to the saved result file, or null if save failed
     */
    public Path saveResult(ExerciseResult result, String agentName, String model, String language, String[] languages) {
        return resultPersister.saveResult(result, agentName, model, language, languages);
    }

    /**
     * Checks if a result file already exists for the given exercise.
     * Delegates to ResultPersister.
     *
     * @param exerciseName Name of the exercise
     * @param agentName    Name of the agent used
     * @param language     Programming language
     * @return true if result file exists, false otherwise
     */
    public boolean resultFileExists(String exerciseName, String agentName, String language) {
        return resultPersister.resultFileExists(exerciseName, agentName, language);
    }

    /**
     * Print a summary of results.
     */
    public void printSummary(List<ExerciseResult> results) {
        long successful = results.stream().filter(ExerciseResult::isSuccess).count();
        long failed = results.size() - successful;
        double successRate = results.isEmpty() ? 0.0 : (successful * 100.0 / results.size());

        System.out.println("\n=== Benchmark Summary ===");
        System.out.println("Exercises run: " + results.size());
        System.out.println("Tests passed: " + successful + " (" + String.format("%.1f%%", successRate) + ")");
        System.out.println("Tests failed: " + failed);

        if (!failedResults(results).isEmpty()) {
            System.out.println("\nFailed exercises:");
            for (ExerciseResult r : failedResults(results)) {
                System.out.println("  - " + r.getExerciseName());
                printOutput(r.getOutput(), "    ");
            }
        }
    }

    private static void printOutput(String output, String indent) {
        if (output != null && !output.isEmpty()) {
            String[] lines = output.split("\n");
            for (String line : lines) {
                System.out.println(indent + line);
            }
        }
    }

    private List<ExerciseResult> failedResults(List<ExerciseResult> results) {
        return results.stream().filter(r -> !r.isSuccess()).toList();
    }

    /**
     * @deprecated Use {@link CliEntryPoint#main(String[])} instead.
     * This method is retained for backward compatibility only.
     */
    @Deprecated(since = "1.1", forRemoval = true)
    public static void main(String[] args) {
        CliEntryPoint.main(args);
    }

    public boolean resultFileSuccess(String name, String agentName, String model, String language, String[] languages) {
        return resultPersister.resultFileSuccess(name, agentName, model, language, languages);
    }

    // Package-private getter for config access in web mode
    Config getConfig() {
        return config;
    }

    // Package-private getter for dockerClient access in web mode
    DockerClient getDockerClient() {
        return dockerClient;
    }
}
