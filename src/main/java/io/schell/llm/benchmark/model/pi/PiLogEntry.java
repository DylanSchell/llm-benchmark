package io.schell.llm.benchmark.model.pi;

import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.annotation.JsonSubTypes;
import com.fasterxml.jackson.annotation.JsonTypeInfo;

@JsonTypeInfo(use = JsonTypeInfo.Id.NAME, property = "type", visible = true)
@JsonSubTypes({
        @JsonSubTypes.Type(value = PiSession.class, name = "session"),
        @JsonSubTypes.Type(value = PiModelChange.class, name = "model_change"),
        @JsonSubTypes.Type(value = PiThinkingLevelChange.class, name = "thinking_level_change"),
        @JsonSubTypes.Type(value = PiMessage.class, name = "message"),
})
public class PiLogEntry {
    @JsonProperty("type")
    protected String type;
    @JsonProperty("id")
    public String id;
    @JsonProperty("parentId")
    public String parentId;
    @JsonProperty("timestamp")
    public String timestamp;
}
