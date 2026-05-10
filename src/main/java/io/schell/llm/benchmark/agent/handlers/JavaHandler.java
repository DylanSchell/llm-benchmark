package io.schell.llm.benchmark.agent.handlers;

import io.schell.llm.benchmark.agent.LanguageHandler;
import io.schell.llm.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.util.List;
import java.util.stream.Stream;

/**
 * Handler for Java exercises.
 */
public class JavaHandler implements LanguageHandler {
    private static final Logger logger = LoggerFactory.getLogger(JavaHandler.class);

    @Override
    public String getLanguage() {
        return "java";
    }

    @Override
    public void copyReference(Exercise exercise, Path tempDir) throws IOException {
        Path mainSrcDir = tempDir.resolve("src/main/java");
        Files.createDirectories(mainSrcDir);

        logger.info("Copying reference implementation to {}", mainSrcDir);

        // Copy reference Java files
        for (Path refPath : exercise.getReferencePath()) {
            if (refPath == null || !Files.exists(refPath)) {
                continue;
            }
            String fileName = refPath.getFileName().toString();
            if (fileName.endsWith(".java")) {
                Path destFile = mainSrcDir.resolve(fileName);
                Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
                logger.info("Copied reference file: {}", fileName);
            }
        }

        // Copy example solutions
        for (Path examplePath : exercise.getExamples()) {
            String fileName = examplePath.getFileName().toString();
            Path destFile = tempDir.resolve("src/main/java").resolve(fileName);
            Files.copy(examplePath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied example file: {}", fileName);
        }
    }

    @Override
    public void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        try (Stream<Path> paths = Files.walk(sourceDir)) {
            paths.forEach(sourcePath -> {
                try {
                    Path relativePath = sourceDir.relativize(sourcePath);
                    Path destPath = destDir.resolve(relativePath);

                    if (Files.isDirectory(sourcePath)) {
                        Files.createDirectories(destPath);
                    } else if (relativePath.toString().contains("src/test/java") && 
                               relativePath.toString().endsWith(".java")) {
                        logger.info("Copying fresh test from {} to {}", sourcePath, destPath);
                        Files.copy(sourcePath, destPath, StandardCopyOption.REPLACE_EXISTING);
                    }
                } catch (IOException e) {
                    logger.error("Failed to copy test file {}: {}", sourcePath, e.getMessage());
                }
            });
        }
    }

    @Override
    public List<String> getTestCommand(Exercise exercise) {
        // Determine the build system
        Path exerciseDir = exercise.getExercisePath();

        if (Files.exists(exerciseDir.resolve("pom.xml"))) {
            return List.of("mvn", "test", "-q");
        } else if (Files.exists(exerciseDir.resolve("build.gradle"))) {
            return List.of("./gradlew", "test", "--no-daemon", "-q");
        }

        logger.error("Unable to determine test command for Java exercise {}", exercise.getName());
        return List.of("false");
    }

    @Override
    public void patchTests(Path tempWorkDir) throws IOException {
        logger.info("Removing @Disabled annotations from Java tests");
        Path testDir = tempWorkDir.resolve("src").resolve("test");

        if (!Files.exists(testDir)) {
            return;
        }

        int errorCount = replaceInFilesRecursive(
            testDir, 
            ".java", 
            "@Disabled\\(.*\\)", 
            ""
        );

        if (errorCount > 0) {
            logger.warn("Failed to patch {} Java test file(s) - some @Disabled annotations may not have been removed", errorCount);
        }
    }
}
