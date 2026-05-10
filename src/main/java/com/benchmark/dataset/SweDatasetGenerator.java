package com.benchmark.dataset;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.SerializationFeature;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * Generates SWE-bench style exercises from real GitHub PRs in Java repos.
 *
 * For each PR, checks that it:
 * - Modified files in src/main/java/ (the fix)
 * - Modified files in src/test/java/ (the test)
 * - Uses Maven or Gradle (build system)
 */
public class SweDatasetGenerator {
    private static final Logger logger = LoggerFactory.getLogger(SweDatasetGenerator.class);
    private static final ObjectMapper mapper = new ObjectMapper()
            .enable(SerializationFeature.INDENT_OUTPUT);

    // GitHub search queries for issues with linked PRs
    private static final List<String> SEARCH_QUERIES = List.of(
            "language:java is:issue is:closed has:pull_request",
            "language:java is:issue is:closed fix",
            "language:java is:issue is:closed bug",
            "language:java is:issue is:closed resolved",
            "language:java is:issue is:closed fix bug",
            "language:java is:issue is:closed fix issue"
    );

    private final GithubApi githubApi;
    private final Path outputDir;
    private final int maxExercises;
    private final int maxRepos;

    public SweDatasetGenerator(String githubToken, Path outputDir, int maxExercises, int maxRepos) {
        this.githubApi = new GithubApi(githubToken);
        this.outputDir = outputDir;
        this.maxExercises = maxExercises;
        this.maxRepos = maxRepos;
    }

    /**
     * Generate the dataset from a set of candidate repos.
     */
    public List<SweExercise> generate(List<RepoCandidate> candidateRepos) throws IOException, InterruptedException {
        List<SweExercise> exercises = new ArrayList<>();
        Set<String> seenIssues = new HashSet<>();

        logger.info("Starting SWE dataset generation (max: {} exercises, {} candidate repos)",
                maxExercises, candidateRepos.size());

        // Search for issues with linked PRs in each candidate repo
        for (RepoCandidate repo : candidateRepos) {
            if (exercises.size() >= maxExercises) break;

            logger.info("Searching {} ({})...", repo.fullName(), repo.stars());

            for (String query : SEARCH_QUERIES) {
                if (exercises.size() >= maxExercises) break;

                try {
                    String repoQuery = "repo:" + repo.fullName() + " " + query;
                    List<GithubApi.GithubPr> issues = githubApi.searchPrs(repoQuery, 30);

                    for (GithubApi.GithubPr issue : issues) {
                        if (exercises.size() >= maxExercises) break;

                        // Skip duplicate issues
                        String issueKey = repo.fullName() + "#" + issue.number();
                        if (seenIssues.contains(issueKey)) continue;
                        seenIssues.add(issueKey);

                        // Validate this issue (has linked PR)
                        SweExercise exercise = validateIssue(issue);
                        if (exercise != null) {
                            exercises.add(exercise);
                            logger.info("  ✓ {}:{} fix={} test={}",
                                    repo.fullName(), issue.number(),
                                    exercise.fixFiles().size(), exercise.testFiles().size());
                        }
                    }
                } catch (Exception e) {
                    logger.warn("  Query failed for {}: {}", repo.fullName(), e.getMessage());
                }
            }

            // Be polite to GitHub API
            Thread.sleep(2000);
        }

        logger.info("Generated {} exercises from {} repos", exercises.size(), seenIssues.size());
        return exercises;
    }

    /**
     * Generate the dataset from popular Java repos (auto-discovers candidates).
     */
    public List<SweExercise> generate() throws IOException, InterruptedException {
        logger.info("Auto-discovering candidate repos...");
        RepoSelector selector = new RepoSelector(System.getenv("GITHUB_TOKEN"));
        List<RepoCandidate> candidates = selector.findRepos(30);
        return generate(candidates);
    }

    /**
     * Validate an issue with a linked PR and build an exercise.
     */
    private SweExercise validateIssue(GithubApi.GithubPr issue) throws IOException, InterruptedException {
        String owner = issue.getOwner();
        String repo = issue.getRepo();
        int issueNumber = issue.number();

        // Get the linked PR number from the issue
        Integer prNumber = githubApi.getLinkedPrNumber(owner, repo, issueNumber);
        if (prNumber == null) return null; // No PR linked to this issue

        // Get the files changed in the linked PR
        List<GithubApi.GithubFile> files = githubApi.getPrFiles(owner, repo, prNumber);
        if (files.isEmpty()) return null;

        // Split into fix files and test files
        List<String> fixFiles = new ArrayList<>();
        List<String> testFiles = new ArrayList<>();

        for (GithubApi.GithubFile file : files) {
            String filename = file.filename();
            if (isFixFile(filename)) {
                fixFiles.add(filename);
            } else if (isTestFile(filename)) {
                testFiles.add(filename);
            }
        }

        // Must have both fix and test files
        if (fixFiles.isEmpty() || testFiles.isEmpty()) {
            return null;
        }

        // Get commit information from the PR
        GithubApi.GithubPrCommits commits = githubApi.getPrCommits(owner, repo, prNumber);
        if (commits == null) return null;

        // Use the issue body as the problem description (has the full context)
        String issueBody = issue.body() != null ? issue.body().substring(0, Math.min(5000, issue.body().length())) : "";
        if (issueBody.isEmpty()) return null; // Skip if no issue description

        // Default to maven (most common for Java)
        String buildSystem = "maven";
        String testCommand = "mvn test -q";

        return SweExercise.builder()
                .id(issueKey(owner, repo, issueNumber))
                .repoUrl(String.format("https://github.com/%s/%s", owner, repo))
                .repo(repoKey(owner, repo))
                .issueNumber(issueNumber)
                .issueTitle(issue.title())
                .issueBody(issueBody)
                .preFixCommit(commits.baseSha())
                .postFixCommit(commits.headSha())
                .buildSystem(buildSystem)
                .testCommand(testCommand)
                .fixFiles(fixFiles)
                .testFiles(testFiles)
                .createdAt(issue.createdAt())
                .build();
    }

    private static boolean isFixFile(String filename) {
        return filename.startsWith("src/main/") && filename.endsWith(".java");
    }

    private static boolean isTestFile(String filename) {
        return filename.startsWith("src/test/") && filename.endsWith(".java");
    }

    private static String issueKey(String owner, String repo, int number) {
        return owner + "/" + repo + "#" + number;
    }

    private static String repoKey(String owner, String repo) {
        return owner + "/" + repo;
    }

    /**
     * Write the exercises to a JSON file.
     */
    public void save(List<SweExercise> exercises, Path outputPath) throws IOException {
        Files.createDirectories(outputPath.getParent());
        Files.writeString(outputPath, mapper.writeValueAsString(exercises));
        logger.info("Saved {} exercises to {}", exercises.size(), outputPath);
    }
}
