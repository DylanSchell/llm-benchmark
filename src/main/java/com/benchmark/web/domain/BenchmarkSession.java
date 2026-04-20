package com.benchmark.web.domain;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.MediaType;
import org.springframework.web.servlet.mvc.method.annotation.SseEmitter;

import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Represents a benchmark run session.
 * Tracks the state of a benchmark execution including status, configuration, and output streaming.
 */
public class BenchmarkSession {
    private static final Logger logger = LoggerFactory.getLogger(BenchmarkSession.class);

    private final String id;
    private final String agentName;
    private final String[] languages;
    private final String model;
    private final String exerciseName; // null for "all exercises" runs
    private final Instant startTime;
    private RunStatus status;
    private final SseEmitter sseEmitter;
    private final List<String> accumulatedOutput;
    private final AtomicInteger totalExercises;
    private final AtomicInteger completedExercises;
    private Instant endTime;
    private String errorMessage;

    public BenchmarkSession(String id, String agentName, String[] languages, String model, String exerciseName, long timeoutMs) {
        this.id = id;
        this.agentName = agentName;
        this.languages = languages;
        this.model = model;
        this.exerciseName = exerciseName;
        this.startTime = Instant.now();
        this.status = RunStatus.PENDING;
        this.sseEmitter = new SseEmitter(timeoutMs);
        this.accumulatedOutput = new ArrayList<>();
        this.totalExercises = new AtomicInteger(0);
        this.completedExercises = new AtomicInteger(0);
    }

    public String getId() {
        return id;
    }

    public String getAgentName() {
        return agentName;
    }

    public String[] getLanguages() {
        return languages;
    }

    public String getLanguage() {
        return languages != null && languages.length > 0 ? languages[0] : null;
    }

    public String getExerciseName() {
        return exerciseName;
    }

    public String getModel() {
        return model;
    }

    public Instant getStartTime() {
        return startTime;
    }

    public RunStatus getStatus() {
        return status;
    }

    public void setStatus(RunStatus status) {
        this.status = status;
        if (status == RunStatus.COMPLETED || status == RunStatus.FAILED || status == RunStatus.CANCELLED) {
            this.endTime = Instant.now();
        }
    }

    public Instant getEndTime() {
        return endTime;
    }

    public String getErrorMessage() {
        return errorMessage;
    }

    public void setErrorMessage(String errorMessage) {
        this.errorMessage = errorMessage;
    }

    public int getTotalExercises() {
        return totalExercises.get();
    }

    public void setTotalExercises(int count) {
        this.totalExercises.set(count);
    }

    public int getCompletedExercises() {
        return completedExercises.get();
    }

    public void incrementCompletedExercises() {
        this.completedExercises.incrementAndGet();
    }

    public void setCompletedExercises(int count) {
        this.completedExercises.set(count);
    }

    /**
     * Emits output line to SSE subscribers and accumulates it.
     * Safe to call after completeOutput() - silently drops output if emitter is closed.
     */
    public void emitOutput(String line) {
        synchronized (accumulatedOutput) {
            accumulatedOutput.add(line);
        }
        try {
            // Wrap data in JSON to preserve leading/trailing whitespace
            // EventSource may trim whitespace from raw SSE data fields
            String jsonPayload = "{\"data\":\"" + escapeJson(line) + "\"}";
            sseEmitter.send(SseEmitter.event()
                .data(jsonPayload, MediaType.APPLICATION_JSON));
        } catch (IllegalStateException e) {
            // Emitter has already been completed (session finished) - drop the output
            logger.debug("Dropping output for completed session {}: {}", id, line);
        } catch (Exception e) {
            // Client may have disconnected
            logger.debug("SSE send failed for session {}: {}", id, e.getMessage());
        }
    }

    /**
     * Escapes special characters for JSON string values.
     */
    private String escapeJson(String value) {
        if (value == null) return "";
        return value
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\t", "\\t");
    }

    /**
     * Returns the accumulated output so far.
     * Concatenates all tokens without separators to preserve original formatting.
     */
    public String getAccumulatedOutput() {
        synchronized (accumulatedOutput) {
            StringBuilder sb = new StringBuilder();
            for (String token : accumulatedOutput) {
                sb.append(token);
            }
            return sb.toString();
        }
    }

    /**
     * Returns the SSE emitter for streaming output.
     */
    public SseEmitter getSseEmitter() {
        return sseEmitter;
    }

    /**
     * Completes the SSE stream.
     */
    public void completeOutput() {
        try {
            sseEmitter.complete();
        } catch (Exception e) {
            // Ignore - client may have disconnected
        }
    }

    /**
     * Forces completion of the SSE stream, used during shutdown.
     */
    public void forceComplete() {
        try {
            sseEmitter.completeWithError(new IllegalStateException("Session terminated during shutdown"));
        } catch (Exception e) {
            // Ignore
        }
    }

    /**
     * Checks if this is a run for all exercises.
     */
    public boolean isAllExercises() {
        return exerciseName == null || exerciseName.isEmpty();
    }

    /**
     * Get progress as a percentage.
     */
    public double getProgress() {
        if (totalExercises.get() == 0) {
            return 0.0;
        }
        return (double) completedExercises.get() / totalExercises.get() * 100.0;
    }
}
