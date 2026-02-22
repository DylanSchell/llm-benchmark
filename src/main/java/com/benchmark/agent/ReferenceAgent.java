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
        if (exercise.hasReference()) {
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
     * For C++ exercises, files are placed in a subdirectory named after the exercise.
     */
    private void copyExerciseFiles(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        logger.info("Copying exercise files from {} to {}", sourceDir, destDir);

        // For C++, create a subdirectory named after the exercise
        final Path actualDestDir;
        if ("cpp".equals(exercise.getLanguage())) {
            actualDestDir = destDir.resolve(exercise.getName());
            Files.createDirectories(actualDestDir);
        } else {
            actualDestDir = destDir;
        }

        try (Stream<Path> paths = Files.walk(sourceDir)) {
            paths.forEach(sourcePath -> {
                try {
                    Path relativePath = sourceDir.relativize(sourcePath);

                    // Skip reference implementation directory
                    if (relativePath.toString().contains(".meta/")) {
                        logger.debug("Skipping reference file: {}", relativePath);
                        return;
                    }

                    Path destPath = actualDestDir.resolve(relativePath);

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
        // For C++, create a subdirectory named after the exercise
        final Path actualDestDir;
        if ("cpp".equals(exercise.getLanguage())) {
            actualDestDir = destDir.resolve(exercise.getName());
            Files.createDirectories(actualDestDir);
        } else {
            actualDestDir = destDir;
        }
        if ("java".equals(exercise.getLanguage())) {
            try (Stream<Path> paths = Files.walk(sourceDir)) {
                paths.forEach(sourcePath -> {
                    try {
                        Path relativePath = sourceDir.relativize(sourcePath);
                        Path destPath = actualDestDir.resolve(relativePath);
                        if (Files.isDirectory(sourcePath)) {
                            Files.createDirectories(destPath);
                        } else {
                            // Skip reference implementation directory
                            if (relativePath.toString().contains("src/test/java") && relativePath.endsWith(".java")) {
                                logger.info("Copying fresh tests from {} to {}", sourceDir, actualDestDir);
                                Files.copy(sourcePath, destPath, StandardCopyOption.REPLACE_EXISTING);
                            }
                        }
                    } catch (IOException e) {
                        logger.error("Failed to copy file {}: {}", sourcePath, e.getMessage());
                    }
                });
            }
        } else if ("go".equals(exercise.getLanguage())) {
            Iterable<Path> testPath = exercise.getTestPath();
            for (Path refPath : testPath) {
                try {
                    String fileName = refPath.getFileName().toString();
                    Path destFile = actualDestDir.resolve(fileName);
                    Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied test file: {}", fileName);
                } catch (IOException e) {
                    logger.error("Failed to copy test file {}: {}", refPath, e.getMessage());
                }
            }
        } else if ("javascript".equals(exercise.getLanguage())) {
            Iterable<Path> testPath = exercise.getTestPath();
            for (Path refPath : testPath) {
                try {
                    String fileName = refPath.getFileName().toString();
                    Path destFile = actualDestDir.resolve(fileName);
                    Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied test file: {}", fileName);
                } catch (IOException e) {
                    logger.error("Failed to copy test file {}: {}", refPath, e.getMessage());
                }
            }
        } else if ("python".equals(exercise.getLanguage())) {
            Iterable<Path> testPath = exercise.getTestPath();
            for (Path refPath : testPath) {
                try {
                    String fileName = refPath.getFileName().toString();
                    Path destFile = actualDestDir.resolve(fileName);
                    Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied test file: {}", fileName);
                } catch (IOException e) {
                    logger.error("Failed to copy test file {}: {}", refPath, e.getMessage());
                }
            }
        } else if ("rust".equals(exercise.getLanguage())) {
            // For Rust, copy from tests/ directory
            Path testsDir = sourceDir.resolve("tests");
            if (Files.exists(testsDir)) {
                try (Stream<Path> paths = Files.walk(testsDir)) {
                    paths.forEach(sourcePath -> {
                        try {
                            if (Files.isRegularFile(sourcePath)) {
                                Path relativePath = testsDir.relativize(sourcePath);
                                Path destPath = actualDestDir.resolve("tests").resolve(relativePath);
                                Files.createDirectories(destPath.getParent());
                                Files.copy(sourcePath, destPath, StandardCopyOption.REPLACE_EXISTING);
                                logger.info("Copied Rust test file: {}", relativePath);
                            }
                        } catch (IOException e) {
                            logger.error("Failed to copy Rust test file {}: {}", sourcePath, e.getMessage());
                        }
                    });
                }
            }
        } else if ("cpp".equals(exercise.getLanguage())) {
            Iterable<Path> testPath = exercise.getTestPath();
            for (Path refPath : testPath) {
                try {
                    String fileName = refPath.getFileName().toString();
                    Path destFile = actualDestDir.resolve(fileName);
                    Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied C++ test file: {}", fileName);
                } catch (IOException e) {
                    logger.error("Failed to copy C++ test file {}: {}", refPath, e.getMessage());
                }
            }
        }
    }

    /**
     * Copies the reference implementation Java files to the main source directory.
     */
    private void copyReferenceImplementation(Exercise exercise, Path tempDir) throws IOException {
        if ("cpp".equals(exercise.getLanguage())) {
            tempDir = tempDir.resolve(exercise.getName());
            Files.createDirectories(tempDir);
        }
        Iterable<Path> refDirs = exercise.getReferencePath();
        if (refDirs == null) {
            return;
        }
        for (Path refPath : refDirs) {
            if (refPath == null || !Files.exists(refPath)) {
                logger.warn("Reference path not found for: {}", exercise.getName());
                return;
            }
            if (exercise.getLanguage().equals("java")) {
                Path mainSrcDir = tempDir.resolve("src/main/java");
                Files.createDirectories(mainSrcDir);

                logger.info("Copying reference implementation from {} to {}", refPath, mainSrcDir);

                try (Stream<Path> paths = Files.walk(refPath)) {
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
                // overwrite with "sample"
                for(Path examplePath: exercise.getExamples()) {
                    try {
                        String fileName = examplePath.getFileName().toString();
                        // copy this over the "reference"?
                        Path destFile = tempDir.resolve("src/main/java").resolve(fileName);
                        //Path destFile = tempDir.resolve(destFileName);
                        Files.copy(examplePath, destFile, StandardCopyOption.REPLACE_EXISTING);
                        logger.info("Copied reference file: {}", fileName);
                    } catch (IOException e) {
                        logger.error("Failed to copy reference file {}: {}", examplePath, e.getMessage());
                    }
                }
            } else if (exercise.getLanguage().equals("go")) {
                try {
                    String fileName = refPath.getFileName().toString();
                    Path destFile = tempDir.resolve(fileName);
                    Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied reference file: {}", fileName);
                } catch (IOException e) {
                    logger.error("Failed to copy reference file {}: {}", refPath, e.getMessage());
                }
            } else if ("javascript".equals(exercise.getLanguage())) {
                try {
                    String fileName = refPath.getFileName().toString();
                    Path destFile = tempDir.resolve(fileName);
                    Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied JavaScript reference file: {}", fileName);
                } catch (IOException e) {
                    logger.error("Failed to copy JavaScript reference file {}: {}", refPath, e.getMessage());
                }
            } else if ("python".equals(exercise.getLanguage())) {
                try {
                    String fileName = refPath.getFileName().toString();
                    Path destFile = tempDir.resolve(fileName);
                    Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied Python reference file: {}", fileName);
                } catch (IOException e) {
                    logger.error("Failed to copy Python reference file {}: {}", refPath, e.getMessage());
                }
            } else if ("rust".equals(exercise.getLanguage())) {
                try {
                    // For Rust, maintain directory structure: src/lib.rs -> src/lib.rs, Cargo.toml -> Cargo.toml
                    String fileName = refPath.getFileName().toString();
                    Path destFile;
                    if ("src".equals(refPath.getParent().getFileName().toString())) {
                        // File is in src/ directory, maintain that structure
                        Path srcDir = tempDir.resolve("src");
                        Files.createDirectories(srcDir);
                        destFile = srcDir.resolve(fileName);
                    } else {
                        // File is at root level (e.g., Cargo.toml)
                        destFile = tempDir.resolve(fileName);
                    }
                    Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied Rust reference file: {}", fileName);
                } catch (IOException e) {
                    logger.error("Failed to copy Rust reference file {}: {}", refPath, e.getMessage());
                }
            } else if ("cpp".equals(exercise.getLanguage())) {
                // For C++, skip copying the stub files from getReferencePath()
                // The example solution files will be copied below and renamed appropriately
                logger.debug("Skipping C++ stub file: {}", refPath.getFileName());
            }
        }
        for(Path examplePath: exercise.getExamples()) {
            try {
                String fileName = examplePath.getFileName().toString();
                // Determine destination based on language
                Path destFile;
                if ("rust".equals(exercise.getLanguage())) {
                    // For Rust examples:
                    // - .meta/example.rs -> src/lib.rs
                    String destFileName = exercise.getReferencePath().iterator().next().getFileName().toString();
                    Path referencePath = exercise.getReferencePath().iterator().next();
                    if ("src".equals(referencePath.getParent().getFileName().toString())) {
                        // Reference is in src/, so example should go to src/
                        Path srcDir = tempDir.resolve("src");
                        Files.createDirectories(srcDir);
                        destFile = srcDir.resolve(destFileName);
                    } else {
                        destFile = tempDir.resolve(destFileName);
                    }

                } else if ("cpp".equals(exercise.getLanguage())) {
                    // For C++, rename example files to match the stub file names
                    // e.g., example.cpp -> all_your_base.cpp, example.h -> all_your_base.h
                    String exampleExtension = fileName.substring(fileName.lastIndexOf('.'));

                    // Find the solution file with the same extension as this example
                    String stubFileName = null;
                    for (Path refPath : exercise.getReferencePath()) {
                        String refExt = refPath.getFileName().toString().substring(refPath.getFileName().toString().lastIndexOf('.'));
                        if (refExt.equals(exampleExtension)) {
                            stubFileName = refPath.getFileName().toString();
                            break;
                        }
                    }

                    if (stubFileName != null) {
                        destFile = tempDir.resolve(stubFileName);
                        Files.copy(examplePath, destFile, StandardCopyOption.REPLACE_EXISTING);
                        logger.info("Copied C++ example file as: {}", stubFileName);
                    } else {
                        logger.warn("Could not find matching solution file for C++ example: {}", fileName);
                    }
                    continue; // Skip the regular copy logic below

                } else {
                    String destFileName = exercise.getReferencePath().iterator().next().getFileName().toString();
                    destFile = tempDir.resolve(destFileName);
                }
                Files.copy(examplePath, destFile, StandardCopyOption.REPLACE_EXISTING);
                logger.info("Copied reference file: {}", fileName);
            } catch (IOException e) {
                logger.error("Failed to copy reference file {}: {}", examplePath, e.getMessage());
            }
        }
        if ( "rust".equals(exercise.getLanguage())) {
            var cargoFile = exercise.getExercisePath().resolve(".meta").resolve("Cargo-example.toml");
            if (Files.exists(cargoFile)) {
                var destFile = tempDir.resolve("Cargo.toml");
                Files.copy(cargoFile, destFile, StandardCopyOption.REPLACE_EXISTING);
                logger.info("Copied Rust reference file: {}", cargoFile);
            }
        }
    }

    /**
     * Runs tests inside the Docker container.
     */
    private ReferenceResult runTestsInDocker(Exercise exercise, Path hostExerciseDir, Path tempWorkDir, Instant startTime) {
        // Mount the temp exercise directory as /workspace in the container
        // For C++, files are in a subdirectory named after the exercise
        String containerWorkDir;
        if ("cpp".equals(exercise.getLanguage())) {
            containerWorkDir = "/workspace/" + exercise.getName();
        } else {
            containerWorkDir = "/workspace";
        }

        List<String> command;
        if ("javascript".equals(exercise.getLanguage())) {
            // For JavaScript, first run npm install to get dependencies, then run tests
            command = List.of("sh", "-c", "npm install && npm run test");
        } else if ("python".equals(exercise.getLanguage())) {
            // For Python, use uv to create a venv and install pytest
            // First check if .venv exists, if so activate it, otherwise create new venv
            command = List.of("sh", "-c",
                "if [ -d \".venv\" ]; then source .venv/bin/activate; else uv venv && source .venv/bin/activate; fi && " +
                "uv pip install -q pytest && pytest");
        } else {
            // Determine the test command based on available build files
            command = getTestCommand(exercise);
        }

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
        } else if ("rust".equals(exercise.getLanguage())) {
            logger.info("Ensuring no Rust tests are ignored (removing #[ignore] annotations)");
            // patch all rust test files under tests/ to remove #[ignore] annotations
            Path testsDir = tempWorkDir.resolve("tests");
            if (Files.exists(testsDir)) {
                try {
                    Files.walkFileTree(testsDir, new SimpleFileVisitor<>() {
                        @Override
                        public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) {
                            if (file.toString().endsWith(".rs")) {
                                try {
                                    String testCode = Files.readString(file);
                                    // Remove #[ignore] annotations (with or without arguments)
                                    String updatedCode = testCode.replaceAll("#\\[ignore]", "");
                                    updatedCode = updatedCode.replaceAll("#\\[ignore[(].*[)]", "");
                                    Files.writeString(file, updatedCode);
                                } catch (IOException e) {
                                    logger.error("Error reading file {}", file);
                                }
                            }
                            return FileVisitResult.CONTINUE;
                        }
                    });
                } catch (IOException e) {
                    logger.error("There was an error while removing #[ignore] annotations.", e);
                }
            }
        }
    }

}
