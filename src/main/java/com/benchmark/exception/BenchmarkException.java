package com.benchmark.exception;

/**
 * Base exception for all benchmark-related errors.
 */
public class BenchmarkException extends RuntimeException {
    public BenchmarkException(String message) {
        super(message);
    }

    public BenchmarkException(String message, Throwable cause) {
        super(message, cause);
    }
}
