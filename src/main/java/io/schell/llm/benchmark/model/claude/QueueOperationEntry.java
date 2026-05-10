package io.schell.llm.benchmark.model.claude;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Represents a queue-operation entry in the benchmark log.
 */
public class QueueOperationEntry extends LogEntry {
    @JsonProperty("operation")
    private String operation;

    @JsonProperty("content")
    private String content;

    public String getOperation() {
        return operation;
    }

    public void setOperation(String operation) {
        this.operation = operation;
    }

    public String getContent() {
        return content;
    }

    public void setContent(String content) {
        this.content = content;
    }
}
