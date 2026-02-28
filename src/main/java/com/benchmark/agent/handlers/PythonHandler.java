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
 * Handler for Python exercises.
 */
public class PythonHandler implements LanguageHandler {
    private static final Logger logger = LoggerFactory.getLogger(PythonHandler.class);

    @Override
    public String getLanguage() {
        return "python";
    }

    @Override
    public void copyReference(Exercise exercise, Path tempDir) throws IOException {
        for (Path refPath : exercise.getReferencePath()) {
            if (refPath == null || !Files.exists(refPath)) continue;

            String fileName = refPath.getFileName().toString();
            Path destFile = tempDir.resolve(fileName);
            Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied Python reference file: {}", fileName);
        }
    }

    @Override
    public void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        for (Path testPath : exercise.getTestPath()) {
            String fileName = testPath.getFileName().toString();
            Path destFile = destDir.resolve(fileName);
            Files.copy(testPath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied Python test file: {}", fileName);
        }
    }

    @Override
    public List<String> getTestCommand(Exercise exercise) {
        return List.of("sh", "-c",
            "if [ -d \".venv\" ]; then source .venv/bin/activate; else uv venv && source .venv/bin/activate; fi && " +
            "uv pip install -q pytest && pytest");
    }

    @Override
    public void patchTests(Path tempWorkDir) throws IOException {
        // Python tests don't typically have skip annotations in this context
        logger.debug("No test patching needed for Python");
    }
}
