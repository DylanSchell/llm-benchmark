package com.benchmark.dataset;

import com.benchmark.exercise.Exercise;
import com.benchmark.exercise.ExerciseMetadata;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.util.ArrayList;
import java.util.List;

/**
 * Loads SWE exercises from a JSON dataset into the benchmark exercise directory.
 *
 * For each exercise:
 * 1. Clones the repo at pre_fix_commit
 * 2. Creates exercise directory with issue description
 * 3. Copies repo files (code + tests)
 * 4. Creates exercise metadata
 */
public class SweExerciseLoader {
    private static final Logger logger = LoggerFactory.getLogger(SweExerciseLoader.class);
    private static final ObjectMapper mapper = new ObjectMapper();

    private final Path outputDir;
    private final Path tempDir;

    public SweExerciseLoader(Path outputDir) {
        this.outputDir = outputDir;
        this.tempDir = Path.of("/tmp/swe-loader-" + System.currentTimeMillis());
    }

    /**
     * Load exercises from a JSON dataset file.
     */
    public List<Exercise> load(Path datasetFile) throws IOException, InterruptedException {
        String json = Files.readString(datasetFile);
        List<SweExercise> exercises = mapper.readValue(json,
                mapper.getTypeFactory().constructCollectionType(List.class, SweExercise.class));

        logger.info("Loading {} exercises from {}", exercises.size(), datasetFile);

        List<Exercise> loaded = new ArrayList<>();
        for (SweExercise exercise : exercises) {
            try {
                Exercise loadedExercise = loadExercise(exercise);
                loaded.add(loadedExercise);
                logger.info("  ✓ Loaded {}", exercise.id());
            } catch (Exception e) {
                logger.error("  ✗ Failed to load {}: {}", exercise.id(), e.getMessage());
            }
        }

        logger.info("Loaded {} of {} exercises", loaded.size(), exercises.size());
        return loaded;
    }

    /**
     * Load a single exercise.
     */
    private Exercise loadExercise(SweExercise exercise) throws IOException, InterruptedException {
        // Create exercise directory
        String safeName = exercise.id().replace("/", "-").replace("#", "-");
        Path exerciseDir = outputDir.resolve(safeName);
        Files.createDirectories(exerciseDir);

        // Clone repo at pre_fix_commit
        Path repoDir = tempDir.resolve(safeName);
        try {
            cloneRepo(exercise.repoUrl(), exercise.preFixCommit(), repoDir);

            // Copy repo files to exercise directory
            copyRepoFiles(repoDir, exerciseDir);

            // Add test files from the PR (they don't exist at pre-fix commit)
            addTestFilesFromPr(safeName, exercise, exerciseDir);

            // Create description from issue
            createDescription(exercise, exerciseDir);

            // Create exercise metadata
            createMetadata(exercise, exerciseDir);

            // Return exercise info
            return new Exercise(
                    exercise.id(),
                    "java",
                    exerciseDir,
                    new ExerciseMetadata()
            );

        } finally {
            // Cleanup temp dir
            try {
                Files.walk(repoDir)
                        .sorted((a, b) -> b.compareTo(a))
                        .forEach(p -> {
                            try { Files.deleteIfExists(p); } catch (IOException e) { /* ignore */ }
                        });
            } catch (Exception e) {
                logger.warn("Failed to cleanup temp dir: {}", e.getMessage());
            }
        }
    }

    /**
     * Add test files from the PR to the exercise directory.
     * These files don't exist at pre-fix commit but are needed for the agent to know what to fix.
     */
    private void addTestFilesFromPr(String safeName, SweExercise exercise, Path exerciseDir) throws IOException, InterruptedException {
        // Clone repo at post_fix_commit to get the test files
        Path postFixDir = tempDir.resolve(safeName + "-post");
        try {
            cloneRepo(exercise.repoUrl(), exercise.postFixCommit(), postFixDir);

            // Copy test files from post-fix to exercise directory
            Path testSrcDir = postFixDir.resolve("src/test/java");
            if (Files.exists(testSrcDir)) {
                Files.walk(testSrcDir)
                        .filter(Files::isRegularFile)
                        .filter(p -> p.toString().endsWith(".java"))
                        .forEach(src -> {
                            try {
                                // Check if this is one of the test files from the PR
                                String relPath = testSrcDir.relativize(src).toString();
                                boolean isPrTestFile = exercise.testFiles().stream()
                                        .anyMatch(tf -> tf.endsWith(relPath));
                                
                                if (isPrTestFile) {
                                    Path dest = exerciseDir.resolve("src/test/java/" + relPath);
                                    Files.createDirectories(dest.getParent());
                                    Files.copy(src, dest, StandardCopyOption.REPLACE_EXISTING);
                                    logger.debug("  Added test file: {}", relPath);
                                }
                            } catch (IOException e) {
                                logger.warn("Failed to copy test file {}: {}", src.getFileName(), e.getMessage());
                            }
                        });
            }
        } finally {
            // Cleanup post-fix dir
            try {
                Files.walk(postFixDir)
                        .sorted((a, b) -> b.compareTo(a))
                        .forEach(p -> {
                            try { Files.deleteIfExists(p); } catch (IOException e) { /* ignore */ }
                        });
            } catch (Exception e) {
                logger.warn("Failed to cleanup post-fix dir: {}", e.getMessage());
            }
        }
    }

