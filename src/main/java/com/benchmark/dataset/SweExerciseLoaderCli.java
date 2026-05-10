package com.benchmark.dataset;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * CLI for loading SWE exercises into the benchmark directory.
 *
 * Usage:
 *   java -cp target/classes:$CP com.benchmark.dataset.SweExerciseLoaderCli \
 *     --dataset benchmark-data/java-swe.json \
 *     --output benchmark-exercises/
 */
public class SweExerciseLoaderCli {
    private static final Logger logger = LoggerFactory.getLogger(SweExerciseLoaderCli.class);

    public static void main(String[] args) {
        CliOptions options = parseArgs(args);

        Path datasetFile = options.dataset != null ? Paths.get(options.dataset) : Paths.get("benchmark-data/java-swe.json");
        Path outputDir = options.output != null ? Paths.get(options.output) : Paths.get("benchmark-exercises/");

        try {
            System.err.println("Loading SWE exercises...");
            System.err.println("  Dataset: " + datasetFile);
            System.err.println("  Output: " + outputDir);
            System.err.println();

            SweExerciseLoader loader = new SweExerciseLoader(outputDir);
            loader.load(datasetFile);

            System.err.println("\nDone! Exercises loaded to " + outputDir);

        } catch (Exception e) {
            System.err.println("Error: " + e.getMessage());
            e.printStackTrace();
            System.exit(1);
        }
    }

    private static CliOptions parseArgs(String[] args) {
        String dataset = null;
        String output = null;

        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--dataset") && i + 1 < args.length) {
                dataset = args[++i];
            } else if (args[i].equals("--output") && i + 1 < args.length) {
                output = args[++i];
            } else if (args[i].equals("--help")) {
                printUsage();
                System.exit(0);
            }
        }

        return new CliOptions(dataset, output);
    }

    private record CliOptions(String dataset, String output) {}

    private static void printUsage() {
        System.out.println("Usage: java -cp target/classes:$CP com.benchmark.dataset.SweExerciseLoaderCli [options]");
        System.out.println();
        System.out.println("Options:");
        System.out.println("  --dataset <file>    Path to SWE dataset JSON (default: benchmark-data/java-swe.json)");
        System.out.println("  --output <dir>      Output directory for exercises (default: benchmark-exercises/)");
        System.out.println("  --help              Show this help");
    }
}
