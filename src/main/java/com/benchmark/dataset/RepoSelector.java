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
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.stream.Collectors;

/**
 * Evaluates and scores GitHub repositories for SWE benchmark suitability.
 *
 * Scoring criteria:
 * - Stars (popularity): 0-40 points
 * - Library detection: 0-20 points
 * - Recent activity: 0-15 points
 * - Contributor diversity: 0-10 points
 * - License quality: 0-5 points
 * - PR quality indicators: 0-10 points
 */
public class RepoSelector {
    private static final Logger logger = LoggerFactory.getLogger(RepoSelector.class);
    private static final ObjectMapper mapper = new ObjectMapper();
    private static final Duration TIMEOUT = Duration.ofSeconds(30);

    private final HttpClient httpClient;
    private final String token;
    private final AtomicInteger rateLimitRemaining;

    // Excluded repos (too big, too well-known, likely in training data)
    private static final Set<String> EXCLUDED_REPOS = Set.of(
            "spring-projects/spring-framework",
            "apache/kafka", "apache/spark", "apache/flink", "apache/commons-lang",
            "google/guava", "square/retrofit", "square/okhttp",
            "mockito/mockito", "junit-team/junit5",
            "testcontainers/testcontainers-java",
            "hibernate/hibernate-orm", "eclipse-jetty/jetty.project",
            "Netflix/astyanax", "redisson/redisson",
            "alibaba/fastjson", "alibaba/druid",
            "apache/struts", "apache/velocity",
            "google/gson", "google/error-prone",
            "linkedin/ambry", "line/centraldogma"
    );

    // Library-like keywords in repo name/description
    private static final Set<String> LIBRARY_KEYWORDS = Set.of(
            "lib", "library", "sdk", "client", "driver", "connector", "processor",
            "parser", "validator", "serializer", "formatter", "utils", "helper",
            "framework", "tool", "kit", "engine", "core", "common", "api",
            "annotation", "extension", "plugin", "module", "component",
            "http", "json", "yaml", "xml", "csv", "log", "cache", "pool",
            "scheduler", "dispatcher", "handler", "mapper", "converter",
            "builder", "factory", "provider", "registry", "config", "template"
    );

    // Exclude keywords (web apps, Android, etc.)
    private static final Set<String> EXCLUDE_KEYWORDS = Set.of(
            "android", "mobile", "app", "website", "demo", "example", "tutorial",
            "boilerplate", "starter", "scaffold", "template", "sample"
    );

    public RepoSelector(String token) {
        this.token = token;
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(TIMEOUT)
                .build();
        this.rateLimitRemaining = new AtomicInteger(Integer.MAX_VALUE);
    }

    /**
     * Find popular Java library repos suitable for benchmarking.
     */
    public List<RepoCandidate> findRepos(int limit) throws IOException, InterruptedException {
        List<RepoCandidate> candidates = new ArrayList<>();

        // Search for popular Java repos
        String[] queries = {
            "language:java stars:>100 pushed:>2023-01-01",
            "language:java stars:>50 pushed:>2024-01-01",
            "language:java stars:>20 pushed:>2024-06-01",
        };

        int reposFound = 0;
        for (String query : queries) {
            if (candidates.size() >= limit) break;

            logger.info("Searching: {}", query);
            List<JsonNode> repos = searchRepos(query, 100);

            for (JsonNode repo : repos) {
                if (candidates.size() >= limit) break;

                String fullName = repo.path("full_name").asText();
                if (EXCLUDED_REPOS.contains(fullName)) continue;
                if (reposFound++ > 500) break; // Safety limit

                RepoCandidate candidate = evaluate(repo);
                if (candidate != null && candidate.isSuitable()) {
                    candidates.add(candidate);
                    logger.info("  ✓ {} stars={} score={}", fullName, candidate.stars(), candidate.score());
                }
            }

            Thread.sleep(3000);
        }

        // Sort by score descending
        candidates.sort(Comparator.comparingDouble(RepoCandidate::score).reversed());
        return candidates.subList(0, Math.min(limit, candidates.size()));
    }

