package com.benchmark.web.service;

import com.benchmark.BenchmarkResultAnalyzer;
import com.benchmark.config.Config;
import com.benchmark.exercise.ExerciseResult;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.ConcurrentHashMap;
import java.util.stream.Stream;

/**
 * Service for reading and managing benchmark results.
 * Caches all results in memory on startup for fast access.
 */
@Service
public class ResultService {
    private static final Logger logger = LoggerFactory.getLogger(ResultService.class);

    private final Config config;
    private final ObjectMapper objectMapper;

    // In-memory cache of all cachedResult files (keyed by filename)
    private final Map<String, CachedResult> cachedResults = new ConcurrentHashMap<>();

    // Cache of models (subdirectory names)
    private final Set<String> cachedModels = ConcurrentHashMap.newKeySet();

    /**
     * Cached result data for fast in-memory access.
     */
    private static class CachedResult {
        String filename;
        String path;
        String timestamp;
        String agent;
        String language;
        String model;
        int totalExercises;
        int successful;
        int failed;
        String successRate;
        List<Map<String, Object>> results;
        String tracePath; // Path to separate trace HTML file if it exists

        // Computed fields for statistics
        Map<String, Integer> exerciseCountByLanguage = new HashMap<>();
        Map<String, Integer> successCountByLanguage = new HashMap<>();
        Map<String, Double> durationByLanguage = new HashMap<>();
    }

    public ResultService(Config config) {
        this.config = config;
        this.objectMapper = new ObjectMapper();
        this.objectMapper.registerModule(new JavaTimeModule());
        logger.info("ResultService initialized with config: {}", config != null ? "present" : "null");
        if (config != null && config.getOutput() != null) {
            logger.info("Results directory: {}", config.getOutput().getResultsDir());
        }
    }

    /**
     * Loads all results into memory on startup.
     */
    @PostConstruct
    public void init() {
        loadAllResults();
    }

    /**
     * Loads all cachedResult files into memory.
     */
    public synchronized void loadAllResults() {
        logger.info("Loading all results into cache...");
        cachedResults.clear();
        cachedModels.clear();

        Path configuredResultsDir = Paths.get(config.getOutput().getResultsDir());

        int count = 0;
        try (Stream<Path> paths = Files.walk(configuredResultsDir)) {
            List<Path> resultFiles = paths.filter(Files::isRegularFile)
                    .filter(p -> p.toString().endsWith(".json"))
                    .filter(p -> {
                        String filename = p.getFileName().toString();
                        // Load both results_* (batch) and result_* (individual exercise) files
                        return filename.startsWith("results_") || filename.startsWith("result_");
                    })
                    .toList();

            for (Path p : resultFiles) {
                try {
                    CachedResult cached = loadCachedResult(p);
                    if (cached != null) {
                        cachedResults.put(p.getFileName().toString(), cached);
                        cachedModels.add(cached.model);
                        count++;
                    }
                } catch (IOException e) {
                    logger.warn("Failed to load cachedResult file {}: {}", p, e.getMessage());
                }
            }
        } catch (IOException e) {
            logger.error("Failed to list results: {}", e.getMessage(), e);
        }

        logger.info("Loaded {} cachedResult files into cache", count);
    }

