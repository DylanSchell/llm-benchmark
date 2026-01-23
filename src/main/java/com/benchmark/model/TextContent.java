package com.benchmark.model;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Content element representing plain text within a message.
 */
public class TextContent extends Content {
    @JsonProperty("text")
    private String text;

    @JsonProperty("tool_use_id")
    private String toolUseId;

    @JsonProperty("content")
    private Object content;

    @JsonProperty("is_error")
    private Boolean isError;

    public String getText() {
        return text;
    }

    public void setText(String text) {
        this.text = text;
    }

    public String getToolUseId() {
        return toolUseId;
    }

    public void setToolUseId(String toolUseId) {
        this.toolUseId = toolUseId;
    }

    public Object getContent() {
        return content;
    }

    public void setContent(Object content) {
        this.content = content;
    }

    public Boolean getIsError() {
        return isError;
    }

    public void setIsError(Boolean isError) {
        this.isError = isError;
    }
}