    /**
     * Evaluate a single repo from GitHub API response.
     */
    private RepoCandidate evaluate(JsonNode repo) throws IOException, InterruptedException {
        String fullName = repo.path("full_name").asText();
        String description = repo.path("description").asText(null);
        String name = repo.path("name").asText();

        // Quick library detection from name/description
        boolean isLibrary = isLibraryCandidate(name, description);
        if (!isLibrary) return null; // Skip non-library repos

        // Get detailed stats
        JsonNode details = getRepoDetails(repo.path("url").asText());
        if (details == null) return null;

        // Check for README (required for build instructions)
        boolean hasReadme = hasReadme(details);
        if (!hasReadme) {
            logger.info("  Skipping {} - no README or no build instructions", fullName);
            return null;
        }

        int stars = details.path("stargazers_count").asInt(0);
        int forks = details.path("forks_count").asInt(0);
        int openIssues = details.path("open_issues_count").asInt(0);
        int contributors = details.path("subscribers_count").asInt(0);

        // Get contributor count (separate API call)
        int contributorCount = getContributorCount(fullName);

        // Count recently merged PRs (last 90 days)
        int recentMergedPrs = getRecentMergedPrCount(fullName);

        // Calculate score
        double score = calculateScore(stars, forks, openIssues, contributorCount, recentMergedPrs, details);

        return RepoCandidate.builder()
                .owner(details.path("owner").path("login").asText())
                .name(name)
                .fullName(fullName)
                .url(details.path("html_url").asText())
                .description(description)
                .stars(stars)
                .forks(forks)
                .openIssues(openIssues)
                .contributors(contributorCount)
                .language(details.path("language").asText())
                .license(details.path("license").path("spdx_id").asText(null))
                .hasReadme(hasReadme)
                .isLibrary(isLibrary)
                .recentMergedPrs(recentMergedPrs)
                .score(score)
                .topBranches(new String[]{})
                .build();
    }

    /**
     * Check if repo has a README file.
     */
    private boolean hasReadme(JsonNode repoDetails) throws IOException, InterruptedException {
        String owner = repoDetails.path("owner").path("login").asText();
        String name = repoDetails.path("name").asText();

        // Try common README filenames
        String[] readmeFiles = {
            "README.md", "README", "readme.md", "README.rst", "README.txt"
        };

        for (String readme : readmeFiles) {
            try {
                String url = String.format(
                        "https://api.github.com/repos/%s/%s/contents/%s",
                        owner, name, readme);
                JsonNode content = getJson(url);
                if (!content.has("message")) {
                    return true;
                }
            } catch (Exception e) {
                // File doesn't exist, try next
            }
        }

        return false;
    }

    /**
     * Check if text appears to be in English (not CJK).
     */
    private boolean isEnglish(String text) {
        // Count CJK characters (Chinese, Japanese, Korean)
        int cjkCount = 0;
        int totalChars = 0;
        
        for (char c : text.toCharArray()) {
            if (c >= 0x2E80 && c <= 0x9FFF) {
                cjkCount++;
            }
            totalChars++;
        }
        
        // If more than 20% CJK, likely not English
        return totalChars == 0 || (cjkCount * 100 / totalChars) < 20;
    }

    /**
     * Check if README has build instructions.
     */
    private boolean hasBuildInstructions(String readme) {
        String lower = readme.toLowerCase();
        
        // Common build-related keywords
        String[] buildKeywords = {
            "mvn", "gradle", "maven", "sbt", "npm", "yarn",
            "build", "compile", "install", "test", "run",
            "cargo", "go build", "make",
            "getting started", "quick start", "installation"
        };
        
        for (String keyword : buildKeywords) {
            if (lower.contains(keyword)) {
                return true;
            }
        }
        
        return false;
    }

    /**
     * Quick check if repo looks like a library.
     */
    private boolean isLibraryCandidate(String name, String description) {
        if (name == null && description == null) return false;

        String text = (name + " " + (description != null ? description : "")).toLowerCase();

        // Check for exclude keywords first
        for (String keyword : EXCLUDE_KEYWORDS) {
            if (text.contains(keyword)) return false;
        }

        // Check for library keywords
        for (String keyword : LIBRARY_KEYWORDS) {
            if (text.contains(keyword)) return true;
        }

        return false;
    }

