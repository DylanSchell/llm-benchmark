package io.schell.llm.benchmark.exception;

/**
 * Exception thrown when an exercise is not found.
 */
public class ExerciseNotFoundException extends BenchmarkException {
    public ExerciseNotFoundException(String language, String exerciseName) {
        super(String.format("Exercise not found: %s/%s", language, exerciseName));
    }

    public ExerciseNotFoundException(String message, Throwable cause) {
        super(message, cause);
    }
}
