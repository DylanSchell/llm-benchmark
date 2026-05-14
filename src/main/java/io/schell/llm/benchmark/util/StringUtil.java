package io.schell.llm.benchmark.util;

/**
 * Utility methods for common string operations.
 */
public final class StringUtil {

    private StringUtil() {
        // Prevent instantiation
    }

    /**
     * Returns the given value if it is non-null and non-empty, otherwise returns null.
     * Used throughout the codebase to normalize "empty string" to null for directory naming.
     */
    public static String toNonNull(String value) {
        return (value != null && !value.isEmpty()) ? value : null;
    }

    /**
     * Checks if a string is non-null and non-empty.
     */
    public static boolean isNonEmpty(String value) {
        return value != null && !value.isEmpty();
    }

    /**
     * Safely joins an array of strings with the given separator, skipping null elements.
     */
    public static String join(String[] parts, String separator) {
        if (parts == null || parts.length == 0) {
            return "";
        }
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < parts.length; i++) {
            if (parts[i] != null) {
                if (i > 0) sb.append(separator);
                sb.append(parts[i]);
            }
        }
        return sb.toString();
    }
}
