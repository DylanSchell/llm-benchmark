package com.benchmark.model.pi;

import com.fasterxml.jackson.annotation.JsonSubTypes;
import com.fasterxml.jackson.annotation.JsonTypeInfo;

@JsonTypeInfo(use = JsonTypeInfo.Id.NAME, property = "type", visible = true)
@JsonSubTypes({
        @JsonSubTypes.Type(value = PiToolCall.class, name = "toolCall"),
        @JsonSubTypes.Type(value = PiText.class, name="text"),
        @JsonSubTypes.Type(value = PiThinking.class, name = "thinking")
})
public class PiContent {
    public String type;
}
