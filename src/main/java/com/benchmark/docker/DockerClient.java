package com.benchmark.docker;

import com.benchmark.config.DockerConfig;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import java.util.function.Consumer;

/**
 * Wrapper for Docker operations required by the benchmark runner.
 * Uses ProcessBuilder to invoke Docker CLI commands.
 */
public class DockerClient {
    private static final Logger logger = LoggerFactory.getLogger(DockerClient.class);

    private final DockerConfig config;

    public DockerClient(DockerConfig config) {
        this.config = config;
    }

    /**
     * Updates the model environment variables in the Docker config.
     * This allows dynamic model selection at runtime.
     *
     * @param modelName The model name to use
     */
    public void setModel(String modelName) {
        if (modelName != null && !modelName.isEmpty()) {
            config.updateModelEnvironment(modelName);
            logger.info("Updated Docker environment to use model: {}", modelName);
        }
    }

    /**
     * Returns the DockerConfig instance.
     * Used by agents that need access to configuration (e.g., PiAgent for models.json).
     */
    public DockerConfig getConfig() {
        return config;
    }

    /**
     * Checks if Docker is available and running.
     *
     * @return true if Docker is available
     */
    public boolean isAvailable() {
        try {
            return executeCommand(List.of("docker", "version", "--format", "{{.Server.Version}}")) != null;
        } catch (Exception e) {
            logger.error("Docker is not available: {}", e.getMessage());
            return false;
        }
    }

    /**
     * Runs a command with memory limits and a custom volume mount, streaming output to a callback.
     *
     * @param containerImage Docker image to use (uses config default if null)
     * @param workDir        Working directory inside the container (uses config default if null)
     * @param command        Command to execute
     * @param timeoutSeconds Timeout in seconds (uses config default if <= 0)
     * @param memoryLimit    Memory limit (uses config default if null)
     * @param volumeHostDir  Host directory to mount as /workspace (uses current dir if null)
     * @param outputCallback Optional callback to receive output lines in real-time
     * @return ProcessResult with exit code and output
     * @throws IOException          if execution fails
     * @throws InterruptedException if execution is interrupted
     */
    public ProcessResult runCommandWithLimitsAndVolume(String containerImage, String workDir, List<String> command,
                                                       int timeoutSeconds, String memoryLimit, String volumeHostDir,
                                                       Consumer<String> outputCallback)
            throws IOException, InterruptedException {
        return runCommandWithLimitsAndVolume(containerImage, workDir, command, timeoutSeconds, memoryLimit, volumeHostDir, outputCallback, false);
    }

