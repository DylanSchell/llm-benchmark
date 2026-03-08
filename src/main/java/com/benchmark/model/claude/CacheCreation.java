package com.benchmark.model.claude;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Typed representation of the {@code cache_creation} field when it is an object.
 */
public class CacheCreation {
    @JsonProperty("ephemeral_1h_input_tokens")
    private int ephemeral1hInputTokens;

    @JsonProperty("ephemeral_5m_input_tokens")
    private int ephemeral5mInputTokens;

    public int getEphemeral1hInputTokens() {
        return ephemeral1hInputTokens;
    }

    public void setEphemeral1hInputTokens(int ephemeral1hInputTokens) {
        this.ephemeral1hInputTokens = ephemeral1hInputTokens;
    }

    public int getEphemeral5mInputTokens() {
        return ephemeral5mInputTokens;
    }

    public void setEphemeral5mInputTokens(int ephemeral5mInputTokens) {
        this.ephemeral5mInputTokens = ephemeral5mInputTokens;
    }
}
