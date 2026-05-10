package io.schell.llm.benchmark.model.claude;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Represents the container field in a message.
 */
public class Container {
    @JsonProperty("command")
    private String command;

    @JsonProperty("session_id")
    private String sessionId;

    public String getCommand() {
        return command;
    }

    public void setCommand(String command) {
        this.command = command;
    }

    public String getSessionId() {
        return sessionId;
    }

    public void setSessionId(String sessionId) {
        this.sessionId = sessionId;
    }
}
