package io.schell.llm.benchmark.exercise;

import java.nio.file.Path;
import java.util.List;

/**
 * Represents an exercise in the benchmark suite.
 */
public class Exercise {
    private final String name;
    private final String language;
    private final ExerciseMetadata metadata;
    private final Path exercisePath;

    public Exercise(String name, String language, Path exercisePath, ExerciseMetadata metadata) {
        this.name = name;
        this.language = language;
        this.exercisePath = exercisePath;
        this.metadata = metadata;
    }

    public String getName() {
        return name;
    }

    public String getLanguage() {
        return language;
    }

    public Iterable<Path> getSourcePath() {
        return metadata.getFiles().getSolution().stream().map(exercisePath::resolve).toList();
    }

    public Iterable<Path> getTestPath() {
        return metadata.getFiles().getTest().stream().map(exercisePath::resolve).toList();
    }

    public Iterable<Path> getReferencePath() {
        return metadata.getFiles().getSolution().stream().map(exercisePath::resolve).toList();
    }

    /**
     * Returns the blurb from metadata, or null if not available.
     */
    public String getBlurb() {
        return metadata != null ? metadata.getBlurb() : null;
    }

    @Override
    public String toString() {
        return String.format("Exercise{name='%s', language='%s', path=%s}", name, language, exercisePath);
    }

    public boolean hasReference() {
        return metadata != null && !metadata.getFiles().getSolution().isEmpty();
    }

    public Iterable<? extends Path> getExamples() {
        return metadata.getFiles().getExample().stream().map(exercisePath::resolve).toList();
    }

    public Path getExercisePath() {
        return exercisePath;
    }
}
