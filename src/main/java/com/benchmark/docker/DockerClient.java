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
import java.time.Duration;
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

        String image = containerImage != null ? containerImage : config.getImage();
        String work = workDir != null ? workDir : config.getWorkDir();
        int timeout = timeoutSeconds > 0 ? timeoutSeconds : config.getTimeout();
        String memory = memoryLimit != null ? memoryLimit : config.getMemory();
        String hostDir = volumeHostDir != null ? volumeHostDir : getCurrentDir();

        Files.createDirectories(Paths.get(hostDir).resolve(".claude"));
        List<String> fullCommand = new ArrayList<>();
        fullCommand.add("docker");
        fullCommand.add("run");
        // Generate a deterministic unique container name for this run
        String containerName = "bench-" + java.util.UUID.randomUUID().toString().replaceAll("-", "").substring(0, 12);
        fullCommand.add("--name");
        fullCommand.add(containerName);
        fullCommand.add("--rm");
        fullCommand.add("-w");
        fullCommand.add(work);
        fullCommand.add("-m");
        fullCommand.add(memory);
        addEnvironmentVariables(fullCommand);
        fullCommand.add("-v");
        fullCommand.add(hostDir + ":/workspace");
        fullCommand.add("-v");
        fullCommand.add(hostDir + "/.claude" + ":/home/runner/.claude");
        fullCommand.add(image);
        fullCommand.addAll(command);

        logger.debug("Executing with memory limit {} and volume {}:/workspace: {}",
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
        readerThread.start();
        boolean completed = waitForProcess(process, timeout);
        readerThread.join();
//        String output = readOutput(process.getInputStream(), outputCallback);
        int exitCode = -1;
        if (completed) {
            // if we cancelled, the process might not have exited
            exitCode = process.exitValue();
        }
        return new ProcessResult(exitCode, sb.toString(), completed, containerName);
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
