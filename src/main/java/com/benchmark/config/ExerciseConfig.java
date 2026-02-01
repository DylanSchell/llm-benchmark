package com.benchmark.config;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Exercise configuration for the benchmark runner.
 */
public class ExerciseConfig {

    @JsonProperty("language")
    private String language = "java";

    @JsonProperty("name")
    private String name;

    @JsonProperty("path")
    private String path;

    public String getLanguage() {
        return language;
    }

    public String getName() {
        return name;
    }

    public String getPath() {
        return path;
    }

    // Setters
    public void setLanguage(String language) {
        this.language = language;
    }

    public void setName(String name) {
        this.name = name;
    }

    public void setPath(String path) {
        this.path = path;
    }
}
