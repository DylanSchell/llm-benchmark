package io.schell.llm.benchmark.model.claude;

import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.databind.annotation.JsonDeserialize;

import java.util.List;

/**
 * Represents the "message" field present in user and assistant log entries.
 * The "content" can be either a single content object or an array of objects
 * describing rich content (text, thinking, tool usage, etc.).
 */

public class Message {

    @JsonProperty("context_management")
    private ContextManagement contextManagement;

    public ContextManagement getContextManagement() {
        return contextManagement;
    }

    public void setContextManagement(ContextManagement contextManagement) {
        this.contextManagement = contextManagement;
    }
    @JsonProperty("id")
    private String id;

    @JsonProperty("type")
    private String type;

    @JsonProperty("role")
    private String role;

    @JsonProperty("model")
    private String model;

    @JsonProperty("stop_reason")
    private String stopReason;

    @JsonProperty("stop_sequence")
    private String stopSequence;

    @JsonProperty("usage")
    private Usage usage;

    @JsonProperty("container")
    private Container container;

    @JsonProperty("content")
    @JsonDeserialize(using = ContentListDeserializer.class)
    private List<Content> content;

    public Container getContainer() {
        return container;
    }

    public void setContainer(Container container) {
        this.container = container;
    }

    public String getId() {
        return id;
    }

    public void setId(String id) {
        this.id = id;
    }

    public String getType() {
        return type;
    }

    public void setType(String type) {
        this.type = type;
    }

    public String getRole() {
        return role;
    }

    public void setRole(String role) {
        this.role = role;
    }

    public String getModel() {
        return model;
    }

    public Usage getUsage() {
        return usage;
    }

    public void setModel(String model) {
        this.model = model;
    }

    public void setUsage(Usage usage) {
        this.usage = usage;
    }

    public String getStopReason() {
        return stopReason;
    }

    public void setStopReason(String stopReason) {
        this.stopReason = stopReason;
    }

    public String getStopSequence() {
        return stopSequence;
    }

    public void setStopSequence(String stopSequence) {
        this.stopSequence = stopSequence;
    }

    public List<Content> getContent() {
        return content;
    }

    public void setContent(List<Content> content) {
        this.content = content;
    }
}
