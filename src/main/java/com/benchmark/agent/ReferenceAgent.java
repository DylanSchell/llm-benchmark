package com.benchmark.agent;

import com.benchmark.docker.DockerClient;
import com.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
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
    private final LanguageHandlerRegistry handlerRegistry;
    private java.util.function.Consumer<String> outputConsumer;

    public ReferenceAgent(DockerClient dockerClient) {
        this.dockerClient = dockerClient;
        this.handlerRegistry = new LanguageHandlerRegistry();
    }

    protected DockerClient getDockerClient() {
        return dockerClient;
    }

    /**
     * Sets an output consumer to receive live output during exercise execution.
     * This is used by the web UI to stream output in real-time via SSE.
     */
    public void setOutputConsumer(java.util.function.Consumer<String> outputConsumer) {
        this.outputConsumer = outputConsumer;
    }

    /**
     * Returns the output consumer, if set.
     */
    protected java.util.function.Consumer<String> getOutputConsumer() {
        return outputConsumer;
    }

    /**
     * Returns the output callback for Docker commands, using outputConsumer if set.
     */
    protected java.util.function.Consumer<String> getOutputCallback() {
        return outputConsumer != null ? outputConsumer : System.out::println;
    }

    /**
     * Prepares the workspace for a language by running setup commands (npm install, uv venv, etc.).
     * This should be called once before running tests to avoid repeating setup.
     *
     * @param exercise The exercise
     * @param tempWorkDir The temporary working directory
     * @throws IOException if preparation fails
     */
    public void prepareWorkspace(Exercise exercise, Path tempWorkDir) throws IOException {
        LanguageHandler handler = handlerRegistry.getHandler(exercise);
        if (handler == null) {
            logger.warn("No handler found for language: {}, skipping workspace preparation", exercise.getLanguage());
            return;
        }

        List<String> prepareCommand = handler.prepareWorkspaceCommand(exercise);
        if (prepareCommand == null || prepareCommand.isEmpty()) {
            logger.debug("No workspace preparation needed for {}", exercise.getLanguage());
            return;
        }

        logger.info("Preparing workspace for {} exercise: {}", exercise.getLanguage(), String.join(" ", prepareCommand));
        
        try {
            DockerClient.ProcessResult result = dockerClient.runCommandWithLimitsAndVolume(
                    null,  // use default image from config
                    "/workspace",
                    prepareCommand,
                    -1,    // use default timeout from config
                    null,  // use default memory from config
                    tempWorkDir.toAbsolutePath().toString(),  // mount temp dir as /workspace
                    getOutputCallback()
            );

            if (!result.isSuccess() || result.exitCode() != 0) {
                throw new IOException("Workspace preparation failed for " + exercise.getLanguage() + 
                        ": " + result.output());
            }
            
            logger.info("Workspace prepared successfully for {}", exercise.getLanguage());
        } catch (Exception e) {
            logger.error("Failed to prepare workspace for {}: {}", exercise.getName(), e.getMessage());
            throw new IOException("Workspace preparation failed", e);
        }
    }

    /**
     * Result of running an exercise with the reference agent.
     */
    public record ReferenceResult(String exerciseName, String language, boolean success, int exitCode, String output,
                                  Duration duration, Instant startTime, Instant endTime, String errorMessage,
                                  String trace, String containerId, String model, String agent) {

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
            private String model;
            private String agent;

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

            public Builder model(String model) {
                this.model = model;
                return this;
            }

            public Builder agent(String agent) {
                this.agent = agent;
                return this;
            }

            public ReferenceResult build() {
                return new ReferenceResult(exerciseName, language, success, exitCode, output,
                        duration, startTime, endTime, errorMessage, trace, containerId, model, agent);
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
    public ReferenceResult runReferenceSolution(Exercise exercise, Path hostExerciseDir, Path resultDir, String model) {
        Instant startTime = Instant.now();
        logger.info("Running reference agent for exercise: {}", exercise.getName());

        try {
            // Create temporary working directory for this exercise
            // Directory is created in project root so it's accessible to Docker container
            Path tempWorkDir = createTempWorkDir(exercise);
            logger.info("Created temporary work directory: {}", tempWorkDir);

            // Copy exercise files to temp directory (excluding reference implementation)
            copyExerciseFiles(exercise, hostExerciseDir, tempWorkDir);

            // prepare workspace ( npm install / etc )
            prepareWorkspace(exercise, tempWorkDir);

            ReferenceResult agentResult = runAgent(exercise, hostExerciseDir, tempWorkDir, resultDir, model);

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
                    .agent(agentResult.agent)
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
                    .agent("reference")
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
    protected ReferenceResult runAgent(Exercise exercise, Path hostExerciseDir, Path tempWorkDir, Path resultDir, String model) throws IOException {
        // Get language handler
        LanguageHandler handler = handlerRegistry.getHandler(exercise);
        if (handler == null) {
            throw new IOException("No handler found for language: " + exercise.getLanguage());
        }

        // Copy reference implementation to source directory
        Instant startTime = Instant.now();
        if (exercise.hasReference()) {
            handler.copyReference(exercise, tempWorkDir);
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
                .agent("reference")
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
     * For C++ exercises, files are placed in a subdirectory named after the exercise.
     */
    private void copyExerciseFiles(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        // Get language handler
        LanguageHandler handler = handlerRegistry.getHandler(exercise);
        if (handler == null) {
            throw new IOException("No handler found for language: " + exercise.getLanguage());
        }

        logger.info("Copying exercise files from {} to {}", sourceDir, destDir);

        // Delegate to handler (which handles C++ subdirectory logic)
        handler.copyExerciseFiles(exercise, sourceDir, destDir);
    }

    protected void copyFreshTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        // Get language handler
        LanguageHandler handler = handlerRegistry.getHandler(exercise);
        if (handler == null) {
            throw new IOException("No handler found for language: " + exercise.getLanguage());
        }

        handler.copyTests(exercise, sourceDir, destDir);
    }

    /**
     * Copies the reference implementation files to the temp directory.
     * Delegates to language-specific handler.
     */
    private void copyReferenceImplementation(Exercise exercise, Path tempDir) throws IOException {
        // Get language handler
        LanguageHandler handler = handlerRegistry.getHandler(exercise);
        if (handler == null) {
            throw new IOException("No handler found for language: " + exercise.getLanguage());
        }

        handler.copyReference(exercise, tempDir);
    }

    /**
     * Runs tests inside the Docker container.
     */
    private ReferenceResult runTestsInDocker(Exercise exercise, Path hostExerciseDir, Path tempWorkDir, Instant startTime) {
        // Get language handler
        LanguageHandler handler = handlerRegistry.getHandler(exercise);

        // Prepare workspace (npm install, uv venv, etc.) - only needed once per language/benchmark run
        try {
            prepareWorkspace(exercise, tempWorkDir);
        } catch (IOException e) {
            logger.error("Failed to prepare workspace for {}: {}", exercise.getName(), e.getMessage());
            // Continue anyway - tests might still work without preparation
        }

        // Determine container work directory (default is /workspace, C++ uses subdirectory)
        String containerWorkDir = (handler != null) 
                ? handler.getContainerWorkDir(exercise)
                : "/workspace";

        // Get test command from handler
        List<String> command = (handler != null)
                ? handler.getTestCommand(exercise)
                : getTestCommand(exercise);

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
                    getOutputCallback()  // stream output to stdout or custom consumer
            );

            Instant endTime = Instant.now();
            Duration duration = Duration.between(startTime, endTime);

            // For Rust, the exit code from cargo test is reliable, so skip containsTestFailures check
            // which can produce false positives from "0 failed" in test output
            boolean skipFailureCheck = "rust".equals(exercise.getLanguage());
            boolean success = result.isSuccess() && result.exitCode() == 0
                    && (skipFailureCheck || !containsTestFailures(result.output()));

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
     * Delegates to language-specific handler when available.
     */
    private List<String> getTestCommand(Exercise exercise) {
        // Get language handler if available
        LanguageHandler handler = handlerRegistry.getHandler(exercise);
        if (handler != null) {
            return handler.getTestCommand(exercise);
        }

        // Fallback for unsupported languages
        Path exerciseDir = Path.of(System.getProperty("user.dir"))
                .resolve("../polyglot-benchmark")
                .resolve(exercise.getLanguage())
                .resolve("exercises")
                .resolve("practice")
                .resolve(exercise.getName());

        if (Files.exists(exerciseDir.resolve("pom.xml"))) {
            return List.of("mvn", "test", "-q");
        } else if (Files.exists(exerciseDir.resolve("build.gradle"))) {
            return List.of("/workspace/gradlew", "test", "--no-daemon", "-q");
        } else if (Files.exists(exerciseDir.resolve("go.mod"))) {
            return List.of("go", "test");
        } else if (Files.exists(exerciseDir.resolve("package.json"))) {
            return List.of("npm", "run", "test");
        } else if (Files.exists(exerciseDir.resolve("Cargo.toml"))) {
            return List.of("cargo", "test");
        } else if (Files.exists(exerciseDir.resolve("CMakeLists.txt"))) {
            return List.of("sh", "-c", "mkdir -p build && cd build && cmake -DEXERCISM_RUN_ALL_TESTS=1 -G \"Unix Makefiles\" .. && make");
        } else {
            logger.error("Unable to determine test command for exercise {}", exercise.getName());
            return List.of("false");
        }
    }

    /**
     * Checks if the test output contains failure indicators.
     * This catches cases where the test command returns exit code 0 but tests actually failed.
     * Note: This is primarily used for Java/Gradle/Maven output. For Rust, the exit code
     * from cargo test is reliable, so this check is skipped.
     */
    protected static boolean containsTestFailures(String output) {
        if (output == null || output.isEmpty()) {
            return false;
        }
        // Common failure patterns from Java test frameworks (Gradle/Maven)
        String[] failurePatterns = {
                "BUILD FAILED",
                "BUILD FAILURE",
                "Tests FAILED",
                "Test FAILED",
                "FAILED",
                "FAILURE"
        };
        for (String pattern : failurePatterns) {
            if (output.contains(pattern)) {
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

    protected void patchTests(Exercise exercise, Path tempWorkDir) throws IOException {
        // Get language handler
        LanguageHandler handler = handlerRegistry.getHandler(exercise);
        if (handler == null) {
            logger.warn("No handler found for language: {}, skipping test patching", exercise.getLanguage());
            return;
        }

        handler.patchTests(tempWorkDir);
    }

    /**
     * Creates a prompt for AI agents to solve the exercise.
     * This method is shared by all agent implementations (ClaudeAgent, PiAgent).
     */
    protected String createExercisePrompt(Exercise exercise, Path tempWorkDir) throws IOException {
        StringBuilder prompt = new StringBuilder();
        Path instructionsPath = tempWorkDir.resolve(".docs").resolve("instructions.md");
        if (Files.exists(instructionsPath)) {
            prompt.append(Files.readString(instructionsPath));
        } else {
            prompt.append("Please solve the following programming exercise.\n\n");
            prompt.append("Exercise: ").append(exercise.getName()).append("\n");
            prompt.append("Language: ").append(exercise.getLanguage()).append("\n\n");
            prompt.append("Instructions:\n");
            prompt.append("1. Implement the solution in the source files only, do not touch the test files.\n");
            prompt.append("2. Run the tests to verify your solution\n\n");
            prompt.append("3. The tests are validated to be correct, never assume the test to be wrong!\n\n");
            prompt.append("4. Do not run tests in the background, run them synchronously in the foreground.\n");
        }

        for (Path testPath : exercise.getTestPath()) {
            if (exercise.getTestPath() != null && Files.exists(testPath)) {
                String needle = "../polyglot-benchmark/" + exercise.getLanguage() + "/exercises/practice/" + exercise.getName();
                String fixedTestPath = exercise.getTestPath().toString().replaceAll(needle, "/workspace");
                prompt.append("Test file location: ").append(fixedTestPath).append("\n");
            }
        }

        prompt.append("\nImplement the solution directly, do not ask me to review.\n");

        // Add language-specific instructions
        if ("java".equals(exercise.getLanguage())) {
            prompt.append("\nDo not stop working until you have executed the test suite (./gradlew test --no-daemon) and you have validated that the tests succeed!\n");
        } else if ("javascript".equals(exercise.getLanguage())) {
            prompt.append("\nRun tests with: npm install && npm run test\n");
            prompt.append("This exercise uses Jest as the test framework.\n");
        } else if ("python".equals(exercise.getLanguage())) {
            prompt.append("\nUse uv to create a virtual environment and run tests:\n");
            prompt.append("1. Create venv: uv venv (or use existing .venv)\n");
            prompt.append("2. Activate: . .venv/bin/activate\n");
            prompt.append("3. Install pytest: uv pip install pytest\n");
            prompt.append("4. Run tests: pytest\n");
        } else if ("rust".equals(exercise.getLanguage())) {
            prompt.append("\nRun tests with: cargo test\n");
            prompt.append("Use cargo test to validate all tests succeed.\n");
        } else if ("cpp".equals(exercise.getLanguage())) {
            prompt.append("\nBuild with: mkdir -p build && cd build && cmake -DEXERCISM_RUN_ALL_TESTS=1 -G \"Unix Makefiles\" .. && make\n");
            prompt.append("Run tests with: ./build/tests or the test executable in the build directory.\n");
        }

        prompt.append("<important>Check that no tests are skipped, enable any tests that shows as skipped in the test results! Any skipped tests will result in failure!</important>\n");
        return prompt.toString();
    }

    public String getName() {
        return "reference";
    }
}
