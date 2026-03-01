package com.benchmark.agent;

import com.benchmark.docker.DockerClient;
import com.benchmark.docker.DockerClient.ProcessResult;
import com.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
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
    protected ReferenceResult runAgent(Exercise exercise, Path hostExerciseDir, Path tempWorkDir, Path resultsDir) throws IOException {
        Instant startTime = Instant.now();

        try {
            logger.info("Starting exercise with Pi agent: {} at {}", exercise.getName(), startTime);

            // Create models.json configuration for pi inside the container
            createModelsJson(tempWorkDir);

            // Create exercise prompt for pi
            String prompt = createExercisePrompt(exercise, tempWorkDir);
            patchTests(exercise, tempWorkDir);

            // Build pi command with JSON output mode
            List<String> command = buildPiCommand(prompt);

            ProcessResult result = getDockerClient().runCommandWithLimitsAndVolume(
                    null,  // use default image from config
                    "/workspace",
                    command,
                    -1,    // use default timeout from config
                    null,  // use default memory from config
                    tempWorkDir.toAbsolutePath().toString(),  // mount temp dir as /workspace
                    getOutputCallback(),  // stream output to stdout
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

            // Collect trace from pi session files
            String trace = collectPiTrace(tempWorkDir, resultsDir, exercise);

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
                    .output(e.getMessage())
                    .language(exercise.getLanguage())
                    .exerciseName(exercise.getName())
                    .agent("pi")
                    .build();
        }
    }

    /**
     * Creates the models.json configuration file for pi inside the working directory.
     * This configures pi to use the correct model endpoint by reading from DockerConfig.
     */
    private void createModelsJson(Path tempWorkDir) throws IOException {
        // Create .pi/agent directory structure
        Path piAgentDir = tempWorkDir.resolve(".pi").resolve("agent");
        Files.createDirectories(piAgentDir);

        // Read environment configuration from Docker config (same as what's passed to container)
        Map<String, String> envVars = getDockerClient().getConfig().getEnvironmentMap();
        
        String baseUrl = envVars.getOrDefault("ANTHROPIC_BASE_URL", "http://host.docker.internal:8080");
        
        String apiKey = envVars.getOrDefault("ANTHROPIC_API_KEY", "");
        if (apiKey.isEmpty()) {
            apiKey = envVars.getOrDefault("ANTHROPIC_AUTH_TOKEN", "placeholder-key");
        }

        String model = envVars.getOrDefault("ANTHROPIC_MODEL", "claude-sonnet-4");

        // Create models.json configuration
        String modelsJson = String.format(
                "{" +
                "  \"providers\": {" +
                "    \"anthropic\": {" +
                "      \"baseUrl\": \"%s\"," +
                "      \"apiKey\": \"%s\"," +
                "      \"api\": \"anthropic-messages\"," +
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
        logger.debug("Created models.json at: {}", modelsFile);
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
    private List<String> buildPiCommand(String prompt) {
        List<String> command = new ArrayList<>();
        command.add("pi");
        command.add("--mode");
        command.add("json");
        command.add("--tools");
        command.add("read,bash,edit,write,grep,find,ls");
        command.add("--no-session");
        command.add(prompt);
        return command;
    }

    /**
     * Collects trace information from pi session files.
     */
    private String collectPiTrace(Path tempWorkDir, Path resultsDir, Exercise exercise) throws IOException {
        // Pi stores sessions in ~/.pi/sessions by default
        // The .pi directory is mounted from host, so we look there after container exits
        Path piSessionsDir = tempWorkDir.resolve(".pi").resolve("sessions");

        if (!Files.exists(piSessionsDir)) {
            logger.debug("No pi sessions directory found at: {}", piSessionsDir);
            return "";
        }

        List<String> htmlTraces = new ArrayList<>();

        // Look for HTML export files or JSON session files
        try (Stream<Path> paths = Files.walk(piSessionsDir)) {
            paths.filter(Files::isRegularFile).forEach(sessionFile -> {
                String fileName = sessionFile.getFileName().toString();
                try {
                    if (fileName.endsWith(".html")) {
                        // Found an HTML trace
                        htmlTraces.add(Files.readString(sessionFile));
                    } else if (fileName.endsWith(".json") || fileName.endsWith(".jsonl")) {
                        // Copy JSON/JSONL log files to results directory
                        String targetName = "log_pi_" + exercise.getLanguage() + "_" + exercise.getName() + "_" + fileName;
                        Files.copy(sessionFile, resultsDir.resolve(targetName), StandardCopyOption.REPLACE_EXISTING);
                        logger.debug("Copied pi log file: {}", targetName);
                    }
                } catch (IOException e) {
                    logger.warn("Failed to process session file {}: {}", sessionFile, e.getMessage());
                }
            });
        }

        // Also check for JSONL trace files that pi might output directly
        Path jsonlTrace = tempWorkDir.resolve(".pi").resolve("trace.jsonl");
        if (Files.exists(jsonlTrace)) {
            String targetName = "log_pi_" + exercise.getLanguage() + "_" + exercise.getName() + ".jsonl";
            Files.copy(jsonlTrace, resultsDir.resolve(targetName), StandardCopyOption.REPLACE_EXISTING);
            logger.debug("Copied pi trace file: {}", targetName);
        }

        return htmlTraces.isEmpty() ? "" : htmlTraces.get(0);
    }

    @Override
    public String getName() {
        return "pi";
    }
}
