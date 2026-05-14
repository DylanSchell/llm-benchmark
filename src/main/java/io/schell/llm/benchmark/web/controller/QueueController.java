package io.schell.llm.benchmark.web.controller;

import io.schell.llm.benchmark.web.domain.BenchmarkQueueItem;
import io.schell.llm.benchmark.web.service.BenchmarkService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Controller;
import org.springframework.web.bind.annotation.*;

import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Controller for queue management endpoints.
 * Handles scheduling, canceling, and clearing benchmark queue items.
 */
@Controller
@RequestMapping
public class QueueController {
    private static final Logger logger = LoggerFactory.getLogger(QueueController.class);

    private final BenchmarkService benchmarkService;

    public QueueController(BenchmarkService benchmarkService) {
        this.benchmarkService = benchmarkService;
    }

    /**
     * Resolves the exercise parameter based on execution mode.
     * 
     * @param mode Execution mode: "single", "all", or "quick"
     * @param exercise The exercise name (used for "single" mode)
     * @return Internal exercise representation: null for all, "__quick__" for quick bench, or the exercise name
     */
    private String resolveExerciseParam(String mode, String exercise) {
        return switch (mode) {
            case "single" -> exercise;
            case "quick" -> "__quick__";
            default -> null; // "all" mode
        };
    }

    /**
     * Get the current benchmark queue.
     */
    @GetMapping("/api/benchmark/queue")
    @ResponseBody
    public List<BenchmarkQueueItem> getQueue() {
        return benchmarkService.getQueueItems();
    }

    /**
     * Schedule a batch of benchmark runs.
     * 
     * Execution modes:
     * - "single": exercise param specifies which exercise to run per language
     * - "all": no exercise param — all exercises for selected languages
     * - "quick": special marker — runs curated list of fast exercises (< 60s each)
     */
    @PostMapping("/api/benchmark/queue/schedule")
    @ResponseBody
    public Map<String, Object> scheduleBatch(
            @RequestParam("agent") String agent,
            @RequestParam("language") String[] languages,
            @RequestParam(value = "model", required = false) String model,
            @RequestParam(value = "exercise", required = false) String exercise,
            @RequestParam(value = "mode", defaultValue = "all") String mode,
            @RequestParam(value = "retry", defaultValue = "false") boolean retry) {

        // Translate frontend mode + exercise into the internal representation
        String effectiveExercise = resolveExerciseParam(mode, exercise);

        logger.info("Scheduling batch benchmark: agent={}, model={}, languages={}, mode={}, exercise={}, retry={}",
                agent, model, String.join(",", languages), mode, effectiveExercise, retry);

        List<BenchmarkQueueItem> items = benchmarkService.scheduleBatch(agent, languages, model, effectiveExercise, retry);

        Map<String, Object> response = new HashMap<>();
        response.put("status", "scheduled");
        response.put("items", items);
        response.put("count", items.size());

        return response;
    }

    /**
     * Cancel a queue item.
     */
    @PostMapping("/api/benchmark/queue/cancel/{itemId}")
    @ResponseBody
    public Map<String, Object> cancelQueueItem(@PathVariable String itemId) {
        Map<String, Object> response = new HashMap<>();
        boolean cancelled = benchmarkService.cancelQueueItem(itemId);

        if (cancelled) {
            response.put("status", "cancelled");
            response.put("itemId", itemId);
        } else {
            response.put("status", "error");
            response.put("message", "Could not cancel - item not found or already completed");
        }

        return response;
    }

    /**
     * Clear pending items from queue.
     */
    @PostMapping("/api/benchmark/queue/clear")
    @ResponseBody
    public Map<String, Object> clearPendingQueue() {
        benchmarkService.clearPendingQueue();

        Map<String, Object> response = new HashMap<>();
        response.put("status", "ok");
        response.put("message", "Pending queue items cleared");

        return response;
    }

    /**
     * Clear completed and cancelled items from the queue.
     */
    @PostMapping("/api/benchmark/queue/clear-terminal")
    @ResponseBody
    public Map<String, Object> clearCompletedAndCancelled() {
        int removed = benchmarkService.clearCompletedAndCancelled();

        Map<String, Object> response = new HashMap<>();
        response.put("status", "ok");
        response.put("message", String.format("%d completed/cancelled items cleared", removed));
        response.put("removed", removed);

        return response;
    }

    /**
     * Retry a failed queue item.
     */
    @PostMapping("/api/benchmark/queue/retry/{itemId}")
    @ResponseBody
    public Map<String, Object> retryQueueItem(@PathVariable String itemId) {
        Map<String, Object> response = new HashMap<>();
        BenchmarkQueueItem newItem = benchmarkService.retryQueueItem(itemId);

        if (newItem != null) {
            response.put("status", "retried");
            response.put("itemId", newItem.getId());
            response.put("message", "Item re-queued for retry");
        } else {
            response.put("status", "error");
            response.put("message", "Could not retry - item not found or not in failed state");
        }

        return response;
    }
}
