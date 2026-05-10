package io.schell.llm.benchmark.model.claude;

import com.fasterxml.jackson.annotation.JsonSubTypes;
import com.fasterxml.jackson.annotation.JsonTypeInfo;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Base class for the elements that appear in the {@code content} array of a {@link Message}.
 * The concrete subclass is selected based on the {@code type} property of the JSON object.
 */
@JsonTypeInfo(use = JsonTypeInfo.Id.NAME, property = "type", visible = true, defaultImpl = TextContent.class)
@JsonSubTypes({
        @JsonSubTypes.Type(value = TextContent.class, name = "text"),
        @JsonSubTypes.Type(value = ThinkingContent.class, name = "thinking"),
        @JsonSubTypes.Type(value = ToolUseContent.class, name = "tool_use")
})
public abstract class Content {
    @JsonProperty("type")
    protected String type;

    public String getType() {
        return type;
    }

    public void setType(String type) {
        this.type = type;
    }
}