    /**
     * Clone a repo at a specific commit.
     */
    private void cloneRepo(String repoUrl, String commit, Path cloneDir) throws IOException, InterruptedException {
        if (Files.exists(cloneDir)) {
            Files.walk(cloneDir)
                    .sorted((a, b) -> b.compareTo(a))
                    .forEach(p -> {
                        try { Files.deleteIfExists(p); } catch (IOException e) { /* ignore */ }
                    });
        }

        // Clone without depth limit, then checkout the specific commit
        String cmd = String.format("git clone %s %s && cd %s && git checkout %s",
                repoUrl, cloneDir, cloneDir, commit);

        ProcessBuilder pb = new ProcessBuilder("sh", "-c", cmd);
        pb.redirectErrorStream(true);
        Process process = pb.start();

        // Wait for completion
        int exitCode = process.waitFor();
        if (exitCode != 0) {
            String output = new String(process.getInputStream().readAllBytes());
            throw new IOException(String.format("Git clone failed (exit %d): %s", exitCode, output.substring(0, Math.min(500, output.length()))));
        }
    }

    /**
     * Copy repo files to exercise directory (excluding .git).
     */
    private void copyRepoFiles(Path sourceDir, Path destDir) throws IOException {
        Files.walk(sourceDir)
                .filter(Files::isRegularFile)
                .filter(p -> !p.toString().contains("/.git/"))
                .forEach(src -> {
                    try {
                        Path dest = destDir.resolve(sourceDir.relativize(src));
                        Files.createDirectories(dest.getParent());
                        Files.copy(src, dest, StandardCopyOption.REPLACE_EXISTING);
                    } catch (IOException e) {
                        logger.warn("Failed to copy {}: {}", src.getFileName(), e.getMessage());
                    }
                });
    }

    /**
     * Create description.md from issue.
     */
    private void createDescription(SweExercise exercise, Path exerciseDir) throws IOException {
        String description = "# " + exercise.issueTitle() + "\n\n" + exercise.issueBody();
        Files.writeString(exerciseDir.resolve("description.md"), description);
    }

    /**
     * Create exercise metadata JSON.
     */
    private void createMetadata(SweExercise exercise, Path exerciseDir) throws IOException {
        // Detect build system from repo files
        String buildSystem = "maven";
        String testCommand = "mvn test -q";

        if (Files.exists(exerciseDir.resolve("build.gradle")) ||
            Files.exists(exerciseDir.resolve("build.gradle.kts"))) {
            buildSystem = "gradle";
            testCommand = "./gradlew test --no-daemon -q";
        } else if (Files.exists(exerciseDir.resolve("pom.xml"))) {
            buildSystem = "maven";
            testCommand = "mvn test -q";
        }

        // Create a simple metadata file for the benchmark runner
        var metadata = new java.util.LinkedHashMap<String, Object>();
        metadata.put("name", exercise.id());
        metadata.put("language", "java");
        metadata.put("build_system", buildSystem);
        metadata.put("test_command", testCommand);
        metadata.put("fix_files", exercise.fixFiles());
        metadata.put("test_files", exercise.testFiles());
        metadata.put("pre_fix_commit", exercise.preFixCommit());
        metadata.put("post_fix_commit", exercise.postFixCommit());
        metadata.put("repo_url", exercise.repoUrl());

        Files.writeString(exerciseDir.resolve("exercise.json"),
                mapper.writerWithDefaultPrettyPrinter().writeValueAsString(metadata));
    }

    /**
     * Clean up temp directory.
     */
    public void cleanup() {
        try {
            if (Files.exists(tempDir)) {
                Files.walk(tempDir)
                        .sorted((a, b) -> b.compareTo(a))
                        .forEach(p -> {
                            try { Files.deleteIfExists(p); } catch (IOException e) { /* ignore */ }
                        });
            }
        } catch (Exception e) {
            logger.warn("Failed to cleanup temp dir: {}", e.getMessage());
        }
    }
}
