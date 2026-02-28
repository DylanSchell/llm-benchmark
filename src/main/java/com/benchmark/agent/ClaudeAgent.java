package com.benchmark.agent;

import com.benchmark.docker.DockerClient;
import com.benchmark.docker.DockerClient.ProcessResult;
import com.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

public class ClaudeAgent extends ReferenceAgent {
    private static final Logger logger = LoggerFactory.getLogger(ClaudeAgent.class);

    public ClaudeAgent(DockerClient dockerClient) {
        super(dockerClient);
    }

    @Override
    protected ReferenceResult runAgent(Exercise exercise, Path hostExerciseDir, Path tempWorkDir, Path resultsDir) throws IOException {
        Instant startTime = Instant.now();

        try {
            logger.info("Starting exercise: {} at {}", exercise.getName(), startTime);
            ClaudeMessageProcessor processor = new ClaudeMessageProcessor(getOutputConsumer());
            // Create exercise prompt for Claude Code
            String prompt = createExercisePrompt(exercise, tempWorkDir);
            patchTests(exercise, tempWorkDir);
            // ["Task","TaskOutput","Bash","Glob","Grep","ExitPlanMode","Read","Edit","Write","NotebookEdit","WebFetch","TodoWrite","WebSearch","KillShell","AskUserQuestion","Skill","EnterPlanMode","MCPSearch"]
            List<String> command = List.of("claude",
                    "--allow-dangerously-skip-permissions",
                    "--dangerously-skip-permissions",
                    "--print",
                    "--tools", "Task,TaskOutput,Bash,Glob,Grep,Read,Edit,Write,NotebookEdit,WebFetch,TodoWrite,WebSearch,KillShell,ExitPlanMode",
                    "--permission-mode", "bypassPermissions",
                    "--verbose",
                    "--output-format", "stream-json",
                    "--include-partial-messages",
                    prompt);
            ProcessResult result = getDockerClient().runCommandWithLimitsAndVolume(
                    null,  // use default image from config
                    "/workspace",
                    command,
                    -1,    // use default timeout from config
                    null,  // use default memory from config
                    tempWorkDir.toAbsolutePath().toString(),  // mount temp dir as /workspace
                    processor  // stream output to stdout
            );
            Instant endTime = Instant.now();
            Duration duration = Duration.between(startTime, endTime);
            boolean success = result.isSuccess() && result.exitCode() == 0;

            if (!success) {
                logger.error("Exercise failed: {}. Exit code: {}, Output: {}",
                        exercise.getName(), result.exitCode(), result.output());
            } else {
                logger.info("Exercise completed successfully: {}. Duration: {}",
                        exercise.getName(), duration);
            }
            logger.info("Attempting to render Claude execution trace");
            List<String> archiveCommand = List.of("/home/runner/.local/bin/claude-code-transcripts", "all", "--include-agents");
            ProcessResult archiveResult = getDockerClient().runCommandWithLimitsAndVolume(
                    null,
                    "/workspace",
                    archiveCommand,
                    -1,
                    null,
                    tempWorkDir.toAbsolutePath().toString(),
                    System.out::println
            );
            Path claudeJsonLogDirectory = tempWorkDir.resolve(".claude").resolve("projects").resolve("-workspace");
            if (Files.isDirectory(claudeJsonLogDirectory)) {
                Files.walkFileTree(claudeJsonLogDirectory, new SimpleFileVisitor<>() {
                    @Override
                    public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) throws IOException {
                        if (file.getFileName().toString().startsWith("agent")) {
                            // sub agent log
                            // log_claude_java_affine-cipher_agent-xxxx.jsonl
                            Files.copy(file, resultsDir.resolve("log_claude_"+exercise.getLanguage()+"_"+exercise.getName()+"_"+file.getFileName()));
                        } else {
                            // main agent log
                            // log_claude_java_affine-cipher.jsonl
                            Files.copy(file, resultsDir.resolve("log_claude_"+exercise.getLanguage()+"_"+exercise.getName()+".jsonl"));
                        }
                        return FileVisitResult.CONTINUE;
                    }
                });
            }
            // attach trace to result
            Path claudeArchive = tempWorkDir.resolve("claude-archive").resolve("workspace");
            final List<String> htmlTraces = new ArrayList<>();
            if (Files.isDirectory(claudeArchive)) {
                try (Stream<Path> stream = Files.find(claudeArchive, Integer.MAX_VALUE,
                        (p, a) -> true)) {
                    stream.forEach(p -> {
                        try {

                            if (p.toString().endsWith(".html") && p.toString().contains("page")) {
                                htmlTraces.add(Files.readString(p));
                            }
                        } catch (IOException e) {
                            throw new RuntimeException(e);
                        }
                    });
                }
            }
            String trace = htmlTraces.isEmpty() ? "" : htmlTraces.get(0);

            return ReferenceResult.builder()
                    .exerciseName(exercise.getName())
                    .language(exercise.getLanguage())
                    .exitCode(result.exitCode())
                    .output(result.output())
                    .success(success)
                    .startTime(startTime)
                    .endTime(endTime)
                    .duration(duration)
                    .trace(trace)
                    .agent("claude")
                    .build();
        } catch (Exception e) {
            Instant endTime = Instant.now();
            Duration duration = Duration.between(startTime, endTime);

            logger.error("Exercise execution failed: {}", e.getMessage(), e);
            return ReferenceResult.builder()
                    .exitCode(-1)
                    .errorMessage(e.getMessage())
                    .success(false)
                    .startTime(startTime)
                    .endTime(endTime)
                    .duration(duration)
                    .output(e.getMessage())
                    .language(exercise.getLanguage())
                    .exerciseName(exercise.getName())
                    .agent("claude")
                    .build();
        }
    }

    /**
     * Creates a prompt for Claude Code to solve the exercise.
     */
    private String createExercisePrompt(Exercise exercise, Path tempWorkDir) throws IOException {
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
            prompt.append("4. Do not run tests in the background, run them synchronously in the forground, so you do not need to poll for results\n");
        }
        for (Path testPath : exercise.getTestPath()) {
            if (exercise.getTestPath() != null && Files.exists(testPath)) {
                String needle = "../polyglot-benchmark/" + exercise.getLanguage() + "/exercises/practice/" + exercise.getName();
                String fixedTestPath = exercise.getTestPath().toString().replaceAll(needle, "/workspace");
                prompt.append("Test file location: ").append(fixedTestPath).append("\n");
            }
        }
        prompt.append("\nImplement the solution directly, do not ask me to review.\n");
        if ("java".equals(exercise.getLanguage())) {
            prompt.append("\nDo not stop working until you have executed the test suite (./gradlew test --no-daemon) and you have validated that the tests succeed!\n");
        } else if ("javascript".equals(exercise.getLanguage())) {
            prompt.append("\nRun tests with: npm install && npm run test\n");
            prompt.append("This exercise uses Jest as the test framework.\n");
        } else if ("python".equals(exercise.getLanguage())) {
            prompt.append("\nUse uv to create a virtual environment and run tests:\n");
            prompt.append("1. Create venv: uv venv (or use existing .venv)\n");
            prompt.append("2. Activate: source .venv/bin/activate\n");
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
        return "claude";
    }
}
