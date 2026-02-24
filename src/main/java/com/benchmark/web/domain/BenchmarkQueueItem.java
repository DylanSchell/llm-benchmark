package com.benchmark.web.domain;

import java.time.Instant;
import java.util.UUID;

/**
 * Represents a single item in the benchmark queue.
 * Each item corresponds to one language/exercise combination to be executed.
 */
public class BenchmarkQueueItem {
    private final String id;
    private final String targetDirectory;
    private final String agentName;
    private final String model;
    private final String language;
    private final String exercise;
    private final Instant queuedAt;
    private QueueItemStatus status;
    private String sessionId;

    /**
     * Status of a queue item.
     */
    public enum QueueItemStatus {
        PENDING,      // Waiting to be executed
        RUNNING,      // Currently executing
        COMPLETED,    // Successfully completed
        FAILED,       // Execution failed
        CANCELLED     // Cancelled by user
    }

    public BenchmarkQueueItem(String targetDirectory, String agentName, String model,
                               String language, String exercise) {
        this.id = UUID.randomUUID().toString();
        this.targetDirectory = targetDirectory;
        this.agentName = agentName;
        this.model = model;
        this.language = language;
        this.exercise = exercise;
        this.queuedAt = Instant.now();
        this.status = QueueItemStatus.PENDING;
    }

    public String getId() {
        return id;
    }

    public String getTargetDirectory() {
        return targetDirectory;
    }

    public String getAgentName() {
        return agentName;
    }

    public String getModel() {
        return model;
    }

    public String getLanguage() {
        return language;
    }

    public String getExercise() {
        return exercise;
    }

    public Instant getQueuedAt() {
        return queuedAt;
    }

    public QueueItemStatus getStatus() {
        return status;
    }

    public void setStatus(QueueItemStatus status) {
        this.status = status;
    }

    public String getSessionId() {
        return sessionId;
    }

    public void setSessionId(String sessionId) {
        this.sessionId = sessionId;
    }

    /**
     * Check if this item is for all exercises (no specific exercise).
     */
    public boolean isAllExercises() {
        return exercise == null || exercise.isEmpty();
    }
}
