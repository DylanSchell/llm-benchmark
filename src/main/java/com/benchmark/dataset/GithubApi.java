package com.benchmark.dataset;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * GitHub API client for searching PRs and retrieving file changes.
 */
public class GithubApi {
    private static final Logger logger = LoggerFactory.getLogger(GithubApi.class);
    private static final ObjectMapper mapper = new ObjectMapper();
    private static final Duration TIMEOUT = Duration.ofSeconds(30);

    private final HttpClient httpClient;
    private final String token;
    private final AtomicInteger rateLimitRemaining;

    public GithubApi(String token) {
        this.token = token;
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(TIMEOUT)
                .build();
        this.rateLimitRemaining = new AtomicInteger(Integer.MAX_VALUE);
    }

    /**
     * Search for PRs matching a query.
     */
    public List<GithubPr> searchPrs(String query, int perPage) throws IOException, InterruptedException {
        String url = String.format(
                "https://api.github.com/search/issues?q=%s&type=pr&per_page=%d&sort=created&order=desc",
                encode(query), perPage);

        JsonNode root = getJson(url);
        JsonNode items = root.path("items");
        if (!items.isArray()) return List.of();

        List<GithubPr> prs = new ArrayList<>();
        for (JsonNode item : items) {
            prs.add(new GithubPr(
                    item.path("repository_url").asText(),
                    item.path("number").asInt(),
                    item.path("title").asText(),
                    item.path("body").asText(null),
                    item.path("created_at").asText(),
                    item.path("html_url").asText()
            ));
        }
        return prs;
    }

    /**
     * Get the linked issue for a PR (has the full problem description).
     */
    public GithubIssue getLinkedIssue(String owner, String repo, int prNumber) throws IOException, InterruptedException {
        // PRs have an issue_number field that points to the linked issue
        String prUrl = String.format(
                "https://api.github.com/repos/%s/%s/pulls/%d", owner, repo, prNumber);
        JsonNode prNode = getJson(prUrl);

        if (prNode.has("message")) return null;

        // Get the linked issue
        int issueNumber = prNode.path("number").asInt();
        String issueUrl = String.format(
                "https://api.github.com/repos/%s/%s/issues/%d", owner, repo, issueNumber);
        JsonNode issueNode = getJson(issueUrl);

        if (issueNode.has("message")) return null;

        return new GithubIssue(
                issueNode.path("title").asText(),
                issueNode.path("body").asText(null),
                issueNode.path("html_url").asText()
        );
    }

    /**
     * Get the linked PR from an issue (has the files changed).
     * Returns the PR number, or null if no PR is linked.
     */
    public Integer getLinkedPrNumber(String owner, String repo, int issueNumber) throws IOException, InterruptedException {
        String issueUrl = String.format(
                "https://api.github.com/repos/%s/%s/issues/%d", owner, repo, issueNumber);
        JsonNode issueNode = getJson(issueUrl);

        if (issueNode.has("message")) return null;

        // Check if issue has a linked PR
        JsonNode pullRequest = issueNode.path("pull_request");
        if (pullRequest.has("url")) {
            String prUrl = pullRequest.path("url").asText();
            // Extract PR number from URL: /repos/owner/repo/pulls/NUMBER
            String[] parts = prUrl.split("/");
            if (parts.length >= 1) {
                return Integer.parseInt(parts[parts.length - 1]);
            }
        }

        return null;
    }

    /**
     * Get the files changed in a PR.
     */
    public List<GithubFile> getPrFiles(String owner, String repo, int prNumber)
            throws IOException, InterruptedException {
        String url = String.format(
                "https://api.github.com/repos/%s/%s/pulls/%d/files",
                owner, repo, prNumber);

        JsonNode root = getJson(url);
        if (!root.isArray()) return List.of();

        List<GithubFile> files = new ArrayList<>();
        for (JsonNode node : root) {
            files.add(new GithubFile(
                    node.path("filename").asText(),
                    node.path("status").asText(),
                    node.path("additions").asInt(0),
                    node.path("deletions").asInt(0)
            ));
        }
        return files;
    }

    /**
     * Get the commits in a PR.
     */
    public GithubPrCommits getPrCommits(String owner, String repo, int prNumber)
            throws IOException, InterruptedException {
        // Get PR details
        String prUrl = String.format(
                "https://api.github.com/repos/%s/%s/pulls/%d", owner, repo, prNumber);
        JsonNode prNode = getJson(prUrl);

        if (prNode.has("message")) {
            logger.debug("PR not found or rate limited: {}/{}#{}", owner, repo, prNumber);
            return null;
        }

        String baseSha = prNode.path("base").path("sha").asText();
        String mergeCommitSha = prNode.path("merge_commit_sha").asText(null);

        // Get commits
        String commitsUrl = String.format(
                "https://api.github.com/repos/%s/%s/pulls/%d/commits", owner, repo, prNumber);
        JsonNode commitsNode = getJson(commitsUrl);

        if (!commitsNode.isArray() || commitsNode.size() == 0) {
            return null;
        }

        String headSha = commitsNode.get(commitsNode.size() - 1).path("sha").asText();

        return new GithubPrCommits(baseSha, headSha, mergeCommitSha);
    }

    /**
     * Check rate limit status.
     */
    public int getRateLimitRemaining() throws IOException, InterruptedException {
        JsonNode root = getJson("https://api.github.com/rate_limit");
        return root.path("rate").path("remaining").asInt(Integer.MAX_VALUE);
    }

    private JsonNode getJson(String url) throws IOException, InterruptedException {
        // Check rate limit
        int remaining = rateLimitRemaining.getAndSet(Integer.MAX_VALUE);
        if (remaining <= 10) {
            long waitTime = estimateRateLimitReset();
            logger.warn("Rate limit low ({} remaining). Waiting {}s...", remaining, waitTime / 1000);
            Thread.sleep(waitTime);
        }

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .timeout(TIMEOUT)
                .header("Authorization", "Bearer " + token)
                .header("Accept", "application/vnd.github.v3+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());

        // Handle rate limiting
        if (response.statusCode() == 403) {
            String resetHeader = response.headers().firstValue("x-ratelimit-reset").orElse(null);
            if (resetHeader != null) {
                long resetTime = Long.parseLong(resetHeader) * 1000 - System.currentTimeMillis();
                if (resetTime > 0) {
                    logger.warn("Rate limited. Waiting {}s...", resetTime / 1000);
                    Thread.sleep(resetTime + 1000);
                    return getJson(url); // Retry
                }
            }
        }

        if (response.statusCode() != 200) {
            throw new IOException(String.format("HTTP %d for %s",
                    response.statusCode(), url));
        }

        return mapper.readTree(response.body());
    }

    private long estimateRateLimitReset() {
        return 60000; // Default 1 minute wait
    }

    private String encode(String s) {
        return java.net.URLEncoder.encode(s, java.nio.charset.StandardCharsets.UTF_8);
    }

    /**
     * PR from search results.
     */
    public record GithubPr(String repositoryUrl, int number, String title,
                           String body, String createdAt, String htmlUrl) {
        public String getOwner() {
            return htmlUrl.split("/")[3];
        }
        public String getRepo() {
            return htmlUrl.split("/")[4];
        }
    }

    /**
     * File changed in a PR.
     */
    public record GithubFile(String filename, String status, int additions, int deletions) {}

    /**
     * Commit information for a PR.
     */
    public record GithubPrCommits(String baseSha, String headSha, String mergeCommitSha) {}

    /**
     * GitHub issue with problem description.
     */
    public record GithubIssue(String title, String body, String htmlUrl) {}
}
