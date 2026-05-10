package io.schell.llm.benchmark.model.claude;

import com.fasterxml.jackson.annotation.JsonSubTypes;
import com.fasterxml.jackson.annotation.JsonTypeInfo;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Base class for the JSON log entries stored in the *.jsonl files produced by the benchmark.
 * Each line is a separate JSON object. The "type" field determines which concrete subclass
 * should be used for deserialization.
 */
@JsonTypeInfo(use = JsonTypeInfo.Id.NAME, property = "type", visible = true)
@JsonSubTypes({
        @JsonSubTypes.Type(value = QueueOperationEntry.class, name = "queue-operation"),
        @JsonSubTypes.Type(value = UserEntry.class, name = "user"),
        @JsonSubTypes.Type(value = AssistantEntry.class, name = "assistant"),
        @JsonSubTypes.Type(value = SystemEntry.class, name = "system"),
        @JsonSubTypes.Type(value = Progress.class, name = "progress")
})
public abstract class LogEntry {
    @JsonProperty("uuid")
    protected String uuid;

    @JsonProperty("parentUuid")
    protected String parentUuid;

    @JsonProperty("isSidechain")
    protected Boolean isSidechain;

    @JsonProperty("userType")
    protected String userType;

    @JsonProperty("cwd")
    protected String cwd;

    @JsonProperty("sessionId")
    protected String sessionId;

    @JsonProperty("version")
    protected String version;

    @JsonProperty("gitBranch")
    protected String gitBranch;

    @JsonProperty("type")
    protected String type;

    @JsonProperty("timestamp")
    protected String timestamp;

    // Getters / Setters (generated for brevity)
    public String getUuid() { return uuid; }
    public void setUuid(String uuid) { this.uuid = uuid; }
    public String getParentUuid() { return parentUuid; }
    public void setParentUuid(String parentUuid) { this.parentUuid = parentUuid; }
    public Boolean getIsSidechain() { return isSidechain; }
    public void setIsSidechain(Boolean isSidechain) { this.isSidechain = isSidechain; }
    public String getUserType() { return userType; }
    public void setUserType(String userType) { this.userType = userType; }
    public String getCwd() { return cwd; }
    public void setCwd(String cwd) { this.cwd = cwd; }
    public String getSessionId() { return sessionId; }
    public void setSessionId(String sessionId) { this.sessionId = sessionId; }
    public String getVersion() { return version; }
    public void setVersion(String version) { this.version = version; }
    public String getGitBranch() { return gitBranch; }
    public void setGitBranch(String gitBranch) { this.gitBranch = gitBranch; }
    public String getType() { return type; }
    public void setType(String type) { this.type = type; }
    public String getTimestamp() { return timestamp; }
    public void setTimestamp(String timestamp) { this.timestamp = timestamp; }
}
