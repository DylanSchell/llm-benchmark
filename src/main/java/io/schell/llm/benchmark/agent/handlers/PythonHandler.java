package io.schell.llm.benchmark.agent.handlers;

import io.schell.llm.benchmark.agent.LanguageHandler;
import io.schell.llm.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Path;
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
        copyReferenceFiles(exercise.getReferencePath(), tempDir);
    }

    @Override
    public void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        copyTestFiles(exercise.getTestPath(), destDir);
    }

    @Override
    public List<String> prepareWorkspaceCommand(Exercise exercise) {
        // Python needs virtual environment setup
        return List.of("sh", "-c",
            "if [ ! -d \".venv\" ]; then uv venv; fi");
    }

    @Override
    public List<String> getTestCommand(Exercise exercise) {
        // Activate venv and run pytest (venv was created in preparation)
        // Use ". " instead of "source" for POSIX compatibility with dash/sh
        return List.of("sh", "-c",
            ". .venv/bin/activate && uv pip install -q pytest && pytest");
    }

    @Override
    public void patchTests(Path tempWorkDir) throws IOException {
        // Python tests don't typically have skip annotations in this context
        logger.debug("No test patching needed for Python");
    }
}
