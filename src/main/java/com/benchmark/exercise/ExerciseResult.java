package com.benchmark.exercise;

import java.time.Duration;
import java.time.Instant;

/**
 * Represents the result of running an exercise.
 */
public class ExerciseResult {
    private final String exerciseName;
    private final String language;
    private final boolean success;
    private final int exitCode;
    private final String output;
    private final Duration duration;
    private final Instant startTime;
    private final Instant endTime;
    private final String errorMessage;
    private final String trace;
    private final String model;

    private ExerciseResult(Builder builder) {
        this.model = builder.model;
        this.exerciseName = builder.exerciseName;
        this.language = builder.language;
        this.success = builder.success;
        this.exitCode = builder.exitCode;
        this.output = builder.output;
        this.duration = builder.duration;
        this.startTime = builder.startTime;
        this.endTime = builder.endTime;
        this.errorMessage = builder.errorMessage;
        this.trace = builder.trace;
    }

    public static Builder builder() {
        return new Builder();
    }

    public String getExerciseName() {
        return exerciseName;
    }

    public String getLanguage() {
        return language;
    }

    public boolean isSuccess() {
        return success;
    }

    public int getExitCode() {
        return exitCode;
    }

    public String getOutput() {
        return output;
    }

    public Duration getDuration() {
        return duration;
    }

    public Instant getStartTime() {
        return startTime;
    }

    public Instant getEndTime() {
        return endTime;
    }

    public String getErrorMessage() {
        return errorMessage;
    }

    public String getTrace() {
        return trace;
    }

    public String getModel() {
        return model;
    }

    @Override
    public String toString() {
        return String.format("ExerciseResult{name='%s', language='%s', success=%s, duration=%s}",
                exerciseName, language, success, duration);
    }

    public static class Builder {
        private String exerciseName;
        private String language;
        private boolean success;
        private int exitCode;
        private String output;
        private Duration duration;
        private Instant startTime;
        private Instant endTime;
        private String errorMessage;
        private String trace;
        private String model;

        public Builder exerciseName(String exerciseName) {
            this.exerciseName = exerciseName;
            return this;
        }

        public Builder language(String language) {
            this.language = language;
            return this;
        }

        public Builder success(boolean success) {
            this.success = success;
            return this;
        }

        public Builder exitCode(int exitCode) {
            this.exitCode = exitCode;
            return this;
        }

        public Builder output(String output) {
            this.output = output;
            return this;
        }

        public Builder duration(Duration duration) {
            this.duration = duration;
            return this;
        }

        public Builder startTime(Instant startTime) {
            this.startTime = startTime;
            return this;
        }

        public Builder endTime(Instant endTime) {
            this.endTime = endTime;
            return this;
        }

        public Builder trace(String trace) {
            this.trace = trace;
            return this;
        }

        public Builder errorMessage(String errorMessage) {
            this.errorMessage = errorMessage;
            return this;
        }

        public Builder model(String model) {
            this.model = model;
            return this;
        }

        public ExerciseResult build() {
            return new ExerciseResult(this);
        }
    }
}
