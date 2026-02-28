package com.benchmark.exception;

/**
 * Exception thrown when Docker execution fails.
 */
public class DockerExecutionException extends BenchmarkException {
    public DockerExecutionException(String message) {
        super(message);
    }

    public DockerExecutionException(String message, Throwable cause) {
        super(message, cause);
    }

    public DockerExecutionException(String containerId, int exitCode, String output) {
        super(String.format("Docker execution failed for container %s: exit code %d. Output: %s", 
                containerId, exitCode, truncateOutput(output)));
    }

    private static String truncateOutput(String output) {
        if (output == null || output.length() <= 200) {
            return output;
        }
        return output.substring(0, 200) + "... (truncated)";
    }
}
