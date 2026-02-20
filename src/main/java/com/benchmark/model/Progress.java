package com.benchmark.model;

import com.fasterxml.jackson.databind.JsonNode;

public class Progress extends LogEntry {
    private String agentId;
    private String slug;
    private ProgressData data;
    private String toolUseID;
    private String parentToolUseID;

    public String getAgentId() {
        return agentId;
    }

    public void setAgentId(String agentId) {
        this.agentId = agentId;
    }

    public String getSlug() {
        return slug;
    }

    public void setSlug(String slug) {
        this.slug = slug;
    }

    public ProgressData getData() {
        return data;
    }

    public void setData(ProgressData data) {
        this.data = data;
    }

    public String getToolUseID() {
        return toolUseID;
    }

    public void setToolUseID(String toolUseID) {
        this.toolUseID = toolUseID;
    }

    public String getParentToolUseID() {
        return parentToolUseID;
    }

    public void setParentToolUseID(String parentToolUseID) {
        this.parentToolUseID = parentToolUseID;
    }
}
