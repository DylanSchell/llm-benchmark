package com.benchmark.agent;

import com.benchmark.docker.DockerClient;
import com.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.time.Duration;
import java.time.Instant;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Stream;

/**
 * Reference agent that validates exercises by:
 * 1. Copying exercise content to a temporary directory in Docker
 * 2. Copying reference implementation files to the correct location
 * 3. Running tests to verify the solution
 */
public class ReferenceAgent {
    private static final Logger logger = LoggerFactory.getLogger(ReferenceAgent.class);

    private final DockerClient dockerClient;

    public ReferenceAgent(DockerClient dockerClient) {
        this.dockerClient = dockerClient;
    }

    protected DockerClient getDockerClient() {
        return dockerClient;
    }

    /**
     * Result of running an exercise with the reference agent.
     */
    public record ReferenceResult(String exerciseName, String language, boolean success, int exitCode, String output,
                                  Duration duration, Instant startTime, Instant endTime, String errorMessage,
                                  String trace, String containerId) {

        public static Builder builder() {
            return new Builder();
        }

        public static class Builder {
            private String exerciseName;
            private String language;
            private boolean success;
            private int exitCode;
            private String output;
            private Duration duration;
            private Instant startTime;
            private Instant endTime;
            private String errorMessage;
            private String trace;
            private String containerId;

            public Builder containerId(String containerId) {
                this.containerId = containerId;
                return this;
            }

            public Builder exerciseName(String exerciseName) {
                this.exerciseName = exerciseName;
                return this;
            }

            public Builder language(String language) {
                this.language = language;
                return this;
            }

            public Builder success(boolean success) {
                this.success = success;
                return this;
            }

            public Builder exitCode(int exitCode) {
                this.exitCode = exitCode;
                return this;
            }

            public Builder output(String output) {
                this.output = output;
                return this;
            }

            public Builder duration(Duration duration) {
                this.duration = duration;
                return this;
            }

            public Builder startTime(Instant startTime) {
                this.startTime = startTime;
                return this;
            }

            public Builder endTime(Instant endTime) {
                this.endTime = endTime;
                return this;
            }

            public Builder errorMessage(String errorMessage) {
                this.errorMessage = errorMessage;
                return this;
            }

            public Builder trace(String trace) {
                this.trace = trace;
                return this;
            }

            public ReferenceResult build() {
                return new ReferenceResult(exerciseName, language, success, exitCode, output,
                        duration, startTime, endTime, errorMessage, trace, containerId);
            }
        }
    }

    /**
     * Runs an exercise using the reference implementation and executes tests.
     *
     * @param exercise        The exercise to run
     * @param hostExerciseDir The directory containing the exercise on the host
     * @return ReferenceResult with the test execution outcome
     */
    public ReferenceResult runReferenceSolution(Exercise exercise, Path hostExerciseDir, Path resultDir) {
        Instant startTime = Instant.now();
        logger.info("Running reference agent for exercise: {}", exercise.getName());

        try {
            // Create temporary working directory for this exercise
            // Directory is created in project root so it's accessible to Docker container
            Path tempWorkDir = createTempWorkDir(exercise);
            logger.info("Created temporary work directory: {}", tempWorkDir);

            // Copy exercise files to temp directory (excluding reference implementation)
            copyExerciseFiles(exercise, hostExerciseDir, tempWorkDir);

            ReferenceResult agentResult = runAgent(exercise, hostExerciseDir, tempWorkDir, resultDir);

            // Run tests inside Docker container
            ReferenceResult testResult = runTestsInDocker(exercise, hostExerciseDir, tempWorkDir, startTime);

            // Cleanup temp directory
            cleanupTempDir(tempWorkDir);

            return ReferenceResult.builder()
                    .exerciseName(agentResult.exerciseName)
                    .startTime(agentResult.startTime)
                    .endTime(agentResult.endTime)
                    .duration(agentResult.duration)
                    .success(testResult.success)
                    .errorMessage(!agentResult.success ? agentResult.errorMessage : testResult.errorMessage)
                    .exitCode(!agentResult.success ? agentResult.exitCode : testResult.exitCode)
                    .output(agentResult.output + "\n" + testResult.output)
                    .language(agentResult.language)
                    .trace(agentResult.trace)
                    .build();
        } catch (Exception e) {
            Instant endTime = Instant.now();
            Duration duration = Duration.between(startTime, endTime);
            logger.error("Reference agent failed for exercise {}: {}", exercise.getName(), e.getMessage(), e);

            return ReferenceResult.builder()
                    .exerciseName(exercise.getName())
                    .language(exercise.getLanguage())
                    .success(false)
                    .duration(duration)
                    .startTime(startTime)
                    .endTime(endTime)
                    .errorMessage(e.getMessage())
                    .build();
        }
    }