    /**
     * Loads a single cachedResult file into a CachedResult object.
     * Handles both batch result files (results_*.json with results array)
     * and individual exercise result files (result_*.json serialized ExerciseResult).
     */
    private CachedResult loadCachedResult(Path resultFile) throws IOException {
        JsonNode node = objectMapper.readTree(resultFile.toFile());

        // Check if this is a batch result file (has "results" array) or individual exercise result
        boolean isBatchResult = node.has("results") && node.get("results").isArray();
        boolean isIndividualResult = node.has("exerciseName") && node.has("language");

        if (!isBatchResult && !isIndividualResult) {
            return null;
        }

        CachedResult cached = new CachedResult();
        cached.filename = resultFile.getFileName().toString();
        cached.path = resultFile.toString();

        // Extract model from file path or from embedded field
        if (node.has("model")) {
            cached.model = node.get("model").asText();
        } else {
            // Derive from directory structure
            cached.model = resultFile.getParent().getFileName().toString();
        }

        cached.timestamp = node.has("timestamp") ? node.get("timestamp").asText() : null;

        // Extract agent from embedded field or derive from filename
        if (node.has("agent")) {
            cached.agent = node.get("agent").asText();
        } else if (isIndividualResult) {
            // Derive agent from filename pattern: result_<agent>_<language>_<exercise>.json
            String filename = resultFile.getFileName().toString();
            if (filename.startsWith("result_")) {
                // Extract agent from filename
                String[] parts = filename.split("_");
                if (parts.length >= 2) {
                    cached.agent = parts[1]; // Second part after "result"
                } else {
                    cached.agent = "unknown";
                }
            } else {
                cached.agent = "unknown";
            }
        } else {
            cached.agent = "unknown";
        }

        // For individual results, extract language and exercise from embedded fields
        if (isIndividualResult) {
            cached.language = node.has("language") ? node.get("language").asText() : "unknown";
            String exercise = node.has("exerciseName") ? node.get("exerciseName").asText() : "unknown";

            // Create a single-item results list for individual result files
            Map<String, Object> singleResult = new HashMap<>();
            singleResult.put("language", cached.language);
            singleResult.put("exercise", exercise);
            singleResult.put("success", node.has("success") ? node.get("success").asBoolean() : false);
            singleResult.put("duration", node.has("duration") ? node.get("duration").toString() : "0");
            singleResult.put("output", node.has("output") ? node.get("output").asText() : "");
            singleResult.put("trace", node.has("trace") ? node.get("trace").asText() : "");

            cached.results = List.of(singleResult);
            cached.totalExercises = 1;
            cached.successful = singleResult.get("success") == Boolean.TRUE ? 1 : 0;
            cached.failed = singleResult.get("success") == Boolean.FALSE ? 1 : 0;
            cached.successRate = cached.successful > 0 ? "100.0%" : "0.0%";

            // Compute per-language statistics for this single result
            String lang = cached.language != null ? cached.language : "unknown";
            cached.exerciseCountByLanguage.put(lang, 1);

            if (singleResult.get("success") == Boolean.TRUE) {
                cached.successCountByLanguage.put(lang, 1);
            }

            // Parse duration if available
            try {
                String durStr = singleResult.get("duration").toString();
                if (durStr != null && !durStr.isEmpty() && !durStr.equals("null")) {
                    // Try to parse duration string (e.g., "PT1.5S" or "100ms")
                    double dur = 0.0;
                    if (durStr.endsWith("ms")) {
                        dur = Double.parseDouble(durStr.substring(0, durStr.length() - 2)) / 1000.0;
                    } else if (durStr.endsWith("s")) {
                        dur = Double.parseDouble(durStr.substring(0, durStr.length() - 1));
                    } else {
                        try {
                            dur = Double.parseDouble(durStr);
                        } catch (NumberFormatException ignored) {}
                    }
                    cached.durationByLanguage.put(lang, dur);
                }
            } catch (Exception e) {
                logger.debug("Failed to parse duration: {}", e.getMessage());
            }
        }

        if (isBatchResult) {
            // Parse batch results array
            cached.language = node.has("language") ? node.get("language").asText() : "unknown";
            cached.totalExercises = node.has("total_exercises") ? node.get("total_exercises").asInt() : 0;
            cached.successful = node.has("successful") ? node.get("successful").asInt() : 0;
            cached.failed = node.has("failed") ? node.get("failed").asInt() : 0;
            cached.successRate = node.has("success_rate") ? node.get("success_rate").asText() : "0.0%";

            // Parse results array
            if (node.has("results") && node.get("results").isArray()) {
                cached.results = objectMapper.readValue(node.get("results").toString(), List.class);

                // Compute per-language statistics
                for (Map<String, Object> result : cached.results) {
                    String lang = (String) result.get("language");
                    if (lang == null) lang = "unknown";

                    Boolean success = (Boolean) result.get("success");
                    Number duration = (Number) result.get("duration");
                    double dur = duration != null ? duration.doubleValue() : 0.0;

                    cached.exerciseCountByLanguage.putIfAbsent(lang, 0);
                    cached.exerciseCountByLanguage.put(lang, cached.exerciseCountByLanguage.get(lang) + 1);

                    cached.durationByLanguage.putIfAbsent(lang, 0.0);
                    cached.durationByLanguage.put(lang, cached.durationByLanguage.get(lang) + dur);

                    if (success != null && success) {
                        cached.successCountByLanguage.putIfAbsent(lang, 0);
                        cached.successCountByLanguage.put(lang, cached.successCountByLanguage.get(lang) + 1);
                    }
                }
            }
        }

        // Check for separate trace HTML file (trace_<agent>_<language>_<exercise>.html)
        // For individual results, look for a corresponding .html trace file
        if (isIndividualResult) {
            String filename = resultFile.getFileName().toString();
            if (filename.startsWith("result_")) {
                // Replace result_ with trace_ and .json with .html
                String traceFilename = filename.replace("result_", "trace_").replace(".json", ".html");
                Path traceFilePath = resultFile.getParent().resolve(traceFilename);
                if (Files.exists(traceFilePath)) {
                    cached.tracePath = traceFilePath.toString();
                }
            }
        }

        return cached;
    }

