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
        Path parentDir = configuredResultsDir.getParent();
        if (parentDir == null || !Files.exists(parentDir)) {
            parentDir = configuredResultsDir;
        }

        if (!Files.exists(parentDir)) {
            logger.warn("Results directory does not exist: {}", parentDir);
            return;
        }

        int count = 0;
        try (Stream<Path> paths = Files.walk(parentDir)) {
            List<Path> resultFiles = paths.filter(Files::isRegularFile)
                    .filter(p -> p.toString().endsWith(".json"))
                    .filter(p -> p.getFileName().toString().startsWith("results_"))
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
     */
    private CachedResult loadCachedResult(Path resultFile) throws IOException {
        JsonNode node = objectMapper.readTree(resultFile.toFile());
        if (!node.has("results") || !node.get("results").isArray()) {
            return null;
        }

        CachedResult cached = new CachedResult();
        cached.filename = resultFile.getFileName().toString();
        cached.path = resultFile.toString();
        cached.model = resultFile.getParent().getFileName().toString();
        cached.timestamp = node.has("timestamp") ? node.get("timestamp").asText() : null;
        cached.agent = node.has("agent") ? node.get("agent").asText() : "unknown";
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

        return cached;
    }

    /**
     * Lists all cachedResult files with optional filtering.
     */
    public List<Map<String, Object>> listResults(String language, String agent, String model) {
        List<Map<String, Object>> results = new ArrayList<>();

        for (CachedResult cached : cachedResults.values()) {
            boolean matchesLanguage = language == null || language.isEmpty() ||
                    (cached.filename != null && cached.filename.contains(language));
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
        metadata.put("total_exercises", cached.totalExercises);
        metadata.put("successful", cached.successful);
        metadata.put("failed", cached.failed);
        metadata.put("success_rate", cached.successRate);
        metadata.put("results", cached.results);
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
