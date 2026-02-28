package com.benchmark.agent.handlers;

import com.benchmark.agent.LanguageHandler;
import com.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;

/**
 * Handler for Go exercises.
 */
public class GoHandler implements LanguageHandler {
    private static final Logger logger = LoggerFactory.getLogger(GoHandler.class);

    @Override
    public String getLanguage() {
        return "go";
    }

    @Override
    public void copyReference(Exercise exercise, Path tempDir) throws IOException {
        for (Path refPath : exercise.getReferencePath()) {
            if (refPath == null || !Files.exists(refPath)) continue;

            String fileName = refPath.getFileName().toString();
            Path destFile = tempDir.resolve(fileName);
            Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied Go reference file: {}", fileName);
        }
    }

    @Override
    public void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        for (Path testPath : exercise.getTestPath()) {
            String fileName = testPath.getFileName().toString();
            Path destFile = destDir.resolve(fileName);
            Files.copy(testPath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied Go test file: {}", fileName);
        }
    }

    @Override
    public List<String> getTestCommand(Exercise exercise) {
        return List.of("go", "test");
    }

    @Override
    public void patchTests(Path tempWorkDir) throws IOException {
        // Go doesn't have disabled tests by default
        logger.debug("No test patching needed for Go");
    }
}