    /**
     * Runs a command with memory limits and custom volume mounts, streaming output to a callback.
     *
     * @param containerImage Docker image to use (uses config default if null)
     * @param workDir        Working directory inside the container (uses config default if null)
     * @param command        Command to execute
     * @param timeoutSeconds Timeout in seconds (uses config default if <= 0)
     * @param memoryLimit    Memory limit (uses config default if null)
     * @param volumeHostDir  Host directory to mount as /workspace (uses current dir if null)
     * @param outputCallback Optional callback to receive output lines in real-time
     * @param enablePiVolume If true, also mounts .pi directory for pi agent session data
     * @return ProcessResult with exit code and output
     * @throws IOException          if execution fails
     * @throws InterruptedException if execution is interrupted
     */
    public ProcessResult runCommandWithLimitsAndVolume(String containerImage, String workDir, List<String> command,
                                                       int timeoutSeconds, String memoryLimit, String volumeHostDir,
                                                       Consumer<String> outputCallback, boolean enablePiVolume)
            throws IOException, InterruptedException {

        String image = containerImage != null ? containerImage : config.getImage();
        String work = workDir != null ? workDir : config.getWorkDir();
        int timeout = timeoutSeconds > 0 ? timeoutSeconds : config.getTimeout();
        String memory = memoryLimit != null ? memoryLimit : config.getMemory();
        String hostDir = volumeHostDir != null ? volumeHostDir : getCurrentDir();

        Files.createDirectories(Paths.get(hostDir).resolve(".claude"));
        if (enablePiVolume) {
            Files.createDirectories(Paths.get(hostDir).resolve(".pi"));
        }
        List<String> fullCommand = new ArrayList<>();
        fullCommand.add("docker");
        fullCommand.add("run");
        // Generate a deterministic unique container name for this run
        String containerName = "bench-" + java.util.UUID.randomUUID().toString().replaceAll("-", "").substring(0, 12);
        fullCommand.add("--name");
        fullCommand.add(containerName);
        // Note: not using --rm flag because it doesn't work properly when the
        // docker CLI process is killed (e.g., on timeout). We explicitly clean up
        // after execution instead.
        fullCommand.add("-w");
        fullCommand.add(work);
        fullCommand.add("-m");
        fullCommand.add(memory);
        addEnvironmentVariables(fullCommand);
        fullCommand.add("-v");
        fullCommand.add(hostDir + ":/workspace");
        fullCommand.add("-v");
        fullCommand.add(hostDir + "/.claude" + ":/home/runner/.claude");
        if (enablePiVolume) {
            fullCommand.add("-v");
            fullCommand.add(hostDir + "/.pi" + ":/home/runner/.pi");
        }
        fullCommand.add(image);
        fullCommand.addAll(command);

        logger.info("Executing with memory limit {} and volume {}:/workspace: {}",
                memory, hostDir, String.join(" ", fullCommand));

        ProcessBuilder pb = new ProcessBuilder(fullCommand);
        pb.redirectErrorStream(true);
        Process process = pb.start();
        StringBuilder sb = new StringBuilder();
        Thread readerThread = new Thread(() -> {
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(process.getInputStream()))) {
                String line = reader.readLine();
                while (line != null) {
                    // Log each line with container identifier for visibility.
                    // logger.info("[{}] {}", containerName, line);
                    // Forward the original line to the callback unchanged (JSON or not).
                    outputCallback.accept(line);
                    sb.append(line);
                    sb.append(System.lineSeparator());
                    line = reader.readLine();
                }
            } catch (IOException e) {
                throw new RuntimeException(e);
            }
        });
        readerThread.setDaemon(true);
        readerThread.start();
        boolean completed = waitForProcess(process, timeout);
        readerThread.join();

        // Always clean up the container, regardless of timeout or normal exit
        cleanupContainer(containerName);

        int exitCode = -1;
        if (completed) {
            // if we cancelled, the process might not have exited
            exitCode = process.exitValue();
        }
        ProcessResult result = new ProcessResult(exitCode, sb.toString(), completed, containerName);
        
        // Log errors but don't throw here - let caller decide how to handle
        if (!result.isSuccess()) {
            logger.error("Docker command failed with exit code {}: {}", exitCode, result.output().substring(0, Math.min(200, result.output().length())));
        }
        
        return result;
    }

    /**
     * Removes a Docker container by name.
     */
    private void cleanupContainer(String containerName) {
        try {
            ProcessBuilder pb = new ProcessBuilder("docker", "rm", "-f", containerName);
            pb.redirectErrorStream(true);
            Process rmProcess = pb.start();
            // Wait briefly for cleanup to complete
            if (!rmProcess.waitFor(5, TimeUnit.SECONDS)) {
                rmProcess.destroyForcibly();
                logger.warn("Forcefully removed container {} after cleanup timeout", containerName);
            }
        } catch (Exception e) {
            logger.warn("Failed to cleanup container {}: {}", containerName, e.getMessage());
        }
    }

    private String getCurrentDir() {
        return System.getProperty("user.dir");
    }

    /**
     * Adds environment variables to the docker command list.
     */
    private void addEnvironmentVariables(List<String> command) {
        Map<String, String> envVars = config.getEnvironmentMap();
        if (envVars != null) {
            for (Map.Entry<String, String> entry : envVars.entrySet()) {
                command.add("-e");
                command.add(entry.getKey() + "=" + entry.getValue());
            }
        }
    }

    private String executeCommand(List<String> command) throws IOException, InterruptedException {
        ProcessBuilder pb = new ProcessBuilder(command);
        pb.redirectErrorStream(true);
        Process process = pb.start();
        boolean completed = waitForProcess(process, 30);

        String output = readOutput(process.getInputStream());

        if (!completed || process.exitValue() != 0) {
            return null;
        }

        return output.trim();
    }

    private boolean waitForProcess(Process process, int timeoutSeconds) throws InterruptedException {
        boolean finished = process.waitFor(timeoutSeconds, TimeUnit.SECONDS);
        if (!finished) {
            logger.warn("Process timed out after {} seconds", timeoutSeconds);
            process.destroyForcibly();
        }
        return finished;
    }

    private String readOutput(InputStream inputStream) throws IOException {
        return readOutput(inputStream, null);
    }

    private String readOutput(InputStream inputStream, Consumer<String> outputCallback) throws IOException {
        StringBuilder output = new StringBuilder();
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(inputStream))) {
            String line;
            try {
                while ((line = reader.readLine()) != null) {
                    output.append(line).append("\n");
                    if (outputCallback != null) {
                        outputCallback.accept(line);
                    }
                }
            } catch (IOException e) {
                // stream closed? return the output we have so far
                logger.error("Error while reading output from docker process: {}, returning partial output", e.getMessage());
            }
        }
        return output.toString();
    }

    /**
     * Result of a process execution.
     */
    public record ProcessResult(int exitCode, String output, boolean completed, String containerId) {

        public boolean isSuccess() {
            return completed && exitCode == 0;
        }
    }
}
