package io.schell.llm.benchmark.agent.handlers;

import io.schell.llm.benchmark.agent.LanguageHandler;
import io.schell.llm.benchmark.exercise.ExerciseMetadata;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Unit tests for {@link JavaHandler}.
 */
class JavaHandlerTest {

    private LanguageHandler handler;

    @TempDir
    Path tempDir;

    @BeforeEach
    void setUp() {
        handler = new JavaHandler();
    }

    @Test
    void testGetLanguage() {
        // When
        String language = handler.getLanguage();

        // Then
        assertEquals("java", language);
    }

    @Test
    void testSupportsJavaExercise() throws Exception {
        // Given - create a minimal exercise structure
        Path exercisePath = tempDir.resolve("exercise");
        Files.createDirectories(exercisePath);
        
        ExerciseMetadata.Files files = new ExerciseMetadata.Files();
        files.setSolution(List.of("TwoFer.java"));
        files.setTest(List.of("TwoFerTest.java"));
        
        ExerciseMetadata metadata = new ExerciseMetadata();
        metadata.setFiles(files);
        
        io.schell.llm.benchmark.exercise.Exercise exercise = new io.schell.llm.benchmark.exercise.Exercise(
            "test-exercise", "java", exercisePath, metadata
        );

        // When
        boolean supports = handler.supports(exercise);

        // Then
        assertTrue(supports);
    }

    @Test
    void testSupportsNonJavaExercise() throws Exception {
        // Given
        Path exercisePath = tempDir.resolve("exercise");
        Files.createDirectories(exercisePath);
        
        ExerciseMetadata.Files files = new ExerciseMetadata.Files();
        files.setSolution(List.of("test.py"));
        files.setTest(List.of("test_test.py"));
        
        ExerciseMetadata metadata = new ExerciseMetadata();
        metadata.setFiles(files);
        
        io.schell.llm.benchmark.exercise.Exercise exercise = new io.schell.llm.benchmark.exercise.Exercise(
            "test-exercise", "python", exercisePath, metadata
        );

        // When
        boolean supports = handler.supports(exercise);

        // Then
        assertFalse(supports);
    }

    @Test
    void testSupportsCaseInsensitive() throws Exception {
        // Given
        Path exercisePath = tempDir.resolve("exercise");
        Files.createDirectories(exercisePath);
        
        ExerciseMetadata.Files files1 = new ExerciseMetadata.Files();
        files1.setSolution(List.of("Test.java"));
        ExerciseMetadata metadata1 = new ExerciseMetadata();
        metadata1.setFiles(files1);
        io.schell.llm.benchmark.exercise.Exercise exercise1 = new io.schell.llm.benchmark.exercise.Exercise(
            "test", "JAVA", exercisePath, metadata1
        );
        
        ExerciseMetadata.Files files2 = new ExerciseMetadata.Files();
        files2.setSolution(List.of("Test.java"));
        ExerciseMetadata metadata2 = new ExerciseMetadata();
        metadata2.setFiles(files2);
        io.schell.llm.benchmark.exercise.Exercise exercise2 = new io.schell.llm.benchmark.exercise.Exercise(
            "test", "Java", exercisePath, metadata2
        );

        // When & Then
        assertTrue(handler.supports(exercise1));
        assertTrue(handler.supports(exercise2));
    }

    @Test
    void testGetTestCommandForMaven() throws Exception {
        // Given - create a mock exercise with pom.xml
        Path exerciseDir = tempDir.resolve("exercise");
        Files.createDirectories(exerciseDir);
        Files.createFile(exerciseDir.resolve("pom.xml"));
        
        ExerciseMetadata.Files files = new ExerciseMetadata.Files();
        files.setSolution(List.of("Test.java"));
        files.setTest(List.of("TestTest.java"));
        
        ExerciseMetadata metadata = new ExerciseMetadata();
        metadata.setFiles(files);
        
        io.schell.llm.benchmark.exercise.Exercise exercise = new io.schell.llm.benchmark.exercise.Exercise(
            "test-exercise", "java", exerciseDir, metadata
        );

        // When
        List<String> command = handler.getTestCommand(exercise);

        // Then
        assertNotNull(command);
        assertEquals(3, command.size());
        assertEquals("mvn", command.get(0));
        assertEquals("test", command.get(1));
        assertEquals("-q", command.get(2));
    }

