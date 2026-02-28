package com.benchmark.web.controller;

import com.benchmark.web.service.BenchmarkService;
import org.springframework.stereotype.Controller;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;

/**
 * Controller for exercise discovery endpoints.
 * Handles listing available languages and exercises.
 */
@Controller
@RequestMapping
public class ExerciseController {
    private final BenchmarkService benchmarkService;

    public ExerciseController(BenchmarkService benchmarkService) {
        this.benchmarkService = benchmarkService;
    }

    /**
     * API endpoint to get available languages and exercises.
     */
    @GetMapping("/api/exercises")
    @ResponseBody
    public Map<String, List<String>> getExercises() {
        Map<String, List<String>> result = new java.util.HashMap<>();
        var exerciseRunner = benchmarkService.getExerciseRunner();
        for (String language : exerciseRunner.getAvailableLanguages()) {
            result.put(language, exerciseRunner.getExercisesForLanguage(language));
        }
        return result;
    }

    /**
     * API endpoint to get available languages only.
     */
    @GetMapping("/api/languages")
    @ResponseBody
    public List<String> getLanguages() {
        return benchmarkService.getExerciseRunner().getAvailableLanguages();
    }

    /**
     * API endpoint to get exercises for a specific language.
     */
    @GetMapping("/api/exercises/{language}")
    @ResponseBody
    public List<String> getExercisesForLanguage(@PathVariable String language) {
        return benchmarkService.getExerciseRunner().getExercisesForLanguage(language);
    }
}
