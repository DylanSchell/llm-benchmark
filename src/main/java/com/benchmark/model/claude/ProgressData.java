package com.benchmark.model.claude;

import com.fasterxml.jackson.annotation.JsonSubTypes;
import com.fasterxml.jackson.annotation.JsonTypeInfo;

@JsonTypeInfo(use = JsonTypeInfo.Id.NAME, property = "type", visible = true)
@JsonSubTypes({
        @JsonSubTypes.Type(value = BashProgress.class, name = "bash_progress"),
        @JsonSubTypes.Type(value = HookProgress.class, name = "hook_progress"),
        @JsonSubTypes.Type(value = WaitingForTask.class, name = "waiting_for_task")
})
public class ProgressData {
    private String type;
    private String waiting_for_task;

    public String getType() {
        return type;
    }

    public void setType(String type) {
        this.type = type;
    }
}
