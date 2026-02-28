package com.benchmark.web.controller;

import com.benchmark.web.domain.BenchmarkQueueItem;
import com.benchmark.web.service.BenchmarkService;
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
     * Get the current benchmark queue.
     */
    @GetMapping("/api/benchmark/queue")
    @ResponseBody
    public List<BenchmarkQueueItem> getQueue() {
        return benchmarkService.getQueueItems();
    }

    /**
     * Schedule a batch of benchmark runs.
     */
    @PostMapping("/api/benchmark/queue/schedule")
    @ResponseBody
    public Map<String, Object> scheduleBatch(
            @RequestParam("agent") String agent,
            @RequestParam("language") String[] languages,
            @RequestParam(value = "model", required = false) String model,
            @RequestParam(value = "exercise", required = false) String exercise) {

        logger.info("Scheduling batch benchmark: agent={}, model={}, languages={}, exercise={}",
                agent, model, String.join(",", languages), exercise);

        List<BenchmarkQueueItem> items = benchmarkService.scheduleBatch(agent, languages, model, exercise);

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
}
