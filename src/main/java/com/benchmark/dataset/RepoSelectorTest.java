package com.benchmark.dataset;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/**
 * Quick test runner for RepoSelector.
 */
public class RepoSelectorTest {
    private static final Logger logger = LoggerFactory.getLogger(RepoSelectorTest.class);
    private static final ObjectMapper mapper = new ObjectMapper();

    public static void main(String[] args) throws Exception {
        String token = System.getenv("GITHUB_TOKEN");
        if (token == null || token.isBlank()) {
            System.err.println("Error: GITHUB_TOKEN environment variable is required.");
            System.err.println("Get one at: https://github.com/settings/tokens");
            System.exit(1);
        }

        RepoSelector selector = new RepoSelector(token);
        int limit = Integer.parseInt(System.getProperty("limit", "10"));

        System.err.println("Searching for Java library repos (limit: " + limit + ")...");
        System.err.println();

        List<RepoCandidate> repos = selector.findRepos(limit);

        System.out.println("=== Top " + repos.size() + " Candidate Repos ===\n");
        System.out.printf("%-4s %-40s %6s %6s %6s %6s  %s\n",
                "#", "Repo", "Stars", "Forks", "PRs/90d", "Score", "Description");
        System.out.println("-".repeat(120));

        for (int i = 0; i < repos.size(); i++) {
            RepoCandidate r = repos.get(i);
            System.out.printf("%-4d %-40s %6d %6d %6d %6.0f  %s\n",
                    i + 1,
                    r.fullName(),
                    r.stars(),
                    r.forks(),
                    r.recentMergedPrs(),
                    r.score(),
                    (r.description() != null && r.description().length() > 50)
                            ? r.description().substring(0, 47) + "..."
                            : r.description()
            );
        }

        // Save full results
        Path output = Path.of("candidate-repos.json");
        Files.writeString(output, mapper.writerWithDefaultPrettyPrinter().writeValueAsString(repos));
        System.err.println("\nSaved full results to " + output);
    }
}
