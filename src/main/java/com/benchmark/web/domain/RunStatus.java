package com.benchmark.web.domain;

/**
 * Enum representing the status of a benchmark run.
 */
public enum RunStatus {
    PENDING,      // Run is queued but not started
    RUNNING,      // Run is in progress
    COMPLETED,    // Run completed successfully
    FAILED,       // Run failed with errors
    CANCELLED     // Run was cancelled by user
}
