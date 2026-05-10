package com.benchmark.dataset;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.List;

/**
 * Represents a SWE-bench style exercise generated from a GitHub PR.
 */
public record SweExercise(
    String id,
    String repoUrl,
    String repo,
    @JsonProperty("issue_number") int issueNumber,
    @JsonProperty("issue_title") String issueTitle,
    @JsonProperty("issue_body") String issueBody,
    @JsonProperty("pre_fix_commit") String preFixCommit,
    @JsonProperty("post_fix_commit") String postFixCommit,
    @JsonProperty("build_system") String buildSystem,
    @JsonProperty("test_command") String testCommand,
    @JsonProperty("fix_files") List<String> fixFiles,
    @JsonProperty("test_files") List<String> testFiles,
    @JsonProperty("created_at") String createdAt
) {
    public static Builder builder() {
        return new Builder();
    }

    public static class Builder {
        private String id;
        private String repoUrl;
        private String repo;
        private int issueNumber;
        private String issueTitle;
        private String issueBody;
        private String preFixCommit;
        private String postFixCommit;
        private String buildSystem;
        private String testCommand;
        private List<String> fixFiles;
        private List<String> testFiles;
        private String createdAt;

        public Builder id(String id) { this.id = id; return this; }
        public Builder repoUrl(String repoUrl) { this.repoUrl = repoUrl; return this; }
        public Builder repo(String repo) { this.repo = repo; return this; }
        public Builder issueNumber(int issueNumber) { this.issueNumber = issueNumber; return this; }
        public Builder issueTitle(String issueTitle) { this.issueTitle = issueTitle; return this; }
        public Builder issueBody(String issueBody) { this.issueBody = issueBody; return this; }
        public Builder preFixCommit(String preFixCommit) { this.preFixCommit = preFixCommit; return this; }
        public Builder postFixCommit(String postFixCommit) { this.postFixCommit = postFixCommit; return this; }
        public Builder buildSystem(String buildSystem) { this.buildSystem = buildSystem; return this; }
        public Builder testCommand(String testCommand) { this.testCommand = testCommand; return this; }
        public Builder fixFiles(List<String> fixFiles) { this.fixFiles = fixFiles; return this; }
        public Builder testFiles(List<String> testFiles) { this.testFiles = testFiles; return this; }
        public Builder createdAt(String createdAt) { this.createdAt = createdAt; return this; }

        public SweExercise build() {
            return new SweExercise(id, repoUrl, repo, issueNumber, issueTitle, issueBody,
                    preFixCommit, postFixCommit, buildSystem, testCommand, fixFiles, testFiles, createdAt);
        }
    }
}