    /**
     * Lists all cachedResult files with optional filtering.
     */
    public List<Map<String, Object>> listResults(String language, String agent, String model) {
        List<Map<String, Object>> results = new ArrayList<>();

        for (CachedResult cached : cachedResults.values()) {
            boolean matchesLanguage = language == null || language.isEmpty() ||
                    (cached.language != null && cached.language.equals(language));
            boolean matchesAgent = agent == null || agent.isEmpty() ||
                    (cached.agent != null && cached.agent.contains(agent));
            boolean matchesModel = model == null || model.isEmpty() ||
                    (cached.model != null && cached.model.equals(model));

            if (matchesLanguage && matchesAgent && matchesModel) {
                results.add(toMetadataMap(cached));
            }
        }

        // Sort by timestamp descending
        results.sort((a, b) -> {
            String tsA = (String) a.get("timestamp");
            String tsB = (String) b.get("timestamp");
            if (tsA == null && tsB == null) return 0;
            if (tsA == null) return 1;
            if (tsB == null) return -1;
            return tsB.compareTo(tsA);
        });

        return results;
    }

    /**
     * Gets list of all model names (subdirectory names).
     */
    public List<String> getModels() {
        return new ArrayList<>(cachedModels.stream().sorted().toList());
    }

    /**
     * Lists individual result files (result_*.json) filtered by agent, language, and model.
     * These are the detailed runs for each exercise.
     */
    public List<Map<String, Object>> listIndividualResults(String language, String agent, String model) {
        List<Map<String, Object>> results = new ArrayList<>();
        Path configuredResultsDir = Paths.get(config.getOutput().getResultsDir());

        try (Stream<Path> paths = Files.walk(configuredResultsDir)) {
            List<Path> individualResultFiles = paths.filter(Files::isRegularFile)
                    .filter(p -> p.toString().endsWith(".json"))
                    .filter(p -> p.getFileName().toString().startsWith("result_"))
                    .toList();

            for (Path p : individualResultFiles) {
                try {
                    JsonNode node = objectMapper.readTree(p.toFile());

                    // Extract fields from the individual result file
                    String fileAgent = node.has("agent") ? node.get("agent").asText() :
                            extractAgentFromFilename(p.getFileName().toString());
                    String fileLanguage = node.has("language") ? node.get("language").asText() : "unknown";
                    String fileModel = node.has("model") ? node.get("model").asText() : p.getParent().getFileName().toString();
                    String exerciseName = node.has("exerciseName") ? node.get("exerciseName").asText() : "unknown";

                    // Apply filters
                    boolean matchesLanguage = language == null || language.isEmpty() || fileLanguage.equals(language);
                    boolean matchesAgent = agent == null || agent.isEmpty() || fileAgent.equals(agent);
                    boolean matchesModel = model == null || model.isEmpty() || fileModel.equals(model);

                    if (matchesLanguage && matchesAgent && matchesModel) {
                        Map<String, Object> individualResult = new HashMap<>();
                        individualResult.put("filename", p.getFileName().toString());
                        individualResult.put("path", p.toString());
                        individualResult.put("agent", fileAgent);
                        individualResult.put("language", fileLanguage);
                        individualResult.put("model", fileModel);
                        individualResult.put("exercise", exerciseName);
                        individualResult.put("success", node.has("success") ? node.get("success").asBoolean() : false);
                        individualResult.put("timestamp", node.has("timestamp") ? node.get("timestamp").asText() : null);

                        // Check for separate trace HTML file
                        String traceFilename = p.getFileName().toString().replace("result_", "trace_").replace(".json", ".html");
                        Path traceFile = p.getParent().resolve(traceFilename);
                        if (Files.exists(traceFile)) {
                            individualResult.put("hasTraceFile", true);
                        } else {
                            individualResult.put("hasTraceFile", node.has("trace") && !node.get("trace").asText().isEmpty());
                        }

                        results.add(individualResult);
                    }
                } catch (IOException e) {
                    logger.warn("Failed to read individual result file {}: {}", p, e.getMessage());
                }
            }
        } catch (IOException e) {
            logger.error("Failed to list individual results: {}", e.getMessage(), e);
        }

        // Sort by timestamp descending
        results.sort((a, b) -> {
            String tsA = (String) a.get("timestamp");
            String tsB = (String) b.get("timestamp");
            if (tsA == null && tsB == null) return 0;
            if (tsA == null) return 1;
            if (tsB == null) return -1;
            return tsB.compareTo(tsA);
        });

        return results;
    }

