package com.benchmark.model;

import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.databind.JsonNode;

public class SystemEntry extends LogEntry {
    @JsonProperty("subtype")
    private String subtype;

    @JsonProperty("level")
    private String level;

    @JsonProperty("error")
    private JsonNode error;

    @JsonProperty("slug")
    private String slug;

    @JsonProperty("retryInMs")
    private double retryInMs;

    @JsonProperty("retryAttempt")
    private int retryAttempt;

    @JsonProperty("maxRetries")
    private int maxRetries;

    @JsonProperty("cause")
    private JsonNode cause;

    public JsonNode getCause() {
        return cause;
    }

    public void setCause(JsonNode cause) {
        this.cause = cause;
    }

    public String getSubtype() {
        return subtype;
    }

    public void setSubtype(String subtype) {
        this.subtype = subtype;
    }

    public String getLevel() {
        return level;
    }

    public void setLevel(String level) {
        this.level = level;
    }

    public JsonNode getError() {
        return error;
    }

    public void setError(JsonNode error) {
        this.error = error;
    }

    public String getSlug() {
        return slug;
    }

    public void setSlug(String slug) {
        this.slug = slug;
    }

    public double getRetryInMs() {
        return retryInMs;
    }

    public void setRetryInMs(double retryInMs) {
        this.retryInMs = retryInMs;
    }

    public int getRetryAttempt() {
        return retryAttempt;
    }

    public void setRetryAttempt(int retryAttempt) {
        this.retryAttempt = retryAttempt;
    }

    public int getMaxRetries() {
        return maxRetries;
    }

    public void setMaxRetries(int maxRetries) {
        this.maxRetries = maxRetries;
    }
}
