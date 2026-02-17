package com.benchmark.model;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Represents the usage information in a message.
 * Example: {"input_tokens":17409,"output_tokens":104}
 */
public class Usage {
    @JsonProperty("server_tool_use")
    private ServerToolUse serverToolUse;

    @JsonProperty("service_tier")
    private ServiceTier serviceTier;

    public ServiceTier getServiceTier() {
        return serviceTier;
    }

    public void setServiceTier(ServiceTier serviceTier) {
        this.serviceTier = serviceTier;
    }

    public ServerToolUse getServerToolUse() {
        return serverToolUse;
    }

    public void setServerToolUse(ServerToolUse serverToolUse) {
        this.serverToolUse = serverToolUse;
    }


    @JsonProperty("cache_read_input_tokens")
    private int cacheReadInputTokens;

    public int getCacheReadInputTokens() {
        return cacheReadInputTokens;
    }

    public void setCacheReadInputTokens(int cacheReadInputTokens) {
        this.cacheReadInputTokens = cacheReadInputTokens;
    }

    @JsonProperty("cache_creation_input_tokens")
    private int cacheCreationInputTokens;

    public int getCacheCreationInputTokens() {
        return cacheCreationInputTokens;
    }

    public void setCacheCreationInputTokens(int cacheCreationInputTokens) {
        this.cacheCreationInputTokens = cacheCreationInputTokens;
    }

    @JsonProperty("input_tokens")
    private long inputTokens;

    @JsonProperty("output_tokens")
    private long outputTokens;

    @JsonProperty("cache_creation")
    private CacheCreation cacheCreation;

    @JsonProperty("inference_geo")
    private String inferenceGeo;

    @JsonProperty("iterations")
    private int iterations;

    public long getInputTokens() {
        return inputTokens;
    }

    public void setInputTokens(long inputTokens) {
        this.inputTokens = inputTokens;
    }

    public long getOutputTokens() {
        return outputTokens;
    }

    public void setOutputTokens(long outputTokens) {
        this.outputTokens = outputTokens;
    }

    public CacheCreation getCacheCreation() {
        return cacheCreation;
    }

    public void setCacheCreation(CacheCreation cacheCreation) {
        this.cacheCreation = cacheCreation;
    }

    public String getInferenceGeo() {
        return inferenceGeo;
    }

    public void setInferenceGeo(String inferenceGeo) {
        this.inferenceGeo = inferenceGeo;
    }

    public int getIterations() {
        return iterations;
    }

    public void setIterations(int iterations) {
        this.iterations = iterations;
    }
}