    @Test
    void testGetTestCommandForGradle() throws Exception {
        // Given - create a mock exercise with build.gradle
        Path exerciseDir = tempDir.resolve("exercise");
        Files.createDirectories(exerciseDir);
        Files.createFile(exerciseDir.resolve("build.gradle"));
        
        ExerciseMetadata.Files files = new ExerciseMetadata.Files();
        files.setSolution(List.of("Test.java"));
        files.setTest(List.of("TestTest.java"));
        
        ExerciseMetadata metadata = new ExerciseMetadata();
        metadata.setFiles(files);
        
        io.schell.llm.benchmark.exercise.Exercise exercise = new io.schell.llm.benchmark.exercise.Exercise(
            "test-exercise", "java", exerciseDir, metadata
        );

        // When
        List<String> command = handler.getTestCommand(exercise);

        // Then
        assertNotNull(command);
        assertEquals(4, command.size());
        assertEquals("./gradlew", command.get(0));
        assertEquals("test", command.get(1));
    }

    @Test
    void testGetTestCommandWhenNoBuildFile() throws Exception {
        // Given - exercise directory with no build file
        Path exerciseDir = tempDir.resolve("exercise");
        Files.createDirectories(exerciseDir);
        
        ExerciseMetadata.Files files = new ExerciseMetadata.Files();
        files.setSolution(List.of("Test.java"));
        files.setTest(List.of("TestTest.java"));
        
        ExerciseMetadata metadata = new ExerciseMetadata();
        metadata.setFiles(files);
        
        io.schell.llm.benchmark.exercise.Exercise exercise = new io.schell.llm.benchmark.exercise.Exercise(
            "test-exercise", "java", exerciseDir, metadata
        );

        // When
        List<String> command = handler.getTestCommand(exercise);

        // Then
        assertNotNull(command);
        assertEquals(1, command.size());
        assertEquals("false", command.get(0)); // Returns "false" as fallback
    }

    @Test
    void testPatchTestsRemovesDisabledAnnotations() throws Exception {
        // Given - create a test file with @Disabled annotation
        Path testDir = tempDir.resolve("src").resolve("test").resolve("java");
        Files.createDirectories(testDir);
        
        Path testFile = testDir.resolve("TestExample.java");
        String testContent = """
            package com.example;
            
            import org.junit.jupiter.api.Disabled;
            import org.junit.jupiter.api.Test;
            
            class TestExample {
                @Disabled("Not implemented yet")
                @Test
                void testSomething() {
                    // test code
                }
                
                @Test
                void testAnotherThing() {
                    // another test
                }
            }
            """;
        Files.writeString(testFile, testContent);

        // When
        handler.patchTests(tempDir);

        // Then
        String updatedContent = Files.readString(testFile);
        assertFalse(updatedContent.contains("@Disabled"));
        assertTrue(updatedContent.contains("@Test"));
    }

    @Test
    void testPatchTestsHandlesEmptyTestDirectory() throws Exception {
        // Given - no test directory exists
        
        // When & Then - should not throw
        assertDoesNotThrow(() -> handler.patchTests(tempDir));
    }

    @Test
    void testPatchTestsHandlesMultipleDisabledAnnotations() throws Exception {
        // Given - test file with multiple @Disabled annotations
        Path testDir = tempDir.resolve("src").resolve("test").resolve("java");
        Files.createDirectories(testDir);
        
        Path testFile = testDir.resolve("TestExample.java");
        String testContent = """
            package com.example;
            
            import org.junit.jupiter.api.Disabled;
            
            @Disabled("Class disabled")
            class TestExample {
                @Disabled("Method disabled")
                void test1() {}
                
                @Disabled(value = "Another reason")
                void test2() {}
            }
            """;
        Files.writeString(testFile, testContent);

        // When
        handler.patchTests(tempDir);

        // Then
        String updatedContent = Files.readString(testFile);
        long disabledCount = updatedContent.lines()
            .filter(line -> line.contains("@Disabled"))
            .count();
        assertEquals(0, disabledCount);
    }
}