    /**
     * Calculate repo suitability score.
     */
    private double calculateScore(int stars, int forks, int openIssues, int contributors,
                                   int recentMergedPrs, JsonNode details) {
        double score = 0;

        // Stars (popularity) (0-30 points)
        if (stars > 1000) score += 30;
        else if (stars > 500) score += 25;
        else if (stars > 200) score += 20;
        else if (stars > 100) score += 15;
        else if (stars > 50) score += 10;
        else if (stars > 20) score += 5;

        // Fork ratio (activity indicator) (0-5 points)
        if (forks > 0) {
            double ratio = (double) forks / stars;
            if (ratio > 0.1) score += 5;
            else if (ratio > 0.05) score += 3;
            else score += 1;
        }

        // Recent merged PRs (activity) (0-25 points)
        if (recentMergedPrs > 50) score += 25;
        else if (recentMergedPrs > 30) score += 20;
        else if (recentMergedPrs > 20) score += 15;
        else if (recentMergedPrs > 10) score += 10;
        else if (recentMergedPrs > 5) score += 5;

        // Recent activity - last push (0-10 points)
        String pushedAt = details.path("pushed_at").asText(null);
        if (pushedAt != null) {
            Instant pushed = Instant.parse(pushedAt);
            long daysSincePushed = ChronoUnit.DAYS.between(pushed, Instant.now());
            if (daysSincePushed < 30) score += 10;
            else if (daysSincePushed < 90) score += 7;
            else if (daysSincePushed < 180) score += 4;
            else score += 1;
        }

        // Contributor diversity (0-10 points)
        if (contributors > 50) score += 10;
        else if (contributors > 20) score += 7;
        else if (contributors > 10) score += 5;
        else if (contributors > 5) score += 3;
        else score += 1;

        // License (0-5 points)
        String license = details.path("license").path("spdx_id").asText(null);
        if (license != null && (license.contains("MIT") || license.contains("Apache") ||
                license.contains("BSD") || license.contains("MPL"))) {
            score += 5;
        } else if (license != null) {
            score += 3;
        }

        return score;
    }

    /**
     * Search GitHub for repos matching a query.
     */
    private List<JsonNode> searchRepos(String query, int perPage) throws IOException, InterruptedException {
        String url = String.format(
                "https://api.github.com/search/repositories?q=%s&sort=stars&order=desc&per_page=%d",
                encode(query), perPage);

        JsonNode root = getJson(url);
        JsonNode items = root.path("items");
        if (!items.isArray()) return List.of();

        List<JsonNode> repos = new ArrayList<>();
        for (JsonNode item : items) {
            // Only include Java repos
            if ("Java".equals(item.path("language").asText(null))) {
                repos.add(item);
            }
        }
        return repos;
    }

    /**
     * Get detailed repo information.
     */
    private JsonNode getRepoDetails(String apiUrl) throws IOException, InterruptedException {
        return getJson(apiUrl);
    }

    /**
     * Get contributor count for a repo.
     */
    private int getContributorCount(String fullName) throws IOException, InterruptedException {
        String url = String.format(
                "https://api.github.com/repos/%s/contributors?per_page=1", fullName);
        JsonNode root = getJson(url);
        return root.isArray() ? root.size() : 0;
    }

    /**
     * Count PRs merged in the last 90 days.
     */
    private int getRecentMergedPrCount(String fullName) throws IOException, InterruptedException {
        String since = java.time.LocalDate.now().minusDays(90).toString();
        String query = "repo:" + fullName + " type:pr is:merged merged:>=" + since;
        String url = String.format(
                "https://api.github.com/search/issues?q=%s&per_page=1",
                encode(query));
        JsonNode root = getJson(url);
        return root.path("total_count").asInt(0);
    }

    private JsonNode getJson(String url) throws IOException, InterruptedException {
        int remaining = rateLimitRemaining.getAndSet(Integer.MAX_VALUE);
        if (remaining <= 10) {
            Thread.sleep(60000);
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

        if (response.statusCode() == 403) {
            String resetHeader = response.headers().firstValue("x-ratelimit-reset").orElse(null);
            if (resetHeader != null) {
                long resetTime = Long.parseLong(resetHeader) * 1000 - System.currentTimeMillis();
                if (resetTime > 0) {
                    Thread.sleep(resetTime + 1000);
                    return getJson(url);
                }
            }
        }

        if (response.statusCode() != 200) {
            throw new IOException(String.format("HTTP %d for %s", response.statusCode(), url));
        }

        return mapper.readTree(response.body());
    }

    private String encode(String s) {
        return java.net.URLEncoder.encode(s, java.nio.charset.StandardCharsets.UTF_8);
    }
}
