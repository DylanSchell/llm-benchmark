package com.benchmark.exercise;

import com.benchmark.BenchmarkRunner;
import com.benchmark.agent.ReferenceAgent;
import com.benchmark.config.Config;
import com.benchmark.docker.DockerClient;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Stream;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

/**
 * Runner for executing benchmark exercises inside Docker containers.
 * Uses Claude Code CLI to solve the exercises.
 */
public class ExerciseRunner {
    private static final Logger logger = LoggerFactory.getLogger(ExerciseRunner.class);
    private static final ObjectMapper objectMapper = new ObjectMapper();

    private final Config config;
    private final DockerClient dockerClient;
    private final Path benchmarkPath;
    private final BenchmarkRunner benchmarkRunner;

    public ExerciseRunner(Config config, DockerClient dockerClient, BenchmarkRunner benchmarkRunner) {
        this.config = config;
        this.dockerClient = dockerClient;
        this.benchmarkPath = config.getBenchmarkPath();
        this.benchmarkRunner = benchmarkRunner;
    }

    /**
     * Runs a single exercise using the reference agent (copies reference implementation and runs tests).
     *
     * @param language     Programming language
     * @param exerciseName Name of the exercise
     * @return ExerciseResult with the outcome
     */
    public ExerciseResult runReferenceExercise(ReferenceAgent agent, String language, String exerciseName) {
        logger.info("Running reference agent for exercise: {} for language: {}", exerciseName, language);

        Exercise exercise = findExercise(language, exerciseName);
        if (exercise == null) {
            logger.error("Exercise not found: {}/{}", language, exerciseName);
            return ExerciseResult.builder()
                    .exerciseName(exerciseName)
                    .language(language)
                    .success(false)
                    .errorMessage("Exercise not found")
                    .build();
        }

        Path exerciseHostDir = findExerciseHostDir(language, exerciseName);
        if (exerciseHostDir == null || !Files.exists(exerciseHostDir)) {
            logger.error("Exercise directory not found: {}", exerciseHostDir);
            return ExerciseResult.builder()
                    .exerciseName(exerciseName)
                    .language(language)
                    .success(false)
                    .errorMessage("Exercise directory not found: " + exerciseHostDir)
                    .build();
        }
        Path resultDir = getResultsDir();
        return runReferenceAgent(agent, exercise, exerciseHostDir, resultDir);
    }

    /**
     * Runs all exercises for the specified language using the reference agent.
     *
     * @param agent     Agent to use for running exercises
     * @param language  Programming language
     * @param agentName Name of the agent (for result file naming)
     * @return List of ExerciseResult for all exercises
     */
    public List<ExerciseResult> runAllReferenceExercises(ReferenceAgent agent, String language, String agentName) {
        logger.info("Running all reference exercises for language: {}", language);

        List<Exercise> exercises = findAllExercises(language);
        logger.info("Found {} exercises for language: {}", exercises.size(), language);

        List<ExerciseResult> results = new ArrayList<>();
        int total = exercises.size();
        int counter = 0;

        // Determine parallelism from runner configuration
        int parallelism = benchmarkRunner.getParallelism();
        ExecutorService executor = Executors.newFixedThreadPool(parallelism);
        List<Callable<ExerciseResult>> tasks = new ArrayList<>();

        for (Exercise exercise : exercises) {
            logger.info("=============================================================================");
            logger.info("Running {} exercise {} ({}/{})", language, exercise.getName(), ++counter, total);
            // Skip if result already exists
            if (benchmarkRunner.resultFileSuccess(exercise.getName(), agentName, language)) {
                continue;
            }
            // Verify exercise directory
            Path exerciseHostDir = findExerciseHostDir(language, exercise.getName());
            if (exerciseHostDir == null || !Files.exists(exerciseHostDir)) {
                logger.warn("Exercise directory not found, skipping: {}/{}", language, exercise.getName());
                continue;
            }
            // Create task for parallel execution
            tasks.add(() -> {
                logger.info("Running reference for exercise {}/{}", language, exercise.getName());
                ExerciseResult result = runReferenceAgent(agent, exercise, exerciseHostDir, getResultsDir());
                // Save result immediately after completion
                benchmarkRunner.saveResult(result, agentName);
                return result;
            });
        }

        try {
            List<Future<ExerciseResult>> futures = executor.invokeAll(tasks);
            for (Future<ExerciseResult> future : futures) {
                try {
                    ExerciseResult result = future.get();
                    results.add(result);
                } catch (Exception e) {
                    logger.error("Error executing benchmark task: {}", e.getMessage(), e);
                }
            }
        } catch (InterruptedException e) {
            logger.error("Benchmark execution interrupted: {}", e.getMessage(), e);
            Thread.currentThread().interrupt();
        } finally {
            executor.shutdown();
        }

        return results;
    }

