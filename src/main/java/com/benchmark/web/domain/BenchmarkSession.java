package com.benchmark.web.domain;

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

    public BenchmarkSession(String id, String agentName, String[] languages, String model, String exerciseName) {
        this.id = id;
        this.agentName = agentName;
        this.languages = languages;
        this.model = model;
        this.exerciseName = exerciseName;
        this.startTime = Instant.now();
        this.status = RunStatus.PENDING;
        this.sseEmitter = new SseEmitter(5 * 60 * 1000L); // 5 minute timeout
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
     */
    public void emitOutput(String line) {
        synchronized (accumulatedOutput) {
            accumulatedOutput.add(line);
        }
        try {
            sseEmitter.send(SseEmitter.event().name("message").data(line));
        } catch (Exception e) {
            // Ignore - client may have disconnected
        }
    }

    /**
     * Returns the accumulated output so far.
     */
    public String getAccumulatedOutput() {
        synchronized (accumulatedOutput) {
            return String.join("\n", accumulatedOutput);
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
