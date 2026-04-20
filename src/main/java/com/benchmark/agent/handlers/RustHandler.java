package com.benchmark.agent.handlers;

import com.benchmark.agent.LanguageHandler;
import com.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.util.List;

/**
 * Handler for Rust exercises.
 */
public class RustHandler implements LanguageHandler {
    private static final Logger logger = LoggerFactory.getLogger(RustHandler.class);

    @Override
    public String getLanguage() {
        return "rust";
    }

    @Override
    public void copyReference(Exercise exercise, Path tempDir) throws IOException {
        for (Path refPath : exercise.getReferencePath()) {
            if (refPath == null || !Files.exists(refPath)) continue;

            String fileName = refPath.getFileName().toString();
            Path destFile;
            
            if ("src".equals(refPath.getParent().getFileName().toString())) {
                Path srcDir = tempDir.resolve("src");
                Files.createDirectories(srcDir);
                destFile = srcDir.resolve(fileName);
            } else {
                destFile = tempDir.resolve(fileName);
            }
            
            Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied Rust reference file: {}", fileName);
        }

        // Copy Cargo-example.toml as Cargo.toml if it exists
        Path cargoExample = exercise.getExercisePath().resolve(".meta").resolve("Cargo-example.toml");
        if (Files.exists(cargoExample)) {
            Path destFile = tempDir.resolve("Cargo.toml");
            Files.copy(cargoExample, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied Rust Cargo-example.toml to Cargo.toml");
        }
    }

    @Override
    public void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        Path testsDir = sourceDir.resolve("tests");
        if (Files.exists(testsDir)) {
            Files.createDirectories(destDir.resolve("tests"));
            copyDirectoryExcluding(testsDir, destDir.resolve("tests"), null);
        }
    }

    @Override
    public List<String> getTestCommand(Exercise exercise) {
        return List.of("cargo", "test");
    }

    @Override
    public void patchTests(Path tempWorkDir) throws IOException {
        logger.info("Removing #[ignore] annotations from Rust tests");
        Path testsDir = tempWorkDir.resolve("tests");
        
        if (!Files.exists(testsDir)) {
            return;
        }

        int errorCount1 = replaceInFilesRecursive(testsDir, ".rs", "#\\[ignore]", "");
        int errorCount2 = replaceInFilesRecursive(testsDir, ".rs", "#\\[ignore[(].*[)]", "");
        
        int totalErrors = errorCount1 + errorCount2;
        if (totalErrors > 0) {
            logger.warn("Failed to patch {} Rust test file(s) - some #[ignore] annotations may not have been removed", totalErrors);
        }
    }
}