    /**
     * Extracts agent from filename pattern: result_<agent>_<language>_<exercise>.json
     */
    private String extractAgentFromFilename(String filename) {
        if (filename.startsWith("result_")) {
            String withoutPrefix = filename.substring(7); // Remove "result_"
            String[] parts = withoutPrefix.split("_");
            if (parts.length >= 1) {
                return parts[0];
            }
        }
        return "unknown";
    }

    /**
     * Reads a specific cachedResult file by filename.
     */
    public Map<String, Object> getResultByFilename(String filename) throws IOException {
        CachedResult cached = cachedResults.get(filename);
        if (cached == null) {
            return null;
        }
        return toMetadataMap(cached);
    }

    /**
     * Reads the trace HTML content for a result.
     * Returns the embedded trace from the JSON or reads from a separate .html file.
     */
    public String getTraceContent(String filename) throws IOException {
        CachedResult cached = cachedResults.get(filename);
        if (cached != null) {
            // First try to get trace from embedded data in results (batch results)
            if (cached.results != null && !cached.results.isEmpty()) {
                Object traceObj = cached.results.get(0).get("trace");
                if (traceObj != null && !traceObj.toString().isEmpty()) {
                    return traceObj.toString();
                }
            }
            // Next, try to get trace directly from the JSON file for individual results
            Path resultFile = Paths.get(config.getOutput().getResultsDir()).resolve(filename);
            if (Files.exists(resultFile)) {
                try {
                    JsonNode node = objectMapper.readTree(resultFile.toFile());
                    if (node.has("trace")) {
                        String trace = node.get("trace").asText();
                        if (!trace.isEmpty()) {
                            return trace;
                        }
                    }
                } catch (Exception e) {
                    logger.debug("Failed to parse trace from JSON file {}: {}", filename, e.getMessage());
                }
            }
            // Then try separate HTML file
            if (cached.tracePath != null) {
                Path traceFile = Paths.get(cached.tracePath);
                if (Files.exists(traceFile)) {
                    return Files.readString(traceFile);
                }
            }
        }
        return null;
    }

