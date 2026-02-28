package com.benchmark.agent;

import com.benchmark.exercise.Exercise;

import java.io.IOException;
import java.nio.file.Path;
import java.util.List;

/**
 * Strategy interface for language-specific operations.
 * Each language has its own handler that knows how to:
 * - Copy reference implementation files
 * - Copy test files  
 * - Get the appropriate test command
 * - Patch tests if needed
 */
public interface LanguageHandler {

    /**
     * Gets the language this handler supports.
     */
    String getLanguage();

    /**
     * Copies the reference implementation to the temp directory.
     *
     * @param exercise The exercise
     * @param tempDir  The temporary working directory
     * @throws IOException if file operations fail
     */
    void copyReference(Exercise exercise, Path tempDir) throws IOException;

    /**
     * Copies test files from the source to destination.
     *
     * @param exercise    The exercise
     * @param sourceDir   Source directory containing tests
     * @param destDir     Destination directory
     * @throws IOException if file operations fail
     */
    void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException;

    /**
     * Gets the test command for this language.
     *
     * @param exercise The exercise
     * @return List of command parts (e.g., ["mvn", "test", "-q"])
     */
    List<String> getTestCommand(Exercise exercise);

    /**
     * Patches tests in the temp directory (removes @Disabled, #[ignore], etc.).
     *
     * @param tempWorkDir The temporary working directory
     * @throws IOException if file operations fail
     */
    void patchTests(Path tempWorkDir) throws IOException;

    /**
     * Checks if this handler supports the given exercise.
     */
    default boolean supports(Exercise exercise) {
        return getLanguage().equalsIgnoreCase(exercise.getLanguage());
    }
}
