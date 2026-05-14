package io.schell.llm.benchmark.agent.handlers;

import io.schell.llm.benchmark.agent.LanguageHandler;
import io.schell.llm.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Path;
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
        copyReferenceFiles(exercise.getReferencePath(), tempDir);
    }

    @Override
    public void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        copyTestFiles(exercise.getTestPath(), destDir);
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
