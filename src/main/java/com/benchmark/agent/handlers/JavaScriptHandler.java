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
 * Handler for JavaScript exercises.
 */
public class JavaScriptHandler implements LanguageHandler {
    private static final Logger logger = LoggerFactory.getLogger(JavaScriptHandler.class);

    @Override
    public String getLanguage() {
        return "javascript";
    }

    @Override
    public void copyReference(Exercise exercise, Path tempDir) throws IOException {
        for (Path refPath : exercise.getReferencePath()) {
            if (refPath == null || !Files.exists(refPath)) continue;

            String fileName = refPath.getFileName().toString();
            Path destFile = tempDir.resolve(fileName);
            Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied JavaScript reference file: {}", fileName);
        }
    }

    @Override
    public void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        for (Path testPath : exercise.getTestPath()) {
            String fileName = testPath.getFileName().toString();
            Path destFile = destDir.resolve(fileName);
            Files.copy(testPath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied JavaScript test file: {}", fileName);
        }
    }

    @Override
    public List<String> prepareWorkspaceCommand(Exercise exercise) {
        // JavaScript needs npm install to get dependencies
        return List.of("npm", "install");
    }

    @Override
    public List<String> getTestCommand(Exercise exercise) {
        // Just run tests, npm install was done in preparation
        return List.of("npm", "run", "test");
    }

    @Override
    public void patchTests(Path tempWorkDir) throws IOException {
        logger.info("Replacing xtest( with test( in JavaScript tests");

        // Find and process all JavaScript/TypeScript test files
        int[] errorCount = {0};
        try (var stream = Files.walk(tempWorkDir)) {
            stream.filter(Files::isRegularFile)
                    .filter(p -> {
                        String name = p.toString().toLowerCase();
                        return (name.endsWith(".js") || name.endsWith(".ts") || 
                                name.endsWith(".mjs") || name.endsWith(".cjs")) &&
                               (name.contains(".test.") || name.contains(".spec.") || 
                                name.contains("test") || name.contains("spec"));
                    })
                    .forEach(testFile -> {
                        try {
                            String testCode = Files.readString(testFile);
                            String updatedCode = testCode.replaceAll("\\bxtest\\(", "test(");
                            if (!testCode.equals(updatedCode)) {
                                Files.writeString(testFile, updatedCode);
                                logger.info("Patched xtest in {}", testFile);
                            }
                        } catch (IOException e) {
                            errorCount[0]++;
                            logger.error("Failed to patch {}: {}", testFile.getFileName(), e.getMessage());
                        }
                    });
        }

        if (errorCount[0] > 0) {
            logger.warn("Failed to patch {} JavaScript/TypeScript test file(s) - some xtest() calls may not have been enabled", errorCount[0]);
        }
    }
}
