package com.benchmark.config;

import com.fasterxml.jackson.annotation.JsonProperty;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Docker configuration for the benchmark runner.
 */
public class DockerConfig {

    @JsonProperty("image")
    private String image = "claude-benchmark-runner:latest";

    @JsonProperty("work_dir")
    private String workDir = "/workspace";

    @JsonProperty("timeout")
    private int timeout = 300;

    @JsonProperty("memory")
    private String memory = "2g";

    @JsonProperty("environment")
    private List<Map<String, String>> environment;

    public String getImage() {
        return image;
    }

    public String getWorkDir() {
        return workDir;
    }

    public int getTimeout() {
        return timeout;
    }

    public String getMemory() {
        return memory;
    }

    public List<Map<String, String>> getEnvironment() {
        return environment;
    }

    /**
     * Returns environment variables as a flat map for convenience.
     */
    public Map<String, String> getEnvironmentMap() {
        Map<String, String> result = new LinkedHashMap<>();
        if (environment != null) {
            for (Map<String, String> entry : environment) {
                result.putAll(entry);
            }
        }
        return result;
    }

    // Setters
    public void setImage(String image) {
        this.image = image;
    }

    public void setWorkDir(String workDir) {
        this.workDir = workDir;
    }

    public void setTimeout(int timeout) {
        this.timeout = timeout;
    }

    public void setMemory(String memory) {
        this.memory = memory;
    }

    public void setEnvironment(List<Map<String, String>> environment) {
        this.environment = environment;
    }
}