    /**
     * Reads a specific cachedResult file by timestamp (searches for matching filename).
     */
    public Map<String, Object> getResultByTimestamp(String timestamp) throws IOException {
        for (CachedResult cached : cachedResults.values()) {
            if (cached.timestamp != null && cached.timestamp.contains(timestamp)) {
                return toMetadataMap(cached);
            }
        }
        return null;
    }

    /**
     * Gets detailed result for a specific exercise.
     */
    public ExerciseResult getExerciseResult(String agent, String language, String exerciseName) throws IOException {
        Path resultsDir = Paths.get(config.getOutput().getResultsDir());
        String filename = String.format("result_%s_%s_%s.json", agent, language, exerciseName);
        Path resultFile = resultsDir.resolve(filename);

        if (!Files.exists(resultFile)) {
            return null;
        }

        return objectMapper.readValue(resultFile.toFile(), ExerciseResult.class);
    }

    /**
     * Gets aggregate statistics.
     */
    public Map<String, Object> getStatistics() {
        return getStatistics(null, null, null);
    }

    /**
     * Gets filtered aggregate statistics from cached data.
     */
    public Map<String, Object> getStatistics(String language, String agent, String model) {
        int totalRuns = 0;
        int totalExercises = 0;
        int successfulExercises = 0;
        Map<String, Integer> byLanguage = new HashMap<>();
        Map<String, Integer> successByLanguage = new HashMap<>();
        Map<String, Double> durationByLanguage = new HashMap<>();
        Map<String, Integer> byAgent = new HashMap<>();
        Map<String, Integer> successByAgent = new HashMap<>();
        Map<String, Double> durationByAgent = new HashMap<>();
        Map<String, Integer> byModel = new HashMap<>();
        Map<String, Integer> successByModel = new HashMap<>();
        Map<String, Double> durationByModel = new HashMap<>();

        for (CachedResult cached : cachedResults.values()) {
            boolean matchesLanguage = language == null || language.isEmpty() ||
                    (cached.filename != null && cached.filename.contains(language));
            boolean matchesAgent = agent == null || agent.isEmpty() ||
                    (cached.agent != null && cached.agent.contains(agent));
            boolean matchesModel = model == null || model.isEmpty() ||
                    (cached.model != null && cached.model.equals(model));

            if (!matchesLanguage || !matchesAgent || !matchesModel) {
                continue;
            }

            totalRuns++;

            // By language - aggregate from individual exercises
            for (Map.Entry<String, Integer> entry : cached.exerciseCountByLanguage.entrySet()) {
                String lang = entry.getKey();
                int count = entry.getValue();

                byLanguage.putIfAbsent(lang, 0);
                byLanguage.put(lang, byLanguage.get(lang) + count);

                Integer successCount = cached.successCountByLanguage.get(lang);
                if (successCount != null) {
                    successByLanguage.putIfAbsent(lang, 0);
                    successByLanguage.put(lang, successByLanguage.get(lang) + successCount);
                    successfulExercises += successCount;
                }

                Double duration = cached.durationByLanguage.get(lang);
                if (duration != null) {
                    durationByLanguage.putIfAbsent(lang, 0.0);
                    durationByLanguage.put(lang, durationByLanguage.get(lang) + duration);
                }
            }

            totalExercises += cached.totalExercises;

            // By agent
            byAgent.putIfAbsent(cached.agent, 0);
            byAgent.put(cached.agent, byAgent.get(cached.agent) + cached.totalExercises);
            durationByAgent.putIfAbsent(cached.agent, 0.0);
            durationByAgent.put(cached.agent, durationByAgent.get(cached.agent) +
                    cached.durationByLanguage.values().stream().mapToDouble(Double::doubleValue).sum());

            if (cached.successful > 0) {
                successByAgent.putIfAbsent(cached.agent, 0);
                successByAgent.put(cached.agent, successByAgent.get(cached.agent) + cached.successful);
            }

            // By model
            byModel.putIfAbsent(cached.model, 0);
            byModel.put(cached.model, byModel.get(cached.model) + cached.totalExercises);
            durationByModel.putIfAbsent(cached.model, 0.0);
            durationByModel.put(cached.model, durationByModel.get(cached.model) +
                    cached.durationByLanguage.values().stream().mapToDouble(Double::doubleValue).sum());

            if (cached.successful > 0) {
                successByModel.putIfAbsent(cached.model, 0);
                successByModel.put(cached.model, successByModel.get(cached.model) + cached.successful);
            }
        }

        Map<String, Object> stats = new HashMap<>();
        stats.put("total_runs", totalRuns);
        stats.put("total_exercises", totalExercises);
        stats.put("successful_exercises", successfulExercises);
        stats.put("success_rate", totalExercises > 0 ? (double) successfulExercises / totalExercises * 100 : 0.0);
        stats.put("by_language", byLanguage);
        stats.put("success_by_language", successByLanguage);
        stats.put("duration_by_language", durationByLanguage);
        stats.put("duration_by_language_formatted", formatDurationMap(durationByLanguage));
        stats.put("by_agent", byAgent);
        stats.put("success_by_agent", successByAgent);
        stats.put("duration_by_agent", durationByAgent);
        stats.put("duration_by_agent_formatted", formatDurationMap(durationByAgent));
        stats.put("by_model", byModel);
        stats.put("success_by_model", successByModel);
        stats.put("duration_by_model", durationByModel);
        stats.put("duration_by_model_formatted", formatDurationMap(durationByModel));

        return stats;
    }

