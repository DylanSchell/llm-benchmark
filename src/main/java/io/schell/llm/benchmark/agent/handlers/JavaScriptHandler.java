package io.schell.llm.benchmark.agent.handlers;

import io.schell.llm.benchmark.agent.LanguageHandler;
import io.schell.llm.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
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
        copyReferenceFiles(exercise.getReferencePath(), tempDir);
    }

    @Override
    public void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        copyTestFiles(exercise.getTestPath(), destDir);
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
        int errorCount = 0;
        errorCount += replaceXtestInDirectory(tempWorkDir, ".js");
        errorCount += replaceXtestInDirectory(tempWorkDir, ".ts");
        errorCount += replaceXtestInDirectory(tempWorkDir, ".mjs");
        errorCount += replaceXtestInDirectory(tempWorkDir, ".cjs");

        if (errorCount > 0) {
            logger.warn("Failed to patch {} JavaScript/TypeScript test file(s) - some xtest() calls may not have been enabled", errorCount);
        }
    }

    /**
     * Replaces xtest( with test( in all files with the given extension.
     */
    private int replaceXtestInDirectory(Path directory, String extension) throws IOException {
        if (!Files.exists(directory)) {
            return 0;
        }

        int[] errorCount = {0};
        Files.walkFileTree(directory, new java.nio.file.SimpleFileVisitor<>() {
            @Override
            public java.nio.file.FileVisitResult visitFile(Path file,
                                                            java.nio.file.attribute.BasicFileAttributes attrs) {
                String name = file.toString().toLowerCase();
                boolean isTestFile = name.contains(".test.") || name.contains(".spec.") || 
                                    name.contains("test") || name.contains("spec");
                
                if (file.toString().endsWith(extension) && isTestFile) {
                    try {
                        String testCode = Files.readString(file);
                        String updatedCode = testCode.replaceAll("\\bxtest\\(", "test(");
                        if (!testCode.equals(updatedCode)) {
                            Files.writeString(file, updatedCode);
                            logger.info("Patched xtest in {}", file);
                        }
                    } catch (IOException e) {
                        errorCount[0]++;
                        logger.error("Failed to patch {}: {}", file.getFileName(), e.getMessage());
                    }
                }
                return java.nio.file.FileVisitResult.CONTINUE;
            }
        });

        return errorCount[0];
    }
}
