package com.benchmark.persistence;

import com.benchmark.config.OutputConfig;
import com.benchmark.exception.BenchmarkException;
import com.benchmark.exercise.ExerciseResult;
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
        Path resultsPath = Path.of(resultsDir);

        try {
            Files.createDirectories(resultsPath);
            String filename = String.format("result_%s_%s_%s.json", agentName, result.getLanguage(), result.getExerciseName());
            Path resultFile = resultsPath.resolve(filename);
            Path traceFile = resultsPath.resolve(String.format("trace_%s_%s_%s.html", agentName, result.getLanguage(), result.getExerciseName()));

            mapper.writeValue(resultFile.toFile(), result);
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
     *
     * @param result    Exercise result to save
     * @param agentName Name of the agent used
     * @param model     Model name (for subdirectory naming)
     * @param language  Programming language
     * @param languages Array of languages (for subdirectory naming)
     * @return Path to the saved result file, or null if save failed
     */
    public Path saveResult(ExerciseResult result, String agentName, String model, String language, String[] languages) {
        if (languages == null) {
            languages = new String[]{language};
        }
        String resultsDir = outputConfig.getResultsDir(agentName, model, languages);
        return saveResult(result, agentName, language, resultsDir);
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
            String langPart = languages != null && languages.length > 0 ? String.join("-", languages) : "unknown";
            String filename = String.format("results_%s_%s_%s.json", agentName, langPart, timestamp);
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
        String filename = String.format("result_%s_%s_%s.json", agentName, language, exerciseName);
        return resultsPath.resolve(filename).toFile().exists();
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
        String resultsDir = outputConfig.getResultsDir(agentName, model, languages);
        Path resultsPath = Path.of(resultsDir);
        String filename = String.format("result_%s_%s_%s.json", agentName, language, exerciseName);
        Path resultPath = resultsPath.resolve(filename);

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
}
