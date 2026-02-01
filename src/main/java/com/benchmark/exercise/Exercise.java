package com.benchmark.exercise;

import java.nio.file.Path;

/**
 * Represents an exercise in the benchmark suite.
 */
public class Exercise {
    private final String name;
    private final String language;
    private final Path sourcePath;
    private final Path testPath;
    private final Path referencePath;

    public Exercise(String name, String language, Path sourcePath, Path testPath, Path referencePath) {
        this.name = name;
        this.language = language;
        this.sourcePath = sourcePath;
        this.testPath = testPath;
        this.referencePath = referencePath;
    }

    public String getName() {
        return name;
    }

    public String getLanguage() {
        return language;
    }

    public Path getSourcePath() {
        return sourcePath;
    }

    public Path getTestPath() {
        return testPath;
    }

    public Path getReferencePath() {
        return referencePath;
    }

    @Override
    public String toString() {
        return String.format("Exercise{name='%s', language='%s', path=%s}", name, language, sourcePath);
    }
}
