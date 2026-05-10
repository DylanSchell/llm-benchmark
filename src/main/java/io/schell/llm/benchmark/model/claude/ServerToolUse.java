package io.schell.llm.benchmark.model.claude;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Represents the {@code server_tool_use} field when it is an object.
 */
public class ServerToolUse {
    @JsonProperty("web_search_requests")
    private int webSearchRequests;

    @JsonProperty("web_fetch_requests")
    private int webFetchRequests;

    public int getWebSearchRequests() {
        return webSearchRequests;
    }

    public void setWebSearchRequests(int webSearchRequests) {
        this.webSearchRequests = webSearchRequests;
    }

    public int getWebFetchRequests() {
        return webFetchRequests;
    }

    public void setWebFetchRequests(int webFetchRequests) {
        this.webFetchRequests = webFetchRequests;
    }
}
