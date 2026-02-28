package com.benchmark;

/**
 * Command-line arguments DTO.
 * Immutable record for CLI configuration.
 */
public record CliArgs(
    String configFile,
    boolean webMode,
    int webPort,
    String model,
    String resultsDir,
    String language,
    String exercise,
    String agent
) {
}
