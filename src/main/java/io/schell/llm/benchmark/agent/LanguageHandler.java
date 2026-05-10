package io.schell.llm.benchmark.agent;

import io.schell.llm.benchmark.exercise.Exercise;
import io.schell.llm.benchmark.util.FileUtils;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.util.List;
import java.util.stream.Stream;

/**
 * Strategy interface for language-specific operations.
 * Each language has its own handler that knows how to:
 * - Copy reference implementation files
 * - Copy test files  
 * - Get the appropriate test command
 * - Patch tests if needed
 */
public interface LanguageHandler {
    Logger logger = LoggerFactory.getLogger(LanguageHandler.class);

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
     * Gets the workspace preparation command for this language.
     * This is run once before the benchmark starts to set up dependencies, virtual environments, etc.
     * Override for languages that need setup (npm install, uv venv, etc.).
     *
     * @param exercise The exercise
     * @return List of command parts for workspace preparation, or null if no preparation needed
     */
    default List<String> prepareWorkspaceCommand(Exercise exercise) {
        return null; // No preparation needed by default
    }

    /**
     * Gets the test command for this language.
     * This is run after workspace preparation to execute tests.
     *
     * @param exercise The exercise
     * @return List of command parts for test execution (e.g., ["mvn", "test", "-q"])
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
     * Copies exercise files from source to destination.
     * Override for languages with special directory structure (e.g., C++).
     *
     * @param exercise The exercise
     * @param sourceDir Source directory containing exercise files
     * @param destDir   Destination directory
     * @throws IOException if file operations fail
     */
    default void copyExerciseFiles(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        // Default implementation: copy all files except .meta/ directory
        try (Stream<Path> paths = Files.walk(sourceDir)) {
            paths.forEach(sourcePath -> {
                try {
                    Path relativePath = sourceDir.relativize(sourcePath);

                    // Skip reference implementation directory
                    if (relativePath.toString().contains(".meta/")) {
                        return;
                    }

                    Path destPath = destDir.resolve(relativePath);

                    if (Files.isDirectory(sourcePath)) {
                        Files.createDirectories(destPath);
                    } else {
                        Files.copy(sourcePath, destPath, StandardCopyOption.REPLACE_EXISTING);
                    }
                } catch (IOException e) {
                    logger.error("Failed to copy file {}: {}", sourcePath, e.getMessage());
                }
            });
        }
    }

    /**
     * Checks if this handler supports the given exercise.
     */
    default boolean supports(Exercise exercise) {
        return getLanguage().equalsIgnoreCase(exercise.getLanguage());
    }

    /**
     * Gets the container working directory for this language.
     * Default is "/workspace", override for languages like C++ that use subdirectories.
     *
     * @param exercise The exercise
     * @return The container working directory path
     */
    default String getContainerWorkDir(Exercise exercise) {
        return "/workspace";
    }

    /**
     * Copies files from a collection of source paths to a destination directory.
     * Optionally filters by file extension.
     *
     * @param sourcePaths Collection of source file paths
     * @param destDir     Destination directory
     * @param fileFilter  File extension filter (e.g., ".java"), or null for no filter
     * @throws IOException if file operations fail
     */
    default void copyFilesFromPaths(Iterable<Path> sourcePaths, Path destDir, String fileFilter) throws IOException {
        FileUtils.copyFilesFromPaths(sourcePaths, destDir, fileFilter);
    }

    /**
     * Copies all files from a source directory to a destination directory,
     * excluding paths that contain the given exclusion pattern.
     *
     * @param sourceDir      Source directory
     * @param destDir        Destination directory
     * @param exclusionPattern Pattern to exclude (e.g., ".meta/")
     * @throws IOException if file operations fail
     */
    default void copyDirectoryExcluding(Path sourceDir, Path destDir, String exclusionPattern) throws IOException {
        FileUtils.copyDirectoryExcluding(sourceDir, destDir, exclusionPattern);
    }

    /**
     * Recursively replaces text patterns in files with the given extension.
     * Used for removing test annotations like @Disabled or #[ignore].
     *
     * @param directory       Directory to search
     * @param fileExtension   File extension to match (e.g., ".java")
     * @param pattern         Regex pattern to replace
     * @param replacement     Replacement string
     * @return Number of files that failed to process
     * @throws IOException if directory traversal fails
     */
    default int replaceInFilesRecursive(Path directory, String fileExtension, 
                                         String pattern, String replacement) throws IOException {
        return FileUtils.replaceInFilesRecursive(directory, fileExtension, pattern, replacement);
    }
}
