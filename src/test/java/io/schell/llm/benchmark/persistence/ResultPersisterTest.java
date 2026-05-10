package io.schell.llm.benchmark.persistence;

import io.schell.llm.benchmark.config.OutputConfig;
import io.schell.llm.benchmark.exercise.ExerciseResult;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.time.Instant;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Unit tests for {@link ResultPersister}.
 */
class ResultPersisterTest {

    private ResultPersister resultPersister;
    private OutputConfig outputConfig;

    @TempDir
    Path tempDir;

    @BeforeEach
    void setUp() {
        outputConfig = new OutputConfig();
        outputConfig.setResultsDir(tempDir.toString());
        resultPersister = new ResultPersister(outputConfig);
    }

    @Test
    void testSaveSingleResult() throws Exception {
        // Given
        ExerciseResult result = createMockExerciseResult("two-fer", "java", true);

        // When
        Path savedPath = resultPersister.saveResult(result, "reference", "java");

        // Then
        assertNotNull(savedPath);
        assertTrue(Files.exists(savedPath));
        assertTrue(savedPath.toString().endsWith(".json"));
        
        // Verify file content is valid JSON
        String content = Files.readString(savedPath);
        assertTrue(content.contains("\"exerciseName\""));
        assertTrue(content.contains("\"two-fer\""));
    }

    @Test
    void testSaveSingleResultWithFailure() throws Exception {
        // Given
        ExerciseResult result = createMockExerciseResult("hello-world", "python", false);

        // When
        Path savedPath = resultPersister.saveResult(result, "reference", "python");

        // Then
        assertTrue(Files.exists(savedPath));
    }

    @Test
    void testSaveMultipleResults() throws Exception {
        // Given
        List<ExerciseResult> results = List.of(
            createMockExerciseResult("exercise1", "java", true),
            createMockExerciseResult("exercise2", "java", true)
        );

        // When
        Path aggregatedPath = resultPersister.saveResults(results, "reference", new String[]{"java"});

        // Then
        assertNotNull(aggregatedPath);
        assertTrue(Files.exists(aggregatedPath));
        
        String content = Files.readString(aggregatedPath);
        assertTrue(content.contains("\"results\""));
    }

    @Test
    void testSaveResultsCreatesDirectoryStructure() throws Exception {
        // Given
        ExerciseResult result = createMockExerciseResult("test", "java", true);

        // When
        resultPersister.saveResult(result, "reference", "java");

        // Then
        assertTrue(Files.exists(tempDir));
    }

    @Test
    void testSaveResultsWithDifferentAgents() throws Exception {
        // Given
        ExerciseResult result1 = createMockExerciseResult("test", "java", true);
        ExerciseResult result2 = createMockExerciseResult("test", "java", true);

        // When
        Path path1 = resultPersister.saveResult(result1, "reference", "java");
        Path path2 = resultPersister.saveResult(result2, "claude", "java");

        // Then
        assertNotNull(path1);
        assertNotNull(path2);
    }

    @Test
    void testSaveResultWithEmptyOutput() throws Exception {
        // Given
        ExerciseResult result = ExerciseResult.builder()
            .exerciseName("empty-test")
            .language("go")
            .success(true)
            .exitCode(0)
            .output("")
            .duration(Duration.ZERO)
            .startTime(Instant.now())
            .endTime(Instant.now())
            .build();

        // When
        Path savedPath = resultPersister.saveResult(result, "reference", "go");

        // Then
        assertTrue(Files.exists(savedPath));
    }

    @Test
    void testSaveResultsWithSingleItem() throws Exception {
        // Given
        List<ExerciseResult> results = List.of(createMockExerciseResult("single", "rust", true));

        // When
        Path aggregatedPath = resultPersister.saveResults(results, "reference", new String[]{"rust"});

        // Then
        assertTrue(Files.exists(aggregatedPath));
    }

    @Test
    void testSaveResultsHandlesEmptyList() throws Exception {
        // Given
        List<ExerciseResult> results = List.of();

        // When
        Path aggregatedPath = resultPersister.saveResults(results, "reference", new String[]{});

        // Then
        assertTrue(Files.exists(aggregatedPath));
    }

    // Helper method to create mock ExerciseResult
    private ExerciseResult createMockExerciseResult(String exerciseName, String language, boolean success) {
        return ExerciseResult.builder()
            .exerciseName(exerciseName)
            .language(language)
            .success(success)
            .exitCode(success ? 0 : 1)
            .output("Test output")
            .duration(Duration.ofSeconds(10))
            .startTime(Instant.now())
            .endTime(Instant.now().plusSeconds(10))
            .build();
    }
}
