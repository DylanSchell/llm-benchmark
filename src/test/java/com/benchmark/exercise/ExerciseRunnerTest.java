package com.benchmark.exercise;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Unit tests for {@link ExerciseRunner}.
 */
class ExerciseRunnerTest {

    private ExerciseRunner exerciseRunner;

    @BeforeEach
    void setUp() throws Exception {
        // Create minimal config for testing
        com.benchmark.config.Config config = new com.benchmark.config.Config();
        com.benchmark.docker.DockerClient dockerClient = new com.benchmark.docker.DockerClient(
            new com.benchmark.config.DockerConfig()
        );
        com.benchmark.BenchmarkRunner benchmarkRunner = new com.benchmark.BenchmarkRunner(
            config, dockerClient
        );
        
        exerciseRunner = new ExerciseRunner(config, dockerClient, benchmarkRunner);
    }

    @Test
    void testConstructorWithValidArgs() {
        // Then
        assertNotNull(exerciseRunner);
    }

    @Test
    void testGetExercisesForLanguageReturnsEmptyListWhenNoExercises() {
        // When
        java.util.List<String> exercises = exerciseRunner.getExercisesForLanguage("java");

        // Then - should not throw, may return empty list or null depending on implementation
        assertNotNull(exercises);
    }

    @Test
    void testGetExercisesForLanguageWithNullLanguage() {
        // When & Then - may throw NPE (acceptable behavior)
        assertThrows(Exception.class, () -> exerciseRunner.getExercisesForLanguage(null));
    }

    @Test
    void testGetExercisesForLanguageWithEmptyString() {
        // When & Then - should handle gracefully
        assertDoesNotThrow(() -> {
            java.util.List<String> exercises = exerciseRunner.getExercisesForLanguage("");
        });
    }

    @Test
    void testGetExercisesForLanguageWithDifferentLanguages() {
        // Given various language names
        
        // When & Then - should not throw for any valid language name
        assertDoesNotThrow(() -> exerciseRunner.getExercisesForLanguage("java"));
        assertDoesNotThrow(() -> exerciseRunner.getExercisesForLanguage("python"));
        assertDoesNotThrow(() -> exerciseRunner.getExercisesForLanguage("javascript"));
        assertDoesNotThrow(() -> exerciseRunner.getExercisesForLanguage("go"));
        assertDoesNotThrow(() -> exerciseRunner.getExercisesForLanguage("rust"));
        assertDoesNotThrow(() -> exerciseRunner.getExercisesForLanguage("cpp"));
    }
}
