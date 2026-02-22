package com.benchmark;

import com.benchmark.agent.ClaudeAgent;
import com.benchmark.agent.ReferenceAgent;
import com.benchmark.config.Config;
import com.benchmark.config.ConfigLoader;
import com.benchmark.docker.DockerClient;
import com.benchmark.exercise.ExerciseResult;
import com.benchmark.exercise.ExerciseRunner;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.SerializationFeature;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

/**
 * Main entry point for the benchmark runner.
 * Provides convenient methods for running exercises.
 */
public class BenchmarkRunner {
    private static final Logger logger = LoggerFactory.getLogger(BenchmarkRunner.class);

    private final Config config;
    private final DockerClient dockerClient;
    private ExerciseRunner exerciseRunner;
    private static final Set<String> supportedLanguages = Set.of("java", "go", "javascript", "python", "rust", "cpp");

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
    private ExerciseRunner getExerciseRunner() {
        if (exerciseRunner == null) {
            return new ExerciseRunner(config, dockerClient, this);
        }
        return exerciseRunner;
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
        return getExerciseRunner().runReferenceExercise(agent, language, exerciseName);
    }

    /**
     * Run all exercises for a language using the reference agent.
     *
     * @param languages comma separated list of programming language exercises to process
     * @return List of results for all exercises
     */
    public List<ExerciseResult> runAllReferenceExercises(ReferenceAgent agent, String languages, String agentName) {
        List<ExerciseResult> result = new ArrayList<>();
        String[] split = languages.split(",");
        for (String language : split) {
            String trimmedLanguage = language.trim().toLowerCase();
            if (supportedLanguages.contains(trimmedLanguage)) {
                List<ExerciseResult> languageResults = getExerciseRunner().runAllReferenceExercises(agent, language.trim(), agentName);
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
     *
     * @param results   List of exercise results to save
     * @param agentName Name of the agent used (for filename)
     * @param language  Language of exercises run
     * @return Path to the saved results file, or null if save failed
     */
    public Path saveResults(List<ExerciseResult> results, String agentName, String language) {
        String resultsDir = config.getOutput().getResultsDir();
        Path resultsPath = Paths.get(resultsDir);
        try {
            // Create results directory if it doesn't exist
            Files.createDirectories(resultsPath);

            // Generate timestamped filename
            String timestamp = LocalDateTime.now().format(DateTimeFormatter.ofPattern("yyyyMMdd_HHmmss"));
            String filename = String.format("results_%s_%s_%s.json", agentName, language, timestamp);
            Path resultFile = resultsPath.resolve(filename);
            // Create summary object
            long successful = results.stream().filter(ExerciseResult::isSuccess).count();
            double successRate = results.isEmpty() ? 0.0 : (successful * 100.0 / results.size());

            var summary = new java.util.HashMap<String, Object>();
            summary.put("timestamp", LocalDateTime.now().toString());
            summary.put("agent", agentName);
            summary.put("language", language);
            summary.put("total_exercises", results.size());
            summary.put("successful", successful);
            summary.put("failed", results.size() - successful);
            summary.put("success_rate", String.format("%.1f%%", successRate));
            summary.put("results", results);

            // Write JSON file
            ObjectMapper mapper = new ObjectMapper();
            mapper.registerModule(new JavaTimeModule());
            mapper.enable(SerializationFeature.INDENT_OUTPUT);
            mapper.writeValue(resultFile.toFile(), summary);

            System.out.println("\nResults saved to: " + resultFile.toAbsolutePath());
            for (ExerciseResult result : results) {
                String traceFileName = String.format("trace_%s_%s_%s.html", agentName, language, timestamp);
                Path traceFile = resultsPath.resolve(traceFileName);
                if (result.getTrace() != null && !result.getTrace().isEmpty()) {
                    Files.writeString(traceFile, result.getTrace());
                }
            }
            return resultFile;

        } catch (IOException e) {
            logger.error("Failed to save results to {}: {}", resultsPath, e.getMessage());
            return null;
        }

    }

    /**
     * Checks if a result file already exists for the given exercise.
     *
     * @param exerciseName Name of the exercise
     * @param agentName    Name of the agent used
     * @param language     Programming language
     * @return true if result file exists, false otherwise
     */
    public boolean resultFileExists(String exerciseName, String agentName, String language) {
        String resultsDir = config.getOutput().getResultsDir();
        Path resultsPath = Paths.get(resultsDir);
        String filename = String.format("result_%s_%s_%s.json", agentName, language, exerciseName);
        return resultsPath.resolve(filename).toFile().exists();
    }

    /**
     * Saves a single exercise result to the results directory.
     *
     * @param result    Exercise result to save
     * @param agentName Name of the agent used
     * @return Path to the saved result file, or null if save failed
     */
    public Path saveResult(ExerciseResult result, String agentName) {
        String resultsDir = config.getOutput().getResultsDir();
        Path resultsPath = Paths.get(resultsDir);

        try {
            // Create results directory if it doesn't exist
            Files.createDirectories(resultsPath);
            // Generate filename with exercise name
            String filename = String.format("result_%s_%s_%s.json", agentName, result.getLanguage(), result.getExerciseName());
            Path resultFile = resultsPath.resolve(filename);
            Path traceFile = resultsPath.resolve(String.format("trace_%s_%s_%s.html", agentName, result.getLanguage(), result.getExerciseName()));
            ObjectMapper mapper = new ObjectMapper();
            mapper.registerModule(new JavaTimeModule());
            mapper.enable(SerializationFeature.INDENT_OUTPUT);
            mapper.writeValue(resultFile.toFile(), result);
            System.out.println("\nResult saved to: " + resultFile.toAbsolutePath());
            if (result.getTrace() != null && !result.getTrace().isEmpty()) {
                Files.writeString(traceFile, result.getTrace());
            }
            return resultFile;

        } catch (IOException e) {
            logger.error("Failed to save result to {}: {}", resultsPath, e.getMessage());
            return null;
        }
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

    public static void main(String[] args) {
        String configFile = "config.yaml";
        boolean webMode = false;
        int webPort = 8080;

        try {
            for (int i = 0; i < args.length; i++) {
                if (args[i].equals("--config") && i + 1 < args.length) {
                    configFile = args[++i];
                } else if (args[i].equals("--web")) {
                    webMode = true;
                    // Check if port is specified as next argument (must be a number)
                    if (i + 1 < args.length) {
                        try {
                            int potentialPort = Integer.parseInt(args[++i]);
                            if (potentialPort > 0 && potentialPort < 65536) {
                                webPort = potentialPort;
                            } else {
                                i--; // Not a valid port, step back
                            }
                        } catch (NumberFormatException e) {
                            i--; // Not a number, step back
                        }
                    }
                } else if (args[i].equals("--port") && i + 1 < args.length) {
                    webPort = Integer.parseInt(args[++i]);
                }
            }

            Path configPath = Paths.get(configFile);
            if (!configPath.toFile().exists()) {
                System.err.printf("%s not found in current directory", configFile);
                System.exit(1);
            }

            BenchmarkRunner runner = new BenchmarkRunner(configPath);

            // Check for web mode - only when --web flag is explicitly passed
            if (webMode) {
                // Start web interface
                System.out.println("Starting web interface on port " + webPort + "...");
                startWebMode(args, configPath, runner, webPort);
                return;
            }

            if (!runner.isDockerAvailable()) {
                System.err.println("Docker is not available. Please ensure Docker is running.");
                System.exit(1);
            }

            String language = "java";
            String exerciseName = null;
            String agentName = "reference";

            // Parse command line arguments
            for (int i = 0; i < args.length; i++) {
                if (args[i].equals("--language") && i + 1 < args.length) {
                    language = args[++i];
                } else if (args[i].equals("--exercise") && i + 1 < args.length) {
                    exerciseName = args[++i];
                } else if (args[i].equals("--agent") && i + 1 < args.length) {
                    agentName = args[++i];
                }
            }

            ReferenceAgent agent = null;
            if (agentName.equals("reference")) {
                agent = new ReferenceAgent(runner.dockerClient);
            } else if (agentName.equals("claude")) {
                agent = new ClaudeAgent(runner.dockerClient);
            } else {
                System.err.println("agent must be either 'reference' or 'claude'");
                System.exit(1);
            }

            if (exerciseName != null) {
                // Run single exercise
                ExerciseResult result;

                System.out.println("Running with " + agentName + " agent ...");
                result = runner.runReferenceExercise(agent, language, exerciseName);
                System.out.println("\n=== Exercise Result ===");
                System.out.println("Exercise: " + result.getExerciseName());
                System.out.println("Language: " + result.getLanguage());
                System.out.println("Success: " + result.isSuccess());
                System.out.println("Duration: " + result.getDuration());
                if (!result.isSuccess()) {
                    System.out.println("\nOutput:");
                    printOutput(result.getOutput(), "  ");
                }

                // Save result
                runner.saveResult(result, agentName);
                System.exit(result.isSuccess() ? 0 : 1);
            } else {
                // Run all exercises
                System.out.println("Running all exercises with " + agentName + " agent ...");

                List<ExerciseResult> results = runner.runAllReferenceExercises(agent, language, agentName);
                runner.printSummary(results);

                // Save results
                runner.saveResults(results, agentName, language);

                long failed = results.stream().filter(r -> !r.isSuccess()).count();
                System.exit(failed > 0 ? 1 : 0);
            }
        } catch (Exception e) {
            logger.error("Failed to run benchmark: {}", e.getMessage(), e);
            System.exit(1);
        }
    }

    public boolean resultFileSuccess(String name, String agentName, String language) {
        String resultsDir = config.getOutput().getResultsDir();
        Path resultsPath = Paths.get(resultsDir);
        String filename = String.format("result_%s_%s_%s.json", agentName, language, name);
        var p = resultsPath.resolve(filename);
        if (Files.exists(p)) {
            ObjectMapper mapper = new ObjectMapper();
            mapper.registerModule(new JavaTimeModule());
            try {
                var er = mapper.readTree(p.toFile());
                return er.has("success") && er.get("success").asBoolean();
            } catch (IOException e) {
                // ignore, and assume it was not succesful
            }
        }
        return false;
    }

    /**
     * Starts the web interface mode.
     * This method is called when --web flag is passed or when no arguments are provided.
     * All beans are now managed by Spring - no manual passing needed.
     */
    private static void startWebMode(String[] args, Path configPath, BenchmarkRunner runner, int port) {
        try {
            // Import and start Spring Boot application
            Class<?> webRunnerClass = Class.forName("com.benchmark.web.WebBenchmarkRunner");
            var method = webRunnerClass.getDeclaredMethod("runWebMode", String[].class);
            method.invoke(null, (Object) args);
        } catch (Exception e) {
            System.err.println("Failed to start web interface: " + e.getMessage());
            e.printStackTrace();
            System.exit(1);
        }
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
