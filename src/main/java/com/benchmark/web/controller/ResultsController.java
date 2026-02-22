package com.benchmark.web.controller;

import com.benchmark.exercise.ExerciseResult;
import com.benchmark.web.service.ResultService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Controller;
import org.springframework.ui.Model;
import org.springframework.web.bind.annotation.*;

import java.io.IOException;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Controller for results browsing endpoints.
 */
@Controller
@RequestMapping("/results")
public class ResultsController {
    private static final Logger logger = LoggerFactory.getLogger(ResultsController.class);

    private final ResultService resultService;

    public ResultsController(ResultService resultService) {
        this.resultService = resultService;
    }

    /**
     * Results browser page.
     */
    @GetMapping
    public String resultsPage(@RequestParam(value = "language", required = false) String language,
                               @RequestParam(value = "agent", required = false) String agent,
                               @RequestParam(value = "model", required = false) String model,
                               Model modelAttr) {
        List<Map<String, Object>> results = resultService.listResults(language, agent, model);
        Map<String, Object> stats = resultService.getStatistics(language, agent, model);
        List<String> models = resultService.getModels();

        modelAttr.addAttribute("results", results);
        modelAttr.addAttribute("stats", stats);
        modelAttr.addAttribute("models", models);
        modelAttr.addAttribute("filterLanguage", language != null ? language : "");
        modelAttr.addAttribute("filterAgent", agent != null ? agent : "");
        modelAttr.addAttribute("filterModel", model != null ? model : "");

        return "results";
    }

    /**
     * API endpoint to list results with filtering.
     */
    @GetMapping("/api/results")
    @ResponseBody
    public List<Map<String, Object>> apiListResults(
            @RequestParam(value = "language", required = false) String language,
            @RequestParam(value = "agent", required = false) String agent,
            @RequestParam(value = "model", required = false) String model) {
        return resultService.listResults(language, agent, model);
    }

    /**
     * Result detail view by timestamp.
     */
    @GetMapping("/{filename}")
    public String resultDetail(@PathVariable String filename, Model model) {
        try {
            Map<String, Object> result = resultService.getResultByFilename(filename);
            if (result != null) {
                model.addAttribute("result", result);
            } else {
                model.addAttribute("error", "Result not found");
            }
        } catch (IOException e) {
            logger.error("Failed to load result: {}", e.getMessage());
            model.addAttribute("error", "Failed to load result: " + e.getMessage());
        }
        return "result-detail";
    }

    /**
     * Exercise detail view.
     */
    @GetMapping("/{language}/{exercise}")
    public String exerciseDetail(@PathVariable String language,
                                  @PathVariable String exercise,
                                  @RequestParam(value = "agent", required = false) String agent,
                                  Model model) {
        try {
            // Try to find the result for this exercise
            if (agent != null) {
                ExerciseResult result = resultService.getExerciseResult(agent, language, exercise);
                model.addAttribute("result", result);
            }
            model.addAttribute("language", language);
            model.addAttribute("exercise", exercise);
            model.addAttribute("availableAgents", new String[]{"reference", "claude"});
        } catch (IOException e) {
            logger.error("Failed to load exercise result: {}", e.getMessage());
            model.addAttribute("error", "Failed to load result: " + e.getMessage());
        }
        return "exercise-detail";
    }

    /**
     * API endpoint to get specific result data.
     */
    @GetMapping("/api/{filename}")
    @ResponseBody
    public Map<String, Object> apiGetResult(@PathVariable String filename) {
        try {
            return resultService.getResultByFilename(filename);
        } catch (IOException e) {
            Map<String, Object> error = new HashMap<>();
            error.put("error", "Failed to load result: " + e.getMessage());
            return error;
        }
    }

    /**
     * API endpoint to get exercise result.
     */
    @GetMapping("/api/{agent}/{language}/{exercise}")
    @ResponseBody
    public ExerciseResult apiGetExerciseResult(
            @PathVariable String agent,
            @PathVariable String language,
            @PathVariable String exercise) {
        try {
            return resultService.getExerciseResult(agent, language, exercise);
        } catch (IOException e) {
            logger.error("Failed to get exercise result: {}", e.getMessage());
            return null;
        }
    }

    /**
     * Get statistics.
     */
    @GetMapping("/api/stats")
    @ResponseBody
    public Map<String, Object> apiGetStats() {
        return resultService.getStatistics();
    }

    /**
     * Results table fragment for HTMX filtering.
     */
    @GetMapping("/table-fragment")
    public String resultsTableFragment(@RequestParam(value = "language", required = false) String language,
                                        @RequestParam(value = "agent", required = false) String agent,
                                        @RequestParam(value = "model", required = false) String model,
                                        Model modelAttr) {
        List<Map<String, Object>> results = resultService.listResults(language, agent, model);
        modelAttr.addAttribute("results", results);
        return "fragments/results-table :: resultsTableRows";
    }
}
