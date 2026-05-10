package io.schell.llm.benchmark.model.claude;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Represents an assistant entry in the benchmark log (type "assistant").
 * It contains a nested {@link Message} object that holds the assistant's response.
 */
public class AssistantEntry extends LogEntry {

    @JsonProperty("error")
    private String error;

    public String getError() {
        return error;
    }

    public void setError(String error) {
        this.error = error;
    }
    @JsonProperty("slug")
    private String slug;

    public String getSlug() {
        return slug;
    }

    public void setSlug(String slug) {
        this.slug = slug;
    }
    @JsonProperty("message")
    private Message message;

    @JsonProperty("isApiErrorMessage")
    private boolean isApiErrorMessage;

    @JsonProperty("agentId")
    private String agentId;

    public Message getMessage() {
        return message;
    }

    public void setMessage(Message message) {
        this.message = message;
    }

    public boolean isApiErrorMessage() {
        return isApiErrorMessage;
    }

    public void setApiErrorMessage(boolean apiErrorMessage) {
        isApiErrorMessage = apiErrorMessage;
    }

    public String getAgentId() {
        return agentId;
    }

    public void setAgentId(String agentId) {
        this.agentId = agentId;
    }
}
