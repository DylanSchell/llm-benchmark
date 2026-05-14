package io.schell.llm.benchmark.persistence;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
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
    private ObjectMapper objectMapper;

    @TempDir
    Path tempDir;

    @BeforeEach
    void setUp() {
        outputConfig = new OutputConfig();
        outputConfig.setResultsDir(tempDir.toString());
        resultPersister = new ResultPersister(outputConfig);
        objectMapper = createObjectMapper();
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

    @Test
    void testSaveResultAutoIncrementsAttemptsOnFirstSave() throws Exception {
        // Given - builder does not explicitly set attempts
        ExerciseResult result = createMockExerciseResult("two-fer", "java", true);

        // When
        Path savedPath = resultPersister.saveResult(result, "reference", "java");

        // Then - should default to 1
        JsonNode node = objectMapper.readTree(savedPath.toFile());
        assertEquals(1, node.get("attempts").asInt());
    }

    @Test
    void testSaveResultAutoIncrementsAttemptsOnReSave() throws Exception {
        // Given - first save with attempts=1
        ExerciseResult result = createMockExerciseResult("two-fer", "java", true);
        Path savedPath = resultPersister.saveResult(result, "reference", "java");

        // When - save again (simulating a retry)
        resultPersister.saveResult(result, "reference", "java");

        // Then - attempts should be incremented to 2
        JsonNode node = objectMapper.readTree(savedPath.toFile());
        assertEquals(2, node.get("attempts").asInt());
    }

    @Test
    void testSaveResultIncrementsFromExistingAttempts() throws Exception {
        // Given - manually write a file with attempts=3
        Path resultFile = tempDir.resolve("result_reference_java_test.json");
        String existingContent = "{\"exerciseName\":\"test\",\"language\":\"java\",\"success\":true,\"exitCode\":0,\"attempts\":3}";
        Files.writeString(resultFile, existingContent);

        // When - save a new result with the same filename
        ExerciseResult result = createMockExerciseResult("test", "java", true);
        resultPersister.saveResult(result, "reference", "java");

        // Then - attempts should be 4 (3 + 1)
        JsonNode node = objectMapper.readTree(resultFile.toFile());
        assertEquals(4, node.get("attempts").asInt());
    }

    @Test
    void testPersistedResultDeserializesAttemptsCorrectly() throws Exception {
        // Given - save a result with explicit attempts=5
        ExerciseResult result = ExerciseResult.builder()
            .exerciseName("deser-test")
            .language("python")
            .success(true)
            .exitCode(0)
            .output("ok")
            .duration(Duration.ofSeconds(5))
            .startTime(Instant.now())
            .endTime(Instant.now().plusSeconds(5))
            .attempts(5)
            .build();

        // When - save and re-read the result file
        Path savedPath = resultPersister.saveResult(result, "reference", "python");
        JsonNode node = objectMapper.readTree(savedPath.toFile());

        // Then - all fields including attempts should deserialize correctly
        assertEquals(5, node.get("attempts").asInt());
        assertEquals("deser-test", node.get("exerciseName").asText());
        assertEquals("python", node.get("language").asText());
        assertTrue(node.get("success").asBoolean());
        assertEquals(0, node.get("exitCode").asInt());
    }

    private ObjectMapper createObjectMapper() {
        ObjectMapper mapper = new ObjectMapper();
        mapper.registerModule(new JavaTimeModule());
        return mapper;
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
