package com.benchmark.web.controller;

import com.benchmark.web.domain.BenchmarkSession;
import com.benchmark.web.domain.RunStatus;
import com.benchmark.web.service.BenchmarkService;
import com.benchmark.web.service.ResultService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.MediaType;
import org.springframework.stereotype.Controller;
import org.springframework.ui.Model;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.servlet.mvc.method.annotation.SseEmitter;

import java.io.IOException;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Controller for benchmark execution endpoints.
 */
@Controller
@RequestMapping
public class BenchmarkController {
    private static final Logger logger = LoggerFactory.getLogger(BenchmarkController.class);

    private final BenchmarkService benchmarkService;
    private final ResultService resultService;

    public BenchmarkController(BenchmarkService benchmarkService, ResultService resultService) {
        this.benchmarkService = benchmarkService;
        this.resultService = resultService;
    }

   /**
     * Test page.
     */
    @GetMapping("/test")
    public String test(Model model) {
        return "test";
    }

    /**
     * Dashboard page.
     */
    @GetMapping("/")
    public String dashboard(Model model) {
        Map<String, Object> stats = resultService.getStatistics();
        model.addAttribute("stats", stats);
        List<BenchmarkSession> activeSessions = benchmarkService.getAllSessions().values().stream()
                .filter(s -> s.getStatus() == RunStatus.RUNNING || s.getStatus() == RunStatus.PENDING)
                .toList();
        model.addAttribute("activeRuns", activeSessions.size());
        model.addAttribute("activeSessions", activeSessions);
        return "dashboard";
    }

    /**
     * Run benchmark form page.
     */
    @GetMapping("/run")
    public String runForm(Model model) {
        model.addAttribute("agents", new String[]{"reference", "claude"});
        model.addAttribute("languages", new String[]{"java", "go", "javascript", "python", "rust", "cpp"});
        return "run";
    }

    /**
     * Start a benchmark run via API.
     */
    @PostMapping("/api/benchmark/run")
    @ResponseBody
    public Map<String, Object> startBenchmark(
            @RequestParam("agent") String agent,
            @RequestParam("language") String language,
            @RequestParam(value = "exercise", required = false) String exercise) {

        logger.info("Starting benchmark: agent={}, language={}, exercise={}", agent, language, exercise);

        String sessionId = benchmarkService.startBenchmark(agent, language, exercise);

        Map<String, Object> response = new HashMap<>();
        response.put("sessionId", sessionId);
        response.put("status", "started");
        response.put("redirectUrl", "/run?session=" + sessionId);

        return response;
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
            } catch (IOException e) {
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

    /**
     * API endpoint to refresh result cache.
     */
    @PostMapping("/api/results/refresh")
    @ResponseBody
    public Map<String, Object> refreshResults() {
        resultService.refreshCache();
        Map<String, Object> response = new HashMap<>();
        response.put("status", "ok");
        response.put("message", "Result cache refreshed");
        return response;
    }

    /**
     * API endpoint to get recent results (returns JSON).
     */
    @GetMapping("/api/recent-results")
    @ResponseBody
    public List<Map<String, Object>> getRecentResults() {
        return resultService.listResults(null, null, null).stream().limit(10).toList();
    }

    /**
     * Recent results fragment for dashboard (returns HTML table rows).
     */
    @GetMapping("/recent-results-fragment")
    public String recentResultsFragment(Model model) {
        List<Map<String, Object>> results = resultService.listResults(null, null, null).stream().limit(10).toList();
        model.addAttribute("results", results);
        return "fragments/recent-results :: recentResultsRows";
    }

    /**
     * Get statistics for dashboard.
     */
    @GetMapping("/api/stats")
    @ResponseBody
    public Map<String, Object> getStats() {
        return resultService.getStatistics();
    }
}