    /**
     * Finds a specific exercise by language and name.
     */
    private Exercise findExercise(String language, String exerciseName) {
        Path exerciseDir = benchmarkPath
                .resolve("exercises")
                .resolve("practice")
                .resolve(exerciseName);

        if (!Files.exists(exerciseDir)) {
            // Try with language-specific structure
            exerciseDir = benchmarkPath
                    .resolve(language)
                    .resolve("exercises")
                    .resolve("practice")
                    .resolve(exerciseName);
        }

        if (!Files.exists(exerciseDir)) {
            return null;
        }

        return new Exercise(
                exerciseName,
                language,
                exerciseDir,
                parseMetadata(exerciseDir)
        );
    }

    /**
     * Finds all exercises for a given language.
     */
    private List<Exercise> findAllExercises(String language) {
        Path exercisesPath = benchmarkPath
                .resolve(language)
                .resolve("exercises")
                .resolve("practice");

        if (!Files.exists(exercisesPath)) {
            logger.warn("Exercises path not found: {}", exercisesPath);
            return List.of();
        }

        List<Exercise> exercises = new ArrayList<>();

        try (Stream<Path> paths = Files.walk(exercisesPath)) {
            paths.filter(Files::isDirectory)
                    .filter(this::isExerciseDirectory)
                    .forEach(exerciseDir -> {
                        String exerciseName = exerciseDir.getFileName().toString();
                        exercises.add(new Exercise(
                                exerciseName,
                                language,
                                exerciseDir,
                                parseMetadata(exerciseDir)
                        ));
                    });
        } catch (IOException e) {
            logger.error("Failed to list exercises: {}", e.getMessage(), e);
        }
        exercises.sort((o1, o2) -> {
            if (o1.getName().equals("pov")) return 1;
            if (o2.getName().equals("pov")) return -1;
            return o1.getName().compareTo(o2.getName());
        });
        for (Exercise exercise : exercises) {
            logger.info("Found exercise {}/{}", language, exercise.getName());
        }
        return exercises;
    }

    /**
     * Checks if a directory is an exercise directory (contains src/main/java or similar).
     */
    private boolean isExerciseDirectory(Path dir) {
        return Files.exists(dir.resolve("build.gradle")) || Files.exists(dir.resolve("go.mod"));
    }

    /**
     * Parses the metadata from .meta/config.json for an exercise.
     */
    private ExerciseMetadata parseMetadata(Path exerciseDir) {
        Path metaConfigPath = exerciseDir.resolve(".meta").resolve("config.json");
        if (!Files.exists(metaConfigPath)) {
            logger.debug("No metadata file found at {}", metaConfigPath);
            return null;
        }
        try {
            return objectMapper.readValue(metaConfigPath.toFile(), ExerciseMetadata.class);
        } catch (IOException e) {
            logger.warn("Failed to parse metadata at {}: {}", metaConfigPath, e.getMessage());
            return null;
        }
    }

    /**
     * Runs the reference agent for an exercise.
     */
    private ExerciseResult runReferenceAgent(ReferenceAgent agent, Exercise exercise, Path exerciseHostDir, Path resultDir) {
        try {
            ReferenceAgent.ReferenceResult refResult = agent.runReferenceSolution(exercise, exerciseHostDir, resultDir);

            return ExerciseResult.builder()
                    .exerciseName(refResult.exerciseName())
                    .language(refResult.language())
                    .success(refResult.success())
                    .exitCode(refResult.exitCode())
                    .output(refResult.output())
                    .duration(refResult.duration())
                    .startTime(refResult.startTime())
                    .endTime(refResult.endTime())
                    .trace(refResult.trace())
                    .errorMessage(refResult.success() ? null : refResult.errorMessage())
                    .build();

        } catch (Exception e) {
            logger.error("Failed to run reference agent for exercise {}: {}", exercise.getName(), e.getMessage());

            return ExerciseResult.builder()
                    .exerciseName(exercise.getName())
                    .language(exercise.getLanguage())
                    .success(false)
                    .errorMessage(e.getMessage())
                    .build();
        }
    }

    /**
     * Finds the host directory for an exercise.
     */
    private Path findExerciseHostDir(String language, String exerciseName) {
        Path exerciseDir = benchmarkPath
                .resolve(language)
                .resolve("exercises")
                .resolve("practice")
                .resolve(exerciseName);

        if (Files.exists(exerciseDir)) {
            return exerciseDir;
        }

        return null;
    }

    private Path getResultsDir() {
        String resultsDir = config.getOutput().getResultsDir();
        Path resultsPath = Paths.get(resultsDir);
        if (!Files.exists(resultsPath)) {
            try {
                Files.createDirectories(resultsPath);
            } catch (IOException e) {
                logger.error("Failed to create directory: {}", e.getMessage(), e);
            }
        }
        return resultsPath;
    }
}
