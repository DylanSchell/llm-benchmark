package io.schell.llm.benchmark.persistence;

import io.schell.llm.benchmark.config.OutputConfig;
import io.schell.llm.benchmark.exception.BenchmarkException;
import io.schell.llm.benchmark.exercise.ExerciseResult;
import io.schell.llm.benchmark.util.StringUtil;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.SerializationFeature;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.List;
import java.util.Map;

/**
 * Handles persistence of benchmark results to disk.
 * Extracted from BenchmarkRunner for better separation of concerns.
 */
public class ResultPersister {
    private static final Logger logger = LoggerFactory.getLogger(ResultPersister.class);

    private static final String RESULT_FILENAME_PATTERN = "result_%s_%s_%s.json";

    private final OutputConfig outputConfig;
    private final ObjectMapper mapper;

    public ResultPersister(OutputConfig outputConfig) {
        this.outputConfig = outputConfig;
        this.mapper = createObjectMapper();
    }

    private ObjectMapper createObjectMapper() {
        ObjectMapper mapper = new ObjectMapper();
        mapper.registerModule(new JavaTimeModule());
        mapper.enable(SerializationFeature.INDENT_OUTPUT);
        return mapper;
    }

    /**
     * Saves a single exercise result to the results directory.
     *
     * @param result    Exercise result to save
     * @param agentName Name of the agent used
     * @param language  Programming language
     * @return Path to the saved result file, or null if save failed
     */
    public Path saveResult(ExerciseResult result, String agentName, String language) {
        return saveResult(result, agentName, language, outputConfig.getResultsDir());
    }

    /**
     * Saves a single exercise result to the specified results directory.
     *
     * @param result       Exercise result to save
     * @param agentName    Name of the agent used
     * @param language     Programming language
     * @param resultsDir   Results directory path
     * @return Path to the saved result file
     * @throws BenchmarkException if save fails
     */
    public Path saveResult(ExerciseResult result, String agentName, String language, String resultsDir) {
        return saveResult(result, agentName, language, resultsDir, false);
    }

    /**
     * Saves a single exercise result to the specified results directory.
     *
     * @param result       Exercise result to save
     * @param agentName    Name of the agent used
     * @param language     Programming language
     * @param resultsDir   Results directory path
     * @param retry        If true and overwriting a successful result, preserve attempts count.
     *                     This is used for benchmark retries where the same exercise is re-run
     *                     against an already-successful result — attempts should not increment,
     *                     but timing/duration are updated to reflect the new run.
     * @return Path to the saved result file
     * @throws BenchmarkException if save fails
     */
    public Path saveResult(ExerciseResult result, String agentName, String language, String resultsDir, boolean retry) {
        Path resultsPath = Path.of(resultsDir);
        String filename = resultFilename(agentName, result.getLanguage(), result.getExerciseName());

        try {
            Files.createDirectories(resultsPath);
            Path resultFile = resultsPath.resolve(filename);

            // Determine whether to save, and what attempts count to use.
            // When retry=false, successful exercises are filtered out in scheduleBatch,
            // so an existing file can only be a failed previous run (now succeeding)
            // or a first-time run with no prior file.
            int newAttempts;
            boolean shouldSave = true;

            if (Files.exists(resultFile)) {
                var existingNode = mapper.readTree(resultFile.toFile());
                int existingAttempts = existingNode.has("attempts") ? Math.max(existingNode.get("attempts").asInt(), 1) : 1;
                boolean existingSuccess = existingNode.has("success") && existingNode.get("success").asBoolean();

                if (retry && existingSuccess && result.isSuccess()) {
                    // Retry overwriting a successful result: only save if faster
                    double existingDuration = existingNode.has("duration") ? existingNode.get("duration").asDouble() : Double.MAX_VALUE;
                    double newDuration = result.getDuration() != null 
                            ? result.getDuration().getSeconds() + result.getDuration().toMillis() / 1000.0 
                            : Double.MAX_VALUE;

                    if (newDuration >= existingDuration) {
                        // New run was not faster — skip saving, keep the better result
                        logger.info("Skipping retry save for {}/{}: new duration {}s >= existing {}s",
                                agentName, filename,
                                String.format("%.2f", newDuration),
                                String.format("%.2f", existingDuration));
                        shouldSave = false;
                    }
                    newAttempts = existingAttempts;
                } else {
                    // Overwriting a failure (or retry overwriting a failure): increment attempts
                    newAttempts = existingAttempts > 0 ? existingAttempts + 1 : result.getAttempts();
                }
            } else {
                // New file: start at 1
                newAttempts = result.getAttempts();
            }

            if (!shouldSave) {
                return resultFile;
            }

            // Create the result with computed attempts
            ExerciseResult toSave = ExerciseResult.builder()
                    .exerciseName(result.getExerciseName())
                    .language(result.getLanguage())
                    .success(result.isSuccess())
                    .exitCode(result.getExitCode())
                    .output(result.getOutput())
                    .duration(result.getDuration())
                    .startTime(result.getStartTime())
                    .endTime(result.getEndTime())
                    .errorMessage(result.getErrorMessage())
                    .trace(result.getTrace())
                    .model(result.getModel())
                    .attempts(newAttempts)
                    .build();

            Path traceFile = resultsPath.resolve(String.format("trace_%s_%s_%s.html", agentName, result.getLanguage(), result.getExerciseName()));

            mapper.writeValue(resultFile.toFile(), toSave);
            logger.info("Result saved to: {}", resultFile.toAbsolutePath());

            if (result.getTrace() != null && !result.getTrace().isEmpty()) {
                Files.writeString(traceFile, result.getTrace());
            }

            return resultFile;

        } catch (IOException e) {
            String errorMsg = String.format("Failed to save result to %s: %s", resultsPath, e.getMessage());
            logger.error(errorMsg, e);
            throw new BenchmarkException(errorMsg, e);
        }
    }

