package com.benchmark.web.controller;

import com.benchmark.config.Config;
import com.benchmark.web.domain.BenchmarkQueueItem;
import com.benchmark.web.domain.BenchmarkSession;
import com.benchmark.web.domain.RunStatus;
import com.benchmark.web.service.BenchmarkService;
import com.benchmark.web.service.ResultService;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.MediaType;
import org.springframework.stereotype.Controller;
import org.springframework.ui.Model;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.servlet.mvc.method.annotation.SseEmitter;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.*;

/**
 * Controller for benchmark execution endpoints.
 * Handles starting, monitoring, and canceling benchmark runs.
 */
@Controller
@RequestMapping
public class BenchmarkController {
    private static final Logger logger = LoggerFactory.getLogger(BenchmarkController.class);

    private final BenchmarkService benchmarkService;
    private final ResultService resultService;
    private final ObjectMapper objectMapper;
    private final Config config;

    // Inference endpoint URL and API key from config
    private final String inferenceEndpoint;
    private final String apiKey;

    public BenchmarkController(BenchmarkService benchmarkService, ResultService resultService, Config config) {
        this.benchmarkService = benchmarkService;
        this.resultService = resultService;
        this.config = config;
        this.objectMapper = new ObjectMapper();
        this.inferenceEndpoint = config.getInferenceEndpoint();
        this.apiKey = config.getApiKey();
    }

    /**
     * Dashboard page.
     */
    @GetMapping("/")
    public String dashboard(@RequestParam(value = "quick", required = false, defaultValue = "false") boolean quickOnly, Model model) {
        // Get statistics from ResultService (aggregates all individual result files)
        Map<String, Object> stats = resultService.getStatistics(null, null, null, null, quickOnly);
        model.addAttribute("stats", stats);
        List<BenchmarkSession> activeSessions = benchmarkService.getAllSessions().values().stream()
                .filter(s -> s.getStatus() == RunStatus.RUNNING || s.getStatus() == RunStatus.PENDING)
                .toList();
        model.addAttribute("activeRuns", activeSessions.size());
        model.addAttribute("activeSessions", activeSessions);
        List<BenchmarkQueueItem> queueItems = benchmarkService.getQueueItems();
        model.addAttribute("queueItems", queueItems);

        // Count items by status for progress bar
        long runningCount = queueItems.stream().filter(i -> i.getStatus() == BenchmarkQueueItem.QueueItemStatus.RUNNING).count();
        long pendingCount = queueItems.stream().filter(i -> i.getStatus() == BenchmarkQueueItem.QueueItemStatus.PENDING).count();
        long completedCount = queueItems.stream().filter(i -> i.getStatus() == BenchmarkQueueItem.QueueItemStatus.COMPLETED).count();
        long failedCount = queueItems.stream().filter(i -> i.getStatus() == BenchmarkQueueItem.QueueItemStatus.FAILED).count();
        long cancelledCount = queueItems.stream().filter(i -> i.getStatus() == BenchmarkQueueItem.QueueItemStatus.CANCELLED).count();

        model.addAttribute("runningCount", runningCount);
        model.addAttribute("pendingCount", pendingCount);
        model.addAttribute("completedCount", completedCount);
        model.addAttribute("failedCount", failedCount);
        model.addAttribute("cancelledCount", cancelledCount);

        // Pre-compute percentage widths for progress bar segments
        int total = queueItems.size();
        double runningPct = total > 0 ? runningCount * 100.0 / total : 0;
        double pendingPct = total > 0 ? pendingCount * 100.0 / total : 0;
        double completedPct = total > 0 ? completedCount * 100.0 / total : 0;
        double failedPct = total > 0 ? failedCount * 100.0 / total : 0;
        double cancelledPct = total > 0 ? cancelledCount * 100.0 / total : 0;

        // Clamp the last visible segment so totals don't exceed 100% due to rounding
        double sum = runningPct + pendingPct + completedPct + failedPct + cancelledPct;
        if (sum > 100.0) {
            if (cancelledCount > 0) cancelledPct -= (sum - 100.0);
            else if (failedCount > 0) failedPct -= (sum - 100.0);
            else if (completedCount > 0) completedPct -= (sum - 100.0);
            else if (pendingCount > 0) pendingPct -= (sum - 100.0);
            else runningPct -= (sum - 100.0);
        }

        model.addAttribute("runningWidth", runningPct);
        model.addAttribute("pendingWidth", pendingPct);
        model.addAttribute("completedWidth", completedPct);
        model.addAttribute("failedWidth", failedPct);
        model.addAttribute("cancelledWidth", cancelledPct);

        return "dashboard";
    }

    /**
     * Run benchmark form page.
     */
    @GetMapping("/run")
    public String runForm(Model model) {
        // Fetch models from inference endpoint
        try {
            List<String> models = fetchModels();
            model.addAttribute("models", models);
        } catch (Exception e) {
            logger.warn("Could not fetch models from inference endpoint: {}", e.getMessage());
            model.addAttribute("models", Arrays.asList("sonnet", "qwen3-coder-next"));
        }

        return "run";
    }

