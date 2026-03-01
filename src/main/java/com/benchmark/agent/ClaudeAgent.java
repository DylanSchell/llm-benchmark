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
    protected ReferenceResult runAgent(Exercise exercise, Path hostExerciseDir, Path tempWorkDir, Path resultsDir, String model) throws IOException {
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
            String trace = htmlTraces.isEmpty() ? "" : htmlTraces.getFirst();

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

    public String getName() {
        return "claude";
    }
}
