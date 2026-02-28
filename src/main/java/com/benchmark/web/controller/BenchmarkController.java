package com.benchmark.web.controller;

import com.benchmark.web.domain.BenchmarkSession;
import com.benchmark.web.domain.RunStatus;
import com.benchmark.web.service.BenchmarkService;
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
    private final ObjectMapper objectMapper;

    // Inference endpoint URL from config
    private final String inferenceEndpoint;

    public BenchmarkController(BenchmarkService benchmarkService) {
        this.benchmarkService = benchmarkService;
        this.objectMapper = new ObjectMapper();
        this.inferenceEndpoint = "http://localhost:8080";
    }

    /**
     * Dashboard page.
     */
    @GetMapping("/")
    public String dashboard(Model model) {
        Map<String, Object> stats = new HashMap<>(); // TODO: Get from ResultService
        model.addAttribute("stats", stats);
        List<BenchmarkSession> activeSessions = benchmarkService.getAllSessions().values().stream()
                .filter(s -> s.getStatus() == RunStatus.RUNNING || s.getStatus() == RunStatus.PENDING)
                .toList();
        model.addAttribute("activeRuns", activeSessions.size());
        model.addAttribute("activeSessions", activeSessions);
        model.addAttribute("queueItems", benchmarkService.getQueueItems());
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
     * Start a benchmark run via API.
     */
    @PostMapping("/api/benchmark/run")
    @ResponseBody
    public Map<String, Object> startBenchmarkRun(
            @RequestParam("agent") String agent,
            @RequestParam("language") String[] languages,
            @RequestParam(value = "model", required = false) String model,
            @RequestParam(value = "exercise", required = false) String exercise) {

        // Validate at least one language selected
        if (languages == null || languages.length == 0) {
            Map<String, Object> response = new HashMap<>();
            response.put("error", "At least one language must be selected");
            return response;
        }

        logger.info("Starting benchmark: agent={}, model={}, languages={}, exercise={}", 
                agent, model, String.join(",", languages), exercise);

        String sessionId = benchmarkService.startBenchmark(agent, languages, model, exercise);

        Map<String, Object> response = new HashMap<>();
        response.put("sessionId", sessionId);
        response.put("status", "started");
        response.put("redirectUrl", "/benchmark/" + sessionId);

        return response;
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
        String url = inferenceEndpoint + "/v1/models";

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .GET()
                .build();

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

    /**
     * Get statistics from ResultService.
     */
    private Map<String, Object> getStatistics() {
        // Delegate to ResultService via BenchmarkController for now
        // In future, could be moved to a dedicated StatisticsService
        return new HashMap<>();
    }
}
