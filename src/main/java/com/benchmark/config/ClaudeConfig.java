package com.benchmark.config;

import com.fasterxml.jackson.annotation.JsonProperty;

import java.util.List;

/**
 * Claude Code CLI configuration for the benchmark runner.
 */
public class ClaudeConfig {

    @JsonProperty("cli_path")
    private String cliPath = "/usr/local/bin/claude";

    @JsonProperty("model")
    private String model = "sonnet";

    @JsonProperty("extra_args")
    private List<String> extraArgs;

    public String getCliPath() {
        return cliPath;
    }

    public String getModel() {
        return model;
    }

    public List<String> getExtraArgs() {
        return extraArgs;
    }

    // Setters
    public void setCliPath(String cliPath) {
        this.cliPath = cliPath;
    }

    public void setModel(String model) {
        this.model = model;
    }

    public void setExtraArgs(List<String> extraArgs) {
        this.extraArgs = extraArgs;
    }
}
