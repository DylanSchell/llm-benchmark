package io.schell.llm.benchmark.model.pi;

import com.fasterxml.jackson.databind.JsonNode;

public class PiMessageMessage {
    public String api;
    public String provider;
    public String model;
    public String role;
    public PiContent[] content;
    public String timestamp;
    public PiUsage usage;
    public String stopReason;
    public String toolCallId;
    public String toolName;
    public boolean isError;
    public JsonNode details;
    public String responseId;
    public String errorMessage;

}
