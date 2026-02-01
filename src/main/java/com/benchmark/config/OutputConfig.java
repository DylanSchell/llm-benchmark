package com.benchmark.config;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Output configuration for the benchmark runner.
 */
public class OutputConfig {

    @JsonProperty("results_dir")
    private String resultsDir = "../benchmark-results";

    @JsonProperty("log_level")
    private String logLevel = "INFO";

    public String getResultsDir() {
        return resultsDir;
    }

    public String getLogLevel() {
        return logLevel;
    }

    // Setters
    public void setResultsDir(String resultsDir) {
        this.resultsDir = resultsDir;
    }

    public void setLogLevel(String logLevel) {
        this.logLevel = logLevel;
    }
}
