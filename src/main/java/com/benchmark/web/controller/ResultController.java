package com.benchmark.web.controller;

import com.benchmark.web.service.ResultService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Controller;
import org.springframework.ui.Model;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;

/**
 * Controller for result-related endpoints.
 * Handles listing, refreshing, and statistics for benchmark results.
 */
@Controller
@RequestMapping
public class ResultController {
    private static final Logger logger = LoggerFactory.getLogger(ResultController.class);

    private final ResultService resultService;

    public ResultController(ResultService resultService) {
        this.resultService = resultService;
    }

    /**
     * API endpoint to refresh result cache.
     */
    @PostMapping("/api/results/refresh")
    @ResponseBody
    public Map<String, Object> refreshResults() {
        long startTime = System.currentTimeMillis();
        resultService.refreshCache();
        long duration = System.currentTimeMillis() - startTime;
        
        Map<String, Object> response = new java.util.HashMap<>();
        response.put("status", "ok");
        response.put("message", String.format("Result cache refreshed in %dms", duration));
        response.put("durationMs", duration);
        return response;
    }

    /**
     * API endpoint to get recent results (returns JSON).
     */
    @GetMapping("/api/recent-results")
    @ResponseBody
    public List<Map<String, Object>> getRecentResults() {
        return resultService.listResults(null, null, null);
    }

    /**
     * Recent results fragment for dashboard (returns HTML table rows).
     */
    @GetMapping("/recent-results-fragment")
    public String recentResultsFragment(Model model) {
        List<Map<String, Object>> results = resultService.listResults(null, null, null);
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

    /**
     * API endpoint to get filtered results.
     */
    @GetMapping("/api/results")
    @ResponseBody
    public List<Map<String, Object>> getResults(
            @RequestParam(required = false) String language,
            @RequestParam(required = false) String agent,
            @RequestParam(required = false) String model) {
        return resultService.listResults(language, agent, model);
    }

    /**
     * API endpoint to get individual exercise results.
     */
    @GetMapping("/api/individual-results")
    @ResponseBody
    public List<Map<String, Object>> getIndividualResults(
            @RequestParam(required = false) String language,
            @RequestParam(required = false) String agent,
            @RequestParam(required = false) String model) {
        return resultService.listIndividualResults(language, agent, model);
    }

    /**
     * API endpoint to get models list.
     */
    @GetMapping("/api/models")
    @ResponseBody
    public List<String> getModels() {
        return resultService.getModels();
    }
}
