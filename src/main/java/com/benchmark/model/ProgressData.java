package com.benchmark.model;

import com.fasterxml.jackson.annotation.JsonSubTypes;
import com.fasterxml.jackson.annotation.JsonTypeInfo;

@JsonTypeInfo(use = JsonTypeInfo.Id.NAME, property = "type", visible = true)
@JsonSubTypes({
        @JsonSubTypes.Type(value = BashProgress.class, name = "bash_progress"),
        @JsonSubTypes.Type(value = HookProgress.class, name = "hook_progress"),
})
public class ProgressData {
    private String type;


    public String getType() {
        return type;
    }

    public void setType(String type) {
        this.type = type;
    }


}
