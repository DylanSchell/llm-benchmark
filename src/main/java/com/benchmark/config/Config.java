package com.benchmark.config;

import com.fasterxml.jackson.annotation.JsonProperty;

import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * Root configuration class for the benchmark runner.
 */
public class Config {

    @JsonProperty("parallelism")
    private int parallelism = 1; // default: run sequentially

    @JsonProperty("benchmark_path")
    private String benchmarkPath = "../polyglot-benchmark";

    @JsonProperty("docker")
    private DockerConfig docker;

    @JsonProperty("exercise")
    private ExerciseConfig exercise;

    @JsonProperty("claude")
    private ClaudeConfig claude;

    @JsonProperty("output")
    private OutputConfig output;

    public Path getBenchmarkPath() {
        return Paths.get(benchmarkPath);
    }

    public DockerConfig getDocker() {
        return docker;
    }

    public ExerciseConfig getExercise() {
        return exercise;
    }

    public ClaudeConfig getClaude() {
        return claude;
    }

    public OutputConfig getOutput() {
        return output;
    }

    public int getParallelism() {
        return parallelism;
    }

    public void setParallelism(int parallelism) {
        this.parallelism = parallelism;
    }

    // Setters for builder pattern
    public void setBenchmarkPath(String benchmarkPath) {
        this.benchmarkPath = benchmarkPath;
    }

    public void setDocker(DockerConfig docker) {
        this.docker = docker;
    }

    public void setExercise(ExerciseConfig exercise) {
        this.exercise = exercise;
    }

    public void setClaude(ClaudeConfig claude) {
        this.claude = claude;
    }

    public void setOutput(OutputConfig output) {
        this.output = output;
    }
}
