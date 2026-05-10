package com.benchmark.web.controller;

import com.benchmark.exercise.ExerciseResult;
import com.benchmark.web.service.ResultService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Controller;
import org.springframework.ui.Model;
import org.springframework.web.bind.annotation.*;
import jakarta.servlet.http.HttpServletResponse;

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
                               @RequestParam(value = "exercise", required = false) String exercise,
                               @RequestParam(value = "quick", required = false, defaultValue = "false") boolean quickOnly,
                               Model modelAttr) {
        logger.info("resultsPage called with: language='{}', agent='{}', model='{}', exercise='{}', quick={}", language, agent, model, exercise, quickOnly);
        if (model != null) {
            logger.info("Model parameter bytes: {}", java.util.Arrays.toString(model.getBytes(java.nio.charset.StandardCharsets.UTF_8)));
        }
        List<Map<String, Object>> results = resultService.listResults(language, agent, model, exercise, quickOnly);
        Map<String, Object> stats = resultService.getStatistics(language, agent, model, exercise, quickOnly);
        List<String> models = resultService.getModels();
        List<String> exercises = resultService.getExercises(language);

        // Also get individual results for the selected filters
        List<Map<String, Object>> individualResults = resultService.listIndividualResults(language, agent, model, exercise, quickOnly);

        modelAttr.addAttribute("results", results);
        modelAttr.addAttribute("individualResults", individualResults);
        modelAttr.addAttribute("stats", stats);
        modelAttr.addAttribute("models", models);
        modelAttr.addAttribute("exercises", exercises);
        modelAttr.addAttribute("filterLanguage", language != null ? language : "");
        modelAttr.addAttribute("filterAgent", agent != null ? agent : "");
        modelAttr.addAttribute("filterModel", model != null ? model : "");
        modelAttr.addAttribute("filterExercise", exercise != null ? exercise : "");
        modelAttr.addAttribute("filterQuick", quickOnly);

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
            @RequestParam(value = "model", required = false) String model,
            @RequestParam(value = "exercise", required = false) String exercise,
            @RequestParam(value = "quick", required = false, defaultValue = "false") boolean quickOnly) {
        return resultService.listResults(language, agent, model, exercise, quickOnly);
    }

    /**
     * Result detail view by agent/directory/language/exercise (e.g., /results/pi/pi-qwen35-122b/javascript/rational-numbers).
     * The directory uniquely identifies each model run.
     */
    @GetMapping("/{agent}/{directory}/{language}/{exercise}")
    public String resultDetail(@PathVariable String agent, @PathVariable String directory, @PathVariable String language, @PathVariable String exercise, Model modelAttr) {
        try {
            String key = directory + "/" + language + "/" + exercise;
            Map<String, Object> result = resultService.getResultByFilename(key);
            if (result != null) {
                modelAttr.addAttribute("result", result);
            } else {
                modelAttr.addAttribute("error", "Result not found");
            }
        } catch (IOException e) {
            logger.error("Failed to load result: {}", e.getMessage());
            modelAttr.addAttribute("error", "Failed to load result: " + e.getMessage());
        }
        return "result-detail";
    }

    /**
     * View trace HTML content for a result.
     * Serves the HTML trace directly (not wrapped in a Thymeleaf template).
     */
    @GetMapping("/{agent}/{directory}/{language}/{exercise}/trace")
    public void viewTrace(@PathVariable String agent, @PathVariable String directory, @PathVariable String language, @PathVariable String exercise, Model modelAttr, HttpServletResponse response) {
        try {
            String key = directory + "/" + language + "/" + exercise;
            String traceContent = resultService.getTraceContent(key);
            if (traceContent != null) {
                response.setContentType("text/html; charset=UTF-8");
                response.getWriter().write(traceContent);
            } else {
                response.setStatus(HttpServletResponse.SC_NOT_FOUND);
                response.setContentType("text/html; charset=UTF-8");
                response.getWriter().write("<html><body><h1>Trace not found</h1><p>No trace file available for this result.</p></body></html>");
            }
        } catch (IOException e) {
            logger.error("Failed to load trace: {}", e.getMessage());
            try {
                response.setStatus(HttpServletResponse.SC_INTERNAL_SERVER_ERROR);
                response.setContentType("text/html; charset=UTF-8");
                response.getWriter().write("<html><body><h1>Error</h1><p>Failed to load trace: " + e.getMessage() + "</p></body></html>");
            } catch (IOException ioEx) {
                logger.error("Failed to write error response: {}", ioEx.getMessage());
            }
        }
    }

    /**
     * Exercise detail view (must be before /{directory}/{filename} since language is a known set of values).
     */
    @GetMapping("/{language:java|python|go|javascript|rust|cpp}/{exercise}")
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
    @GetMapping("/api/{agent}/{directory}/{language}/{exercise}")
    @ResponseBody
    public Map<String, Object> apiGetResult(@PathVariable String agent, @PathVariable String directory, @PathVariable String language, @PathVariable String exercise) {
        try {
            String key = directory + "/" + language + "/" + exercise;
            return resultService.getResultByFilename(key);
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
                                        @RequestParam(value = "exercise", required = false) String exercise,
                                        @RequestParam(value = "quick", required = false, defaultValue = "false") boolean quickOnly,
                                        Model modelAttr) {
        List<Map<String, Object>> results = resultService.listResults(language, agent, model, exercise, quickOnly);
        List<Map<String, Object>> individualResults = resultService.listIndividualResults(language, agent, model, exercise, quickOnly);
        modelAttr.addAttribute("results", results);
        modelAttr.addAttribute("individualResults", individualResults);
        return "fragments/results-table :: resultsTableRows";
    }
}
