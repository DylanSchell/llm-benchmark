package com.benchmark.model;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * A simple Content wrapper for when content is a plain string value.
 */
public class StringContent extends Content {
    @JsonProperty("text")
    private String text;

    public StringContent() {
        this.type = "text";
    }

    public StringContent(String value) {
        this.type = "text";
        this.text = value;
    }

    public String getText() {
        return text;
    }

    public void setText(String text) {
        this.text = text;
    }
}
