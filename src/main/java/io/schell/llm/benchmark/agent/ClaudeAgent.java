package io.schell.llm.benchmark.agent;

import io.schell.llm.benchmark.docker.DockerClient;
import io.schell.llm.benchmark.docker.DockerClient.ProcessResult;
import io.schell.llm.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;

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
            ClaudeMessageProcessor processor = new ClaudeMessageProcessor(getOutputConsumer(), isVerbose());
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
            
            // Collect JSONL trace files
            Path claudeJsonLogDirectory = tempWorkDir.resolve(".claude").resolve("projects").resolve("-workspace");
            collectTraceFiles(claudeJsonLogDirectory, resultsDir, exercise, ".jsonl");
            
            // Collect HTML trace files
            Path claudeArchive = tempWorkDir.resolve("claude-archive").resolve("workspace");
            String trace = collectHtmlTraces(claudeArchive, resultsDir, exercise);


            return ReferenceResult.builder()
                    .exerciseName(exercise.getName())
                    .language(exercise.getLanguage())
                    .exitCode(result.exitCode())
                    .output("")  // Don't store raw output - trace is saved separately
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

    /**
     * Collects trace files (JSONL, JSON) from a source directory to the results directory.
     */
    private void collectTraceFiles(Path sourceDir, Path resultsDir, Exercise exercise, String extension) throws IOException {
        if (!Files.isDirectory(sourceDir)) {
            return;
        }

        Files.walkFileTree(sourceDir, new SimpleFileVisitor<>() {
            @Override
            public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) throws IOException {
                String fileName = file.getFileName().toString();
                if (fileName.endsWith(extension)) {
                    String targetName;
                    if (extension.equals(".jsonl")) {
                        // Main agent log - use standard naming: trace_{language}_{exercise}.jsonl
                        targetName = "trace_" + exercise.getLanguage() + "_" + exercise.getName() + ".jsonl";
                    } else {
                        // Sub agent or other JSON logs - keep original naming with prefix
                        targetName = "trace_" + exercise.getLanguage() + "_" + exercise.getName() + "_" + fileName;
                    }
                    Files.copy(file, resultsDir.resolve(targetName), StandardCopyOption.REPLACE_EXISTING);
                }
                return FileVisitResult.CONTINUE;
            }
        });
    }

    /**
     * Collects HTML trace files and returns the first one as a string.
     */
    private String collectHtmlTraces(Path sourceDir, Path resultsDir, Exercise exercise) throws IOException {
        if (!Files.isDirectory(sourceDir)) {
            return "";
        }

        final List<String> htmlTraces = new ArrayList<>();
        
        Files.walkFileTree(sourceDir, new SimpleFileVisitor<>() {
            @Override
            public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) throws IOException {
                String fileName = file.getFileName().toString();
                if (fileName.endsWith(".html") && fileName.contains("page")) {
                    String htmlContent = Files.readString(file);
                    htmlTraces.add(htmlContent);
                    
                    // Save HTML trace to results directory with standard naming
                    String htmlTargetName = "trace_" + exercise.getLanguage() + "_" + exercise.getName() + ".html";
                    Files.copy(file, resultsDir.resolve(htmlTargetName), StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Saved Claude HTML trace: {}", htmlTargetName);
                }
                return FileVisitResult.CONTINUE;
            }
        });

        return htmlTraces.isEmpty() ? "" : htmlTraces.getFirst();
    }
}
