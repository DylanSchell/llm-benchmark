package io.schell.llm.benchmark.exception;

/**
 * Exception thrown when a benchmark execution fails.
 */
public class BenchmarkExecutionException extends BenchmarkException {
    public BenchmarkExecutionException(String message) {
        super(message);
    }

    public BenchmarkExecutionException(String message, Throwable cause) {
        super(message, cause);
    }

    public BenchmarkExecutionException(String exerciseName, String language, Throwable cause) {
        super(String.format("Benchmark execution failed for %s/%s: %s", language, exerciseName, cause.getMessage()), cause);
    }
}
