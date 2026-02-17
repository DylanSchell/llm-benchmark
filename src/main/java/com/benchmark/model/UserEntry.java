package com.benchmark.model;

import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.databind.JsonNode;

/**
 * Represents a user entry in the benchmark log (type "user").
 * It contains a nested {@link Message} object that holds the actual content sent by the user.
 */
public class UserEntry extends LogEntry {
    @JsonProperty("sourceToolAssistantUUID")
    private String sourceToolAssistantUUID;

    @JsonProperty("permissionMode")
    private String permissionMode;

    public String getSourceToolAssistantUUID() {
        return sourceToolAssistantUUID;
    }

    public void setSourceToolAssistantUUID(String sourceToolAssistantUUID) {
        this.sourceToolAssistantUUID = sourceToolAssistantUUID;
    }

    public String getPermissionMode() {
        return permissionMode;
    }

    public void setPermissionMode(String permissionMode) {
        this.permissionMode = permissionMode;
    }
    @JsonProperty("toolUseResult")
    private JsonNode toolUseResult;

    @JsonProperty("agentId")
    private String agentId;

    @JsonProperty("todos")
    private JsonNode todos;

    public JsonNode getToolUseResult() {
        return toolUseResult;
    }

    public void setToolUseResult(JsonNode toolUseResult) {
        this.toolUseResult = toolUseResult;
    }

    public String getAgentId() {
        return agentId;
    }

    public void setAgentId(String agentId) {
        this.agentId = agentId;
    }

    public JsonNode getTodos() {
        return todos;
    }

    public void setTodos(JsonNode todos) {
        this.todos = todos;
    }

    @JsonProperty("slug")
    private String slug;
    @JsonProperty("message")
    private Message message;

    public String getSlug() {
        return slug;
    }

    public void setSlug(String slug) {
        this.slug = slug;
    }

    public Message getMessage() {
        return message;
    }

    public void setMessage(Message message) {
        this.message = message;
    }
}
