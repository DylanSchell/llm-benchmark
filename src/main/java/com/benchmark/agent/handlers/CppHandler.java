package com.benchmark.agent.handlers;

import com.benchmark.agent.LanguageHandler;
import com.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.util.List;

/**
 * Handler for C++ exercises.
 */
public class CppHandler implements LanguageHandler {
    private static final Logger logger = LoggerFactory.getLogger(CppHandler.class);

    @Override
    public String getLanguage() {
        return "cpp";
    }

    @Override
    public void copyReference(Exercise exercise, Path tempDir) throws IOException {
        // For C++, create a subdirectory named after the exercise
        Path exerciseDir = tempDir.resolve(exercise.getName());
        Files.createDirectories(exerciseDir);

        logger.info("Copying C++ reference implementation to {}", exerciseDir);

        // Copy example solutions and rename to match stub file names
        for (Path examplePath : exercise.getExamples()) {
            try {
                String fileName = examplePath.getFileName().toString();
                String exampleExt = fileName.substring(fileName.lastIndexOf('.'));

                // Find matching stub file with same extension
                String stubFileName = null;
                for (Path refPath : exercise.getReferencePath()) {
                    String refExt = refPath.getFileName().toString()
                            .substring(refPath.getFileName().toString().lastIndexOf('.'));
                    if (refExt.equals(exampleExt)) {
                        stubFileName = refPath.getFileName().toString();
                        break;
                    }
                }

                if (stubFileName != null) {
                    Path destFile = exerciseDir.resolve(stubFileName);
                    Files.copy(examplePath, destFile, StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied C++ example as: {}", stubFileName);
                } else {
                    logger.warn("Could not find matching stub for C++ example: {}", fileName);
                }
            } catch (IOException e) {
                logger.error("Failed to copy C++ example {}: {}", examplePath, e.getMessage());
            }
        }
    }

    @Override
    public void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        Path exerciseDest = destDir.resolve(exercise.getName());
        Files.createDirectories(exerciseDest);
        
        for (Path testPath : exercise.getTestPath()) {
            String fileName = testPath.getFileName().toString();
            Path destFile = exerciseDest.resolve(fileName);
            Files.copy(testPath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied C++ test file: {}", fileName);
        }
    }

    @Override
    public List<String> getTestCommand(Exercise exercise) {
        return List.of("sh", "-c", 
            "mkdir -p build && cd build && cmake -DEXERCISM_RUN_ALL_TESTS=1 -G \"Unix Makefiles\" .. && make");
    }

    @Override
    public void patchTests(Path tempWorkDir) throws IOException {
        // C++ tests don't typically have skip annotations
        logger.debug("No test patching needed for C++");
    }

    @Override
    public String getContainerWorkDir(Exercise exercise) {
        return "/workspace/" + exercise.getName();
    }

    @Override
    public void copyExerciseFiles(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        // For C++, create a subdirectory named after the exercise
        Path exerciseDest = destDir.resolve(exercise.getName());
        Files.createDirectories(exerciseDest);

        logger.info("Copying C++ exercise files to {}", exerciseDest);

        // Copy all files except .meta/ directory
        copyDirectoryExcluding(sourceDir, exerciseDest, ".meta/");
    }
}