    /**
     * This is the reference agent implementation of "running". It just copies
     * the reference to the correct directory, this should make all tests pass.
     *
     * @param exercise
     * @param hostExerciseDir
     * @param tempWorkDir
     */
    protected ReferenceResult runAgent(Exercise exercise, Path hostExerciseDir, Path tempWorkDir, Path resultDir) throws IOException {
        // Copy reference implementation to source directory
        Instant startTime = Instant.now();
        if (exercise.getReferencePath() != null) {
            copyReferenceImplementation(exercise, tempWorkDir);
        } else {
            logger.warn("No reference implementation found for: {}", exercise.getName());
        }
        Instant endTime = Instant.now();
        return ReferenceResult.builder()
                .exerciseName(exercise.getName())
                .language(exercise.getLanguage())
                .startTime(startTime)
                .endTime(endTime)
                .duration(Duration.between(startTime, endTime))
                .errorMessage(null)
                .exitCode(0)
                .output("")
                .success(true)
                .build();
    }

    /**
     * Creates a temporary working directory for the exercise.
     * The directory is created in the current working directory so it's accessible
     * to the Docker container (which mounts CWD to /workspace).
     */
    private Path createTempWorkDir(Exercise exercise) throws IOException {
        Path baseDir = Path.of(System.getProperty("user.dir"));
        Path baseTempDir = baseDir.resolve(".benchmark-temp");
        Files.createDirectories(baseTempDir);
        Path exerciseTempDir = baseTempDir.resolve(exercise.getName() + "-" + System.currentTimeMillis());
        Files.createDirectories(exerciseTempDir);
        return exerciseTempDir;
    }

    /**
     * Copies exercise files to the temporary directory, excluding reference implementation files.
     */
    private void copyExerciseFiles(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        logger.info("Copying exercise files from {} to {}", sourceDir, destDir);

        try (Stream<Path> paths = Files.walk(sourceDir)) {
            paths.forEach(sourcePath -> {
                try {
                    Path relativePath = sourceDir.relativize(sourcePath);

                    // Skip reference implementation directory
                    if (relativePath.toString().contains(".meta/src/reference")) {
                        logger.debug("Skipping reference file: {}", relativePath);
                        return;
                    }

                    Path destPath = destDir.resolve(relativePath);

                    if (Files.isDirectory(sourcePath)) {
                        Files.createDirectories(destPath);
                    } else {
                        Files.copy(sourcePath, destPath, StandardCopyOption.REPLACE_EXISTING);
                        if (destPath.endsWith("gradle-wrapper.properties")) {
                            String wrapperProperties = Files.readString(destPath);
                            wrapperProperties = wrapperProperties.replace("distributionUrl=https\\://services.gradle.org/distributions/gradle-8.7-bin.zip", "distributionUrl=file:///opt/gradle/gradle-8.7-bin.zip");
                            Files.writeString(destPath, wrapperProperties);
                        }
                    }
                } catch (IOException e) {
                    logger.error("Failed to copy file {}: {}", sourcePath, e.getMessage());
                }
            });
        }
    }

