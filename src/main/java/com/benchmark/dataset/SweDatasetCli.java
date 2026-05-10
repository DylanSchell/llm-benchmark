package com.benchmark.dataset;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * CLI entry point for the SWE dataset generator.
 *
 * Usage:
 *   export GITHUB_TOKEN="ghp_xxx"
 *   java -cp target/classes:$CP com.benchmark.dataset.SweDatasetCli --output benchmark-data/java-swe.json --limit 50
 */
public class SweDatasetCli {
    private static final Logger logger = LoggerFactory.getLogger(SweDatasetCli.class);

    public static void main(String[] args) {
        CliOptions options = parseArgs(args);

        String token = System.getenv("GITHUB_TOKEN");
        if (token == null || token.isBlank()) {
            System.err.println("Error: GITHUB_TOKEN environment variable is required.");
            System.err.println("Get one at: https://github.com/settings/tokens");
            System.exit(1);
        }

        Path outputDir = options.output != null ? Paths.get(options.output) : Paths.get("java-swe.json");
        Path outputDirParent = outputDir.getParent() != null ? outputDir.getParent() : Paths.get(".");

        SweDatasetGenerator generator = new SweDatasetGenerator(
                token,
                outputDirParent,
                options.limit,
                options.maxRepos
        );

        try {
            System.err.println("Generating SWE dataset...");
            System.err.println("  Output: " + outputDir);
            System.err.println("  Limit: " + options.limit + " exercises");
            System.err.println("  Max repos: " + options.maxRepos);
            System.err.println();

            var exercises = generator.generate(); // Auto-discovers candidate repos

            if (exercises.isEmpty()) {
                System.err.println("No exercises found. Try adjusting search queries or increasing limits.");
                System.exit(1);
            }

            generator.save(exercises, outputDir);
            System.err.println("\nDone! Generated " + exercises.size() + " exercises.");

        } catch (Exception e) {
            System.err.println("Error: " + e.getMessage());
            e.printStackTrace();
            System.exit(1);
        }
    }

    private static CliOptions parseArgs(String[] args) {
        String output = null;
        int limit = 50;
        int maxRepos = 200;

        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--output") && i + 1 < args.length) {
                output = args[++i];
            } else if (args[i].equals("--limit") && i + 1 < args.length) {
                limit = Integer.parseInt(args[++i]);
            } else if (args[i].equals("--max-repos") && i + 1 < args.length) {
                maxRepos = Integer.parseInt(args[++i]);
            } else if (args[i].equals("--help")) {
                printUsage();
                System.exit(0);
            }
        }

        return new CliOptions(output, limit, maxRepos);
    }

    private record CliOptions(String output, int limit, int maxRepos) {}

    private static void printUsage() {
        System.out.println("Usage: java -jar claude-benchmark.jar dataset [options]");
        System.out.println();
        System.out.println("Options:");
        System.out.println("  --output <file>       Output JSON file (default: java-swe.json)");
        System.out.println("  --limit <n>           Max exercises to generate (default: 50)");
        System.out.println("  --max-repos <n>       Max repos to scan (default: 200)");
        System.out.println("  --help                Show this help");
        System.out.println();
        System.out.println("Requires: GITHUB_TOKEN environment variable");
    }
}
