package io.schell.llm.benchmark.config;

import com.fasterxml.jackson.annotation.JsonProperty;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * Root configuration class for the benchmark runner.
 */
public class Config {
    private static final Logger logger = LoggerFactory.getLogger(Config.class);

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

    @JsonProperty("model")
    private String model;

    @JsonProperty("inference_endpoint")
    private String inferenceEndpoint = "http://localhost:8000/v1";

    @JsonProperty("api_key")
    private String apiKey;

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
        if ( model != null ) {
            docker.updateModelEnvironment(model);
        }
    }

    public void setModel(String model) {
        this.model = model;
        if ( docker != null ) {
            docker.updateModelEnvironment(model);
        }
        if ( claude != null ) {
            claude.setModel(model);
        }
    }

    public String getModel() {
        return model;
    }

    public String getInferenceEndpoint() {
        return inferenceEndpoint;
    }

    public void setInferenceEndpoint(String inferenceEndpoint) {
        this.inferenceEndpoint = inferenceEndpoint;
    }

    public String getApiKey() {
        return apiKey;
    }

    public void setApiKey(String apiKey) {
        this.apiKey = apiKey;
    }

    public void setExercise(ExerciseConfig exercise) {
        this.exercise = exercise;
    }

    public void setClaude(ClaudeConfig claude) {
        this.claude = claude;
        if ( model != null ) {
            this.claude.setModel(model);
        }
    }

    public void setOutput(OutputConfig output) {
        this.output = output;
    }

    /**
     * Validates the configuration.
     * Checks for required fields and valid paths.
     *
     * @throws ConfigurationException if validation fails
     */
    public void validate() throws ConfigurationException {
        // Validate parallelism
        if (parallelism < 1) {
            throw new ConfigurationException("parallelism must be at least 1, got: " + parallelism);
        }

        // Validate benchmark path exists
        Path benchmarkPath = getBenchmarkPath();
        if (!Files.exists(benchmarkPath)) {
            throw new ConfigurationException("benchmark_path does not exist: " + benchmarkPath.toAbsolutePath());
        }

        // Validate docker configuration
        if (docker == null) {
            throw new ConfigurationException("docker configuration is required");
        }
        docker.validate();

        // Validate output configuration
        if (output == null) {
            throw new ConfigurationException("output configuration is required");
        }
        output.validate();

        // Validate claude configuration (optional but recommended)
        if (claude != null) {
            claude.validate();
        }

        logger.debug("Configuration validation successful");
    }
}