    protected void copyFreshTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        if ("java".equals(exercise.getLanguage())) {
            try (Stream<Path> paths = Files.walk(sourceDir)) {
                paths.forEach(sourcePath -> {
                    try {
                        Path relativePath = sourceDir.relativize(sourcePath);
                        Path destPath = destDir.resolve(relativePath);
                        if (Files.isDirectory(sourcePath)) {
                            Files.createDirectories(destPath);
                        } else {
                            // Skip reference implementation directory
                            if (relativePath.toString().contains("src/test/java") && relativePath.endsWith(".java")) {
                                logger.info("Copying fresh tests from {} to {}", sourceDir, destDir);
                                Files.copy(sourcePath, destPath, StandardCopyOption.REPLACE_EXISTING);
                            }
                        }
                    } catch (IOException e) {
                        logger.error("Failed to copy file {}: {}", sourcePath, e.getMessage());
                    }
                });
            }
        }
    }

    /**
     * Copies the reference implementation Java files to the main source directory.
     */
    private void copyReferenceImplementation(Exercise exercise, Path tempDir) throws IOException {
        Path refDir = exercise.getReferencePath().getParent();

        if (refDir == null || !Files.exists(refDir)) {
            logger.warn("Reference directory not found for: {}", exercise.getName());
            return;
        }
        if (exercise.getLanguage().equals("java")) {
            Path mainSrcDir = tempDir.resolve("src/main/java");
            Files.createDirectories(mainSrcDir);

            logger.info("Copying reference implementation from {} to {}", refDir, mainSrcDir);

            try (Stream<Path> paths = Files.walk(refDir)) {
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
        } else if (exercise.getLanguage().equals("go")) {
            try (Stream<Path> paths = Files.walk(refDir)) {
                paths.filter(Files::isRegularFile)
                        .filter(p -> p.toString().endsWith(".go"))
                        .forEach(refFile -> {
                            try {
                                String fileName = refFile.getFileName().toString();
                                Path destFile = tempDir.resolve(fileName);
                                Files.copy(refFile, destFile, StandardCopyOption.REPLACE_EXISTING);
                                logger.info("Copied reference file: {}", fileName);
                            } catch (IOException e) {
                                logger.error("Failed to copy reference file {}: {}", refFile, e.getMessage());
                            }
                        });
            }
        }
    }

    /**
     * Runs tests inside the Docker container.
     */
    private ReferenceResult runTestsInDocker(Exercise exercise, Path hostExerciseDir, Path tempWorkDir, Instant startTime) {
        // Mount the temp exercise directory as /workspace in the container
        String containerWorkDir = "/workspace";

        // Determine the test command based on available build files
        List<String> command = getTestCommand(exercise);

        logger.info("Running tests in Docker container at {} (mounted from: {})",
                containerWorkDir, tempWorkDir);
        logger.debug("Command: {}", String.join(" ", command));

        try {
            copyFreshTests(exercise, hostExerciseDir, tempWorkDir);
            patchTests(exercise, tempWorkDir);
            DockerClient.ProcessResult result = dockerClient.runCommandWithLimitsAndVolume(
                    null,  // use default image from config
                    containerWorkDir,
                    command,
                    -1,    // use default timeout from config
                    null,  // use default memory from config
                    tempWorkDir.toAbsolutePath().toString(),  // mount temp dir as /workspace
                    System.out::println  // stream output to stdout
            );

            Instant endTime = Instant.now();
            Duration duration = Duration.between(startTime, endTime);

            boolean success = result.isSuccess() && result.exitCode() == 0
                    && !containsTestFailures(result.output());

            if (success) {
                logger.info("Tests passed for exercise: {}. Duration: {}",
                        exercise.getName(), duration);
            } else {
                logger.error("Tests failed for exercise: {}. Exit code: {}, Output: {}",
                        exercise.getName(), result.exitCode(), result.output());
            }

            return ReferenceResult.builder()
                    .exerciseName(exercise.getName())
                    .language(exercise.getLanguage())
                    .success(success)
                    .exitCode(result.exitCode())
                    .output(result.output())
                    .duration(duration)
                    .startTime(startTime)
                    .endTime(endTime)
                    .errorMessage(success ? null : result.output())
                    .build();

        } catch (Exception e) {
            Instant endTime = Instant.now();
            Duration duration = Duration.between(startTime, endTime);

            logger.error("Failed to run tests for exercise {}: {}", exercise.getName(), e.getMessage());

            return ReferenceResult.builder()
                    .exerciseName(exercise.getName())
                    .language(exercise.getLanguage())
                    .success(false)
                    .duration(duration)
                    .startTime(startTime)
                    .endTime(endTime)
                    .errorMessage(e.getMessage())
                    .build();
        }
    }

    /**
     * Determines the appropriate test command based on the build system.
     */
    private List<String> getTestCommand(Exercise exercise) {
        // Check for Maven or Gradle
        Path exerciseDir = Path.of(System.getProperty("user.dir"))
                .resolve("../polyglot-benchmark")
                .resolve(exercise.getLanguage())
                .resolve("exercises")
                .resolve("practice")
                .resolve(exercise.getName());

        if (Files.exists(exerciseDir.resolve("pom.xml"))) {
            // Maven project
            return List.of("mvn", "test", "-q");
        } else if (Files.exists(exerciseDir.resolve("build.gradle"))) {
            return List.of("./gradlew", "test", "--no-daemon", "-q");
        } else if (Files.exists(exerciseDir.resolve("go.mod"))) {
            return List.of("go", "test");
        } else if (Files.exists(exerciseDir.resolve("package.json"))) {
            return List.of("npm", "run", "test");
        } else if (Files.exists(exerciseDir.resolve("CMakeLists.txt"))) {
            return List.of("mkdir", "-p", "build", "&&", "cd", "build", "&&", "cmake", "-DEXERCISM_RUN_ALL_TESTS=1", "-G", "\"Unix Makefiles\"", "..", "&&", "make");
        } else {
            logger.error("Unable to determine test command for exercise {}", exercise.getName());
            return List.of("false");
        }
    }

    /**
     * Checks if the test output contains failure indicators.
     * This catches cases where the test command returns exit code 0 but tests actually failed.
     */
    protected static boolean containsTestFailures(String output) {
        if (output == null || output.isEmpty()) {
            return false;
        }
        // Common failure patterns from various test frameworks
        String[] failurePatterns = {
                "FAILED",
                "FAILURE",
                "BUILD FAILED",
                "BUILD FAILURE",
                "Tests FAILED",
                "Test FAILED",
                "Error:",
                "Exception",
                "failed",
                "FAIL"
        };
        // Check each pattern (case-insensitive)
        String lowerOutput = output.toLowerCase();
        for (String pattern : failurePatterns) {
            if (lowerOutput.contains(pattern.toLowerCase())) {
                return true;
            }
        }
        return false;
    }

    /**
     * Cleans up the temporary directory.
     */
    private void cleanupTempDir(Path tempDir) {
        try {
            if (Files.exists(tempDir)) {
                try (Stream<Path> paths = Files.walk(tempDir)) {
                    paths.sorted(Comparator.reverseOrder())
                            .forEach(path -> {
                                try {
                                    Files.delete(path);
                                } catch (IOException e) {
                                    logger.warn("Failed to delete {}: {}", path, e.getMessage());
                                }
                            });
                }
            }
        } catch (IOException e) {
            logger.warn("Failed to cleanup temp directory {}: {}", tempDir, e.getMessage());
        }
    }

    protected void patchTests(Exercise exercise, Path tempWorkDir) {
        if ("java".equals(exercise.getLanguage())) {
            logger.info("Ensuring no tests are disabled");
            // patch all java files under src/test/java to remove @Disabled annotations
            try {
                Files.walkFileTree(tempWorkDir.resolve("src").resolve("test"), new SimpleFileVisitor<>() {
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
            } catch (IOException e) {
                logger.error("There was an error while removing test annotations.", e);
            }
        }
    }

}