    /**
     * Formats a map of durations (in seconds) to human-readable strings.
     */
    private Map<String, String> formatDurationMap(Map<String, Double> durationMap) {
        Map<String, String> formatted = new HashMap<>();
        for (Map.Entry<String, Double> entry : durationMap.entrySet()) {
            formatted.put(entry.getKey(), formatDuration(entry.getValue()));
        }
        return formatted;
    }

    /**
     * Converts a CachedResult to a metadata map.
     */
    private Map<String, Object> toMetadataMap(CachedResult cached) {
        Map<String, Object> metadata = new HashMap<>();
        metadata.put("filename", cached.filename);
        metadata.put("path", cached.path);
        metadata.put("timestamp", cached.timestamp);
        metadata.put("agent", cached.agent);
        metadata.put("language", cached.language);
        metadata.put("model", cached.model);
        metadata.put("total_exercises", cached.totalExercises);
        metadata.put("successful", cached.successful);
        metadata.put("failed", cached.failed);
        metadata.put("success_rate", cached.successRate);
        metadata.put("results", cached.results);
        if (cached.tracePath != null) {
            metadata.put("tracePath", cached.tracePath);
        }
        return metadata;
    }

    /**
     * Formats duration in seconds to human-readable string (e.g., "1h 2m 30s").
     */
    private static String formatDuration(double totalSeconds) {
        if (totalSeconds == 0) return "0s";

        int days = (int) (totalSeconds / 86400);
        int hours = (int) ((totalSeconds % 86400) / 3600);
        int minutes = (int) ((totalSeconds % 3600) / 60);
        int seconds = (int) (totalSeconds % 60);

        StringBuilder sb = new StringBuilder();
        if (days > 0) sb.append(days).append("d ");
        if (hours > 0) sb.append(hours).append("h ");
        if (minutes > 0) sb.append(minutes).append("m ");
        sb.append(seconds).append("s");

        return sb.toString().trim();
    }

    /**
     * Gets the results directory path.
     */
    public Path getResultsDir() {
        return Paths.get(config.getOutput().getResultsDir());
    }

    /**
     * Refreshes the cache (called when new results are added).
     */
    public void refreshCache() {
        loadAllResults();
    }
}
