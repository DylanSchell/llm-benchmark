package com.benchmark.model;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Represents the {@code context_management} field when it is an object.
 */
public class ContextManagement {
    @JsonProperty("version")
    private String version;

    public String getVersion() {
        return version;
    }

    public void setVersion(String version) {
        this.version = version;
    }
}
