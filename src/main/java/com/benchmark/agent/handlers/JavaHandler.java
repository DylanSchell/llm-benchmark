package com.benchmark.agent.handlers;

import com.benchmark.agent.LanguageHandler;
import com.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
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

        for (Path refPath : exercise.getReferencePath()) {
            if (refPath == null || !Files.exists(refPath)) {
                logger.warn("Reference path not found for: {}", exercise.getName());
                continue;
            }

            try (Stream<Path> paths = Files.walk(refPath)) {
                paths.filter(Files::isRegularFile)
                        .filter(p -> p.toString().endsWith(".java"))
                        .forEach(refFile -> {
                            try {
                                String fileName = refFile.getFileName().toString();
                                Path destFile = mainSrcDir.resolve(fileName);
                                Files.copy(refFile, destFile, StandardCopyOption.REPLACE_EXISTING);
                                logger.info("Copied reference file: {}", fileName);
                            } catch (IOException e) {
                                logger.error("Failed to copy reference file {}: {}", refFile, e.getMessage());
                            }
                        });
            }
        }

        // Copy example solutions
        for (Path examplePath : exercise.getExamples()) {
            try {
                String fileName = examplePath.getFileName().toString();
                Path destFile = tempDir.resolve("src/main/java").resolve(fileName);
                Files.copy(examplePath, destFile, StandardCopyOption.REPLACE_EXISTING);
                logger.info("Copied example file: {}", fileName);
            } catch (IOException e) {
                logger.error("Failed to copy example file {}: {}", examplePath, e.getMessage());
            }
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

        Files.walkFileTree(testDir, new SimpleFileVisitor<>() {
            @Override
            public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) {
                if (file.toString().endsWith(".java")) {
                    try {
                        String testCode = Files.readString(file);
                        String updatedCode = testCode.replaceAll("@Disabled\\(.*\\)", "");
                        Files.writeString(file, updatedCode);
                    } catch (IOException e) {
                        logger.error("Error reading file {}", file);
                    }
                }
                return FileVisitResult.CONTINUE;
            }
        });
    }
}
