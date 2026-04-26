package com.benchmark.agent;

import com.benchmark.docker.DockerClient;
import com.benchmark.docker.DockerClient.ProcessResult;
import com.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.stream.Stream;

/**
 * Pi agent that uses the pi coding agent to solve exercises.
 * Pi is invoked inside a Docker container with appropriate configuration
 * for the model endpoint.
 */
public class PiAgent extends ReferenceAgent {
    private static final Logger logger = LoggerFactory.getLogger(PiAgent.class);

    public PiAgent(DockerClient dockerClient) {
        super(dockerClient);
    }

    @Override
    protected ReferenceResult runAgent(Exercise exercise, Path hostExerciseDir, Path tempWorkDir, Path resultsDir, String model) {
        Instant startTime = Instant.now();

        try {
            logger.info("Starting exercise with Pi agent: {} at {}", exercise.getName(), startTime);
            PiMessageProcessor processor = new PiMessageProcessor(getOutputConsumer());
            // Create models.json configuration for pi inside the container
            createModelsJson(tempWorkDir);
            installPiExtensions(tempWorkDir);
            // Create exercise prompt for pi
            String prompt = createExercisePrompt(exercise, tempWorkDir);
            patchTests(exercise, tempWorkDir);

            // Build pi command with JSON output mode
            List<String> command = buildPiCommand(prompt, model);

            ProcessResult result = getDockerClient().runCommandWithLimitsAndVolume(
                    null,  // use default image from config
                    "/workspace",
                    command,
                    -1,    // use default timeout from config
                    null,  // use default memory from config
                    tempWorkDir.toAbsolutePath().toString(),  // mount temp dir as /workspace
                    processor,  // stream output to stdout
                    true  // enable .pi volume mount for session data
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

            // Collect trace from pi session files (saves JSONL and HTML to resultsDir)
            String trace = collectPiTrace(tempWorkDir, resultsDir, exercise);

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
                    .agent("pi")
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
                    .output("")  // Don't store raw output - trace is saved separately
                    .language(exercise.getLanguage())
                    .exerciseName(exercise.getName())
                    .agent("pi")
                    .build();
        }
    }

    private void installPiExtensions(Path tempWorkDir) throws IOException {
        Path piExtensionDir = tempWorkDir.resolve(".pi")
                .resolve("agent")
                .resolve("extensions")
                .resolve("bash-timeout");
        Files.createDirectories(piExtensionDir);
        Path targetPath = piExtensionDir.resolve("index.ts");
        String content = new String(
                Objects.requireNonNull(getClass().getClassLoader()
                                .getResourceAsStream("bash-timeout.ts"))
                        .readAllBytes(),
                StandardCharsets.UTF_8
        );
        Files.writeString(targetPath,content);
    }

    /**
     * Creates the models.json configuration file for pi inside the working directory.
     * This configures pi to use the OpenAI endpoint (which works with Anthropic-compatible APIs).
     */
    private void createModelsJson(Path tempWorkDir) throws IOException {
        // Create .pi/agent directory structure
        Path piAgentDir = tempWorkDir.resolve(".pi").resolve("agent");
        Files.createDirectories(piAgentDir);

        // Read environment configuration from Docker config (same as what's passed to container)
        Map<String, String> envVars = getDockerClient().getConfig().getEnvironmentMap();

        // Use OpenAI endpoint (derived from ANTHROPIC_BASE_URL with /v1 suffix)
        String baseUrl = envVars.getOrDefault("OPENAI_BASE_URL", "http://host.docker.internal:8000/v1");

        String apiKey = envVars.getOrDefault("OPENAI_API_KEY", "api-key");
        if (apiKey.isEmpty()) {
            apiKey = envVars.getOrDefault("ANTHROPIC_AUTH_TOKEN", "placeholder-key");
        }

        String model = envVars.getOrDefault("ANTHROPIC_MODEL", "claude-sonnet-4");

        // Create models.json configuration using OpenAI provider
        // Pi prefers OpenAI endpoint which is compatible with Anthropic's API
        String modelsJson = String.format(
                "{" +
                        "  \"providers\": {" +
                        "    \"openai\": {" +
                        "      \"baseUrl\": \"%s\"," +
                        "      \"apiKey\": \"%s\"," +
                        "      \"api\": \"openai-completions\"," +
                        "      \"models\": [" +
                        "        { \"id\": \"%s\" }" +
                        "      ]" +
                        "    }" +
                        "  }" +
                        "}",
                escapeJson(baseUrl), escapeJson(apiKey), escapeJson(model)
        );

        Path modelsFile = piAgentDir.resolve("models.json");
        Files.writeString(modelsFile, modelsJson);
        logger.debug("Created models.json at: {} with OpenAI provider", modelsFile);
    }

    /**
     * Escapes special characters for JSON string values.
     */
    private String escapeJson(String value) {
        if (value == null) return "";
        return value.replace("\\", "\\\\")
                .replace("\"", "\\\"")
                .replace("\n", "\\n")
                .replace("\r", "\\r")
                .replace("\t", "\\t");
    }

    /**
     * Builds the command line arguments for invoking pi.
     */
    private List<String> buildPiCommand(String prompt, String model) {
        List<String> command = new ArrayList<>();
        command.add("pi");
        command.add("--mode");
        command.add("json");
        command.add("--tools");
        command.add("read,bash,edit,write,grep,find,ls");
        // Don't use --no-session - we want to capture the session trace
        command.add("--provider");
        command.add("openai");
        command.add("--model");
        command.add(model);
        command.add(prompt);
        return command;
    }

    /**
     * Collects trace information from pi session files and exports to HTML.
     */
    private String collectPiTrace(Path tempWorkDir, Path resultsDir, Exercise exercise) throws IOException {
        // Pi stores sessions in ~/.pi/agent/sessions by default inside the container
        // The .pi directory is mounted at tempWorkDir/.pi on the host
        // Inside container: /home/runner/.pi/agent/sessions
        // On host (mounted): tempWorkDir/.pi/agent/sessions

        Path piSessionsDir = tempWorkDir.resolve(".pi").resolve("agent").resolve("sessions");

        if (!Files.exists(piSessionsDir)) {
            logger.warn("No pi sessions directory found at: {}", piSessionsDir);
            logger.info("Contents of .pi directory: {}", listDirectoryContents(tempWorkDir.resolve(".pi")));
            return "";
        }

        logger.info("Found pi sessions directory at: {}", piSessionsDir);

        List<String> htmlTraces = new ArrayList<>();
        List<Path> jsonlFiles = new ArrayList<>();

        // Look for JSON session files and copy them
        List<Path> sessionFiles;
        try (Stream<Path> paths = Files.walk(piSessionsDir)) {
            sessionFiles = paths.filter(Files::isRegularFile).toList();
        }

        logger.info("Found {} session files in total", sessionFiles.size());

        for (Path sessionFile : sessionFiles) {
            String fileName = sessionFile.getFileName().toString();
            try {
                logger.info("Processing pi session file: {}", fileName);
                if (fileName.endsWith(".jsonl")) {
                    // Copy JSONL log files to results directory with standard naming
                    String targetName = "trace_" + exercise.getLanguage() + "_" + exercise.getName() + ".jsonl";
                    Files.copy(sessionFile, resultsDir.resolve(targetName), StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied pi JSONL log file: {}", targetName);
                    jsonlFiles.add(sessionFile);
                } else if (fileName.endsWith(".json")) {
                    String targetName = "log_pi_" + exercise.getLanguage() + "_" + exercise.getName() + "_" + fileName;
                    Files.copy(sessionFile, resultsDir.resolve(targetName), StandardCopyOption.REPLACE_EXISTING);
                    logger.info("Copied pi JSON log file: {}", targetName);
                } else if (fileName.endsWith(".html")) {
                    // Found an HTML trace
                    String htmlContent = Files.readString(sessionFile);
                    htmlTraces.add(htmlContent);
                    logger.info("Found HTML trace with {} chars", htmlContent.length());
                }
            } catch (IOException e) {
                logger.warn("Failed to process session file {}: {}", sessionFile, e.getMessage());
            }
        }

        // Export JSONL files to HTML using pi --export command
        if (!jsonlFiles.isEmpty()) {
            logger.info("Exporting {} JSONL trace file(s) to HTML", jsonlFiles.size());

            // Base path for .pi directory on host and in container
            Path hostPiBase = tempWorkDir.resolve(".pi");
            String containerPiBase = "/home/runner/.pi";

            for (Path jsonlFile : jsonlFiles) {
                try {
                    String baseName = jsonlFile.getFileName().toString().replace(".jsonl", "");

                    // Relativize the discovered path against tempWorkDir/.pi to get relative path
                    Path relativePath = hostPiBase.relativize(jsonlFile);
                    logger.info("Relative path from .pi: {}", relativePath);

                    // Construct container paths by prepending /home/runner/.pi
                    Path containerJsonlPath = Paths.get(containerPiBase).resolve(relativePath);
                    Path containerHtmlPath = containerJsonlPath.getParent().resolve(baseName + ".html");

                    logger.info("Exporting from {} to {} (container paths)",
                            containerJsonlPath, containerHtmlPath);

                    // Run: pi --export <jsonl_file> <html_file> using CONTAINER paths
                    List<String> exportCommand = List.of("pi", "--export",
                            containerJsonlPath.toString(),
                            containerHtmlPath.toString());

                    logger.info("Running export command: {}", String.join(" ", exportCommand));

                    ProcessResult exportResult = getDockerClient().runCommandWithLimitsAndVolume(
                            null,
                            "/workspace",
                            exportCommand,
                            60,  // 60 second timeout for export
                            null,
                            tempWorkDir.toAbsolutePath().toString(),
                            line -> logger.info("[Export] {}", line)  // Log all output, not just debug
                    );

                    logger.info("Export completed. Success: {}, Exit code: {}",
                            exportResult.isSuccess(), exportResult.exitCode());
                    if (!exportResult.output().isEmpty()) {
                        String preview = exportResult.output().substring(0, Math.min(500, exportResult.output().length()));
                        logger.info("Export output preview: {}", preview);
                    }

                    // Check if HTML file exists on host side using same relative path
                    Path hostHtmlFile = hostPiBase.resolve(relativePath.getParent()).resolve(baseName + ".html");
                    logger.info("Looking for HTML at: {}", hostHtmlFile);

                    if (Files.exists(hostHtmlFile)) {
                        logger.info("Found HTML file at: {}", hostHtmlFile);
                        String htmlContent = Files.readString(hostHtmlFile);
                        htmlTraces.add(htmlContent);
                        logger.info("Read HTML trace with {} chars", htmlContent.length());

                        // Also copy to results directory for persistence with standard naming
                        String htmlTargetName = "trace_" + exercise.getLanguage() + "_" + exercise.getName() + ".html";
                        Files.copy(hostHtmlFile, resultsDir.resolve(htmlTargetName),
                                StandardCopyOption.REPLACE_EXISTING);
                        logger.info("Copied HTML trace to results directory: {}", htmlTargetName);
                    } else {
                        logger.warn("HTML file not found at: {}", hostHtmlFile);
                        // List what's actually in the sessions directory
                        Path sessionsDir = hostPiBase.resolve(relativePath.getParent());
                        if (Files.exists(sessionsDir)) {
                            try (Stream<Path> ls = Files.list(sessionsDir)) {
                                String contents = ls.map(p -> p.getFileName().toString()).collect(
                                        java.util.stream.Collectors.joining(", "));
                                logger.info("Contents of sessions dir [{}]: {}", sessionsDir, contents);
                            }
                        } else {
                            logger.warn("Sessions directory does not exist: {}", sessionsDir);
                        }
                    }
                } catch (Exception e) {
                    logger.warn("Error exporting {}: {}", jsonlFile.getFileName(), e.getMessage());
                }
            }
        }

        if (htmlTraces.isEmpty()) {
            logger.warn("No HTML traces found or generated");
        } else {
            logger.info("Found/generated {} HTML trace(s)", htmlTraces.size());
        }

        return htmlTraces.isEmpty() ? "" : htmlTraces.getFirst();
    }

    /**
     * Helper method to list directory contents for debugging.
     */
    private String listDirectoryContents(Path dir) {
        if (!Files.exists(dir)) {
            return "directory does not exist";
        }
        try (Stream<Path> paths = Files.walk(dir)) {
            return paths.map(p -> p.toString().replace(dir.toString(), ".")).collect(java.util.stream.Collectors.joining(", "));
        } catch (IOException e) {
            return "error reading directory: " + e.getMessage();
        }
    }

    @Override
    public String getName() {
        return "pi";
    }
}