    /**
     * View a benchmark session (running or completed).
     */
    @GetMapping("/benchmark/{id}")
    public String viewBenchmark(@PathVariable String id, Model model) {
        BenchmarkSession session = benchmarkService.getSession(id);

        if (session == null) {
            return "redirect:/";
        }

        model.addAttribute("sessionId", id);
        model.addAttribute("sessionStatus", session.getStatus().name());
        model.addAttribute("sessionProgress", session.getProgress());
        model.addAttribute("sessionCompleted", session.getCompletedExercises());
        model.addAttribute("sessionTotal", session.getTotalExercises());
        model.addAttribute("sessionOutput", session.getAccumulatedOutput());

        return "view_benchmark";
    }

    /**
     * Get status of a benchmark run.
     */
    @GetMapping("/api/benchmark/{id}/status")
    @ResponseBody
    public Map<String, Object> getStatus(@PathVariable String id) {
        BenchmarkSession session = benchmarkService.getSession(id);
        Map<String, Object> response = new HashMap<>();

        if (session == null) {
            response.put("error", "Session not found");
            return response;
        }

        response.put("id", session.getId());
        response.put("status", session.getStatus().name());
        response.put("agent", session.getAgentName());
        response.put("language", session.getLanguage());
        response.put("exercise", session.getExerciseName());
        response.put("progress", session.getProgress());
        response.put("completedExercises", session.getCompletedExercises());
        response.put("totalExercises", session.getTotalExercises());

        if (session.getErrorMessage() != null) {
            response.put("errorMessage", session.getErrorMessage());
        }

        return response;
    }

    /**
     * SSE endpoint for live output streaming.
     */
    @GetMapping(value = "/api/benchmark/{id}/stream", produces = MediaType.TEXT_EVENT_STREAM_VALUE)
    public SseEmitter streamOutput(@PathVariable String id) {
        BenchmarkSession session = benchmarkService.getSession(id);
        if (session == null) {
            SseEmitter emitter = new SseEmitter();
            try {
                emitter.send(SseEmitter.event().name("error").data("Session not found"));
            } catch (Exception e) {
                // Ignore
            }
            emitter.complete();
            return emitter;
        }
        return session.getSseEmitter();
    }

    /**
     * Cancel a running benchmark.
     */
    @PostMapping("/api/benchmark/{id}/cancel")
    @ResponseBody
    public Map<String, Object> cancelBenchmark(@PathVariable String id) {
        Map<String, Object> response = new HashMap<>();
        boolean cancelled = benchmarkService.cancelSession(id);

        if (cancelled) {
            response.put("status", "cancelled");
            response.put("message", "Benchmark run cancelled");
        } else {
            response.put("status", "error");
            response.put("message", "Could not cancel - session not found or not running");
        }

        return response;
    }

    /**
     * API endpoint to get active runs count.
     */
    @GetMapping("/api/active-runs")
    @ResponseBody
    public Map<String, Object> getActiveRuns() {
        Map<String, Object> response = new HashMap<>();
        long count = benchmarkService.getAllSessions().values().stream()
                .filter(s -> s.getStatus() == RunStatus.RUNNING || s.getStatus() == RunStatus.PENDING)
                .count();
        response.put("count", count);
        return response;
    }

    /**
     * API endpoint to get active benchmark sessions.
     */
    @GetMapping("/api/active-sessions")
    @ResponseBody
    public List<BenchmarkSession> getActiveSessions() {
        return benchmarkService.getAllSessions().values().stream()
                .filter(s -> s.getStatus() == RunStatus.RUNNING || s.getStatus() == RunStatus.PENDING)
                .toList();
    }

    // =============================================================================
    // Helper Methods
    // =============================================================================

    /**
     * Fetch available models from the inference endpoint.
     */
    private List<String> fetchModels() throws Exception {
        HttpClient client = HttpClient.newHttpClient();
        String url = inferenceEndpoint + "/models";

        HttpRequest.Builder requestBuilder = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .GET();

        // Add API key header if configured
        if (apiKey != null && !apiKey.isEmpty()) {
            requestBuilder.header("Authorization", "Bearer " + apiKey);
        }

        HttpRequest request = requestBuilder.build();

        HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());

        if (response.statusCode() == 200) {
            // Parse JSON response to extract model IDs
            String body = response.body();
            logger.info("Models response: {}", body);
            JsonNode rootNode = objectMapper.readTree(body);
            JsonNode dataNode = rootNode.get("data");

            if (dataNode != null && dataNode.isArray()) {
                List<String> models = new ArrayList<>();
                for (JsonNode modelNode : dataNode) {
                    JsonNode idNode = modelNode.get("id");
                    if (idNode != null && idNode.isTextual()) {
                        models.add(idNode.asText());
                    }
                }
                logger.info("Found {} models: {}", models.size(), models);
                return models;
            } else {
                logger.warn("'data' field not found or not an array in response");
            }
        } else {
            logger.warn("Failed to fetch models, status code: {}", response.statusCode());
        }

        return Arrays.asList("sonnet", "qwen3-coder-next");
    }

}
