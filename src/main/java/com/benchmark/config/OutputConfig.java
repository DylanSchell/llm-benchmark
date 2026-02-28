package com.benchmark.config;

import com.fasterxml.jackson.annotation.JsonProperty;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

/**
 * Output configuration for the benchmark runner.
 */
public class OutputConfig {

    @JsonProperty("results_dir")
    private String resultsDir = "../benchmark-results";

    @JsonProperty("log_level")
    private String logLevel = "INFO";

    public String getResultsDir() {
        return resultsDir;
    }

    public String getLogLevel() {
        return logLevel;
    }

    // Setters
    public void setResultsDir(String resultsDir) {
        this.resultsDir = resultsDir;
    }

    public void setLogLevel(String logLevel) {
        this.logLevel = logLevel;
    }

    /**
     * Gets the results directory for a specific benchmark run.
     * Constructs subdirectory as: <agent>-<model>-<languages>-<run#>
     * Languages are sorted alphabetically and joined with hyphens.
     *
     * @param agentName   The agent name (reference or claude)
     * @param model       The model name (null for default)
     * @param languages   Array of language names (sorted alphabetically)
     * @return The full path to the results directory
     */
    public String getResultsDir(String agentName, String model, String[] languages) {
        // Build languages string - sort alphabetically
        List<String> sortedLangs = new ArrayList<>();
        if (languages != null) {
            sortedLangs.addAll(Arrays.asList(languages));
        }
        sortedLangs.sort(String.CASE_INSENSITIVE_ORDER);
        String languagesPart = String.join("-", sortedLangs);

        // Build agent string - handle null
        String agentPart = agentName != null ? agentName : "unknown";

        // Build model string - handle null
        String modelPart = model != null ? model : "default";

        // Construct the subdirectory name
        // for now exclude the languages part, don't think we need it any more
        String subdir = String.format("%s-%s", agentPart, modelPart);

        // Check existing subdirectories to find the next run number
        int nextRunNumber = getNextRunNumber(subdir);
        if (nextRunNumber > 1) {
            subdir = subdir + "-r" + nextRunNumber;
        }

        return resultsDir + "/" + subdir;
    }

    /**
     * Finds the next available run number for a given subdirectory pattern.
     * Scans the results_dir for existing subdirectories matching the pattern
     * and returns the next number.
     *
     * @param subdirBase The base subdirectory name (without run number suffix)
     * @return The next run number (1 if no existing runs)
     */
    private int getNextRunNumber(String subdirBase) {
        Path resultsPath = Paths.get(resultsDir);
        if (!Files.exists(resultsPath)) {
            return 1;
        }

        Pattern pattern = Pattern.compile("^" + Pattern.quote(subdirBase) + "(-r(\\d+))?$");
        final int[] maxRunNumber = {0};

        try (Stream<Path> paths = Files.list(resultsPath)) {
            paths.forEach(path -> {
                String name = path.getFileName().toString();
                Matcher matcher = pattern.matcher(name);
                if (matcher.matches()) {
                    String runNumStr = matcher.group(2); // The capture group for the number
                    if (runNumStr != null) {
                        try {
                            int num = Integer.parseInt(runNumStr);
                            maxRunNumber[0] = Math.max(maxRunNumber[0], num);
                        } catch (NumberFormatException e) {
                            // Ignore invalid run numbers
                        }
                    } else {
                        // Directory matches base but has no run number - this shouldn't happen
                        // but if it does, treat it as run 1 for counting purposes
                        maxRunNumber[0] = Math.max(maxRunNumber[0], 0);
                    }
                }
            });
        } catch (IOException e) {
            // If we can't list directories, just return 1
        }

        return maxRunNumber[0] + 1;
    }
}