    /**
     * Saves a single exercise result with model information for directory naming.
     */
    public Path saveResult(ExerciseResult result, String agentName, String model, String language, String[] languages) {
        return saveResult(result, agentName, model, language, languages, false);
    }

    /**
     * Saves a single exercise result with model information for directory naming.
     *
     * @param retry If true and overwriting a successful result, preserve attempts count.
     */
    public Path saveResult(ExerciseResult result, String agentName, String model, String language, String[] languages, boolean retry) {
        String resultsDir = outputConfig.getResultsDir(agentName, StringUtil.toNonNull(model), languages);
        return saveResult(result, agentName, language, resultsDir, retry);
    }

    /**
     * Saves multiple exercise results to a summary file.
     *
     * @param results   List of exercise results to save
     * @param agentName Name of the agent used
     * @param languages Array of languages
     * @return Path to the saved results file, or null if save failed
     */
    public Path saveResults(List<ExerciseResult> results, String agentName, String[] languages) {
        String resultsDir = outputConfig.getResultsDir(agentName, null, languages);
        return saveResults(results, agentName, null, languages);
    }

    /**
     * Saves multiple exercise results to a summary file with model information.
     *
     * @param results   List of exercise results to save
     * @param agentName Name of the agent used
     * @param model     Model name (for subdirectory naming)
     * @param languages Array of languages (for subdirectory naming)
     * @return Path to the saved results file
     * @throws BenchmarkException if save fails
     */
    public Path saveResults(List<ExerciseResult> results, String agentName, String model, String[] languages) {
        String resultsDir = outputConfig.getResultsDir(agentName, model, languages);
        Path resultsPath = Path.of(resultsDir);

        try {
            Files.createDirectories(resultsPath);

            // Generate timestamped filename
            String timestamp = LocalDateTime.now().format(DateTimeFormatter.ofPattern("yyyyMMdd_HHmmss"));
            String langPart = StringUtil.join(languages, "-");
            String filename = String.format("results_%s_%s_%s.json", agentName, langPart.isEmpty() ? "unknown" : langPart, timestamp);
            Path resultFile = resultsPath.resolve(filename);

            // Generate summary
            long successful = results.stream().filter(ExerciseResult::isSuccess).count();
            double successRate = results.isEmpty() ? 0.0 : (successful * 100.0 / results.size());

            Map<String, Object> summary = Map.of(
                "timestamp", LocalDateTime.now().toString(),
                "agent", agentName,
                "language", String.join(",", languages),
                "total_exercises", results.size(),
                "successful", successful,
                "failed", results.size() - successful,
                "success_rate", String.format("%.1f%%", successRate),
                "results", results
            );

            mapper.writeValue(resultFile.toFile(), summary);
            logger.info("Results saved to: {}", resultFile.toAbsolutePath());

            // Save individual trace files
            for (ExerciseResult result : results) {
                if (result.getTrace() != null && !result.getTrace().isEmpty()) {
                    Path traceFile = resultsPath.resolve(String.format("trace_%s_%s_%s.html", agentName, langPart, timestamp));
                    Files.writeString(traceFile, result.getTrace());
                }
            }

            return resultFile;

        } catch (IOException e) {
            String errorMsg = String.format("Failed to save results to %s: %s", resultsPath, e.getMessage());
            logger.error(errorMsg, e);
            throw new BenchmarkException(errorMsg, e);
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
        String resultsDir = outputConfig.getResultsDir(agentName, null, new String[]{language});
        Path resultsPath = Path.of(resultsDir);
        return resultsPath.resolve(resultFilename(agentName, language, exerciseName)).toFile().exists();
    }

    /**
     * Checks if a result file exists and was successful.
     * Reads the JSON to verify the success field.
     *
     * @param exerciseName Name of the exercise
     * @param agentName    Name of the agent used
     * @param model        Model name (for subdirectory naming)
     * @param language     Programming language
     * @param languages    Array of languages (for subdirectory naming)
     * @return true if result file exists and success=true, false otherwise
     */
    public boolean resultFileSuccess(String exerciseName, String agentName, String model, String language, String[] languages) {
        String resultsDir = outputConfig.getResultsDir(agentName, StringUtil.toNonNull(model), languages);
        Path resultsPath = Path.of(resultsDir);
        Path resultPath = resultsPath.resolve(resultFilename(agentName, language, exerciseName));

        if (!Files.exists(resultPath)) {
            return false;
        }

        try {
            var resultNode = mapper.readTree(resultPath.toFile());
            return resultNode.has("success") && resultNode.get("success").asBoolean();
        } catch (IOException e) {
            logger.warn("Failed to read result file {}: {}", resultPath, e.getMessage());
            return false;
        }
    }

    /**
     * Generates the standard result filename for an exercise.
     */
    private static String resultFilename(String agentName, String language, String exerciseName) {
        return String.format(RESULT_FILENAME_PATTERN, agentName, language, exerciseName);
    }

    /**
     * Reads the attempts count from an existing result file.
     * Returns 0 if the file doesn't exist or can't be read.
     */
    private int readExistingAttempts(Path resultFile) {
        if (!Files.exists(resultFile)) {
            return 0;
        }
        try {
            var node = mapper.readTree(resultFile.toFile());
            if (node.has("attempts")) {
                return node.get("attempts").asInt();
            }
            return 0;
        } catch (IOException e) {
            logger.warn("Failed to read attempts from existing result file {}: {}", resultFile, e.getMessage());
            return 0;
        }
    }
}
