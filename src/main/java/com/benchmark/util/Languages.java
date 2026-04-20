package com.benchmark.util;

import java.util.Set;

/**
 * Centralized constants for supported programming languages.
 */
public final class Languages {
    
    /**
     * Set of all supported language identifiers (lowercase).
     */
    public static final Set<String> SUPPORTED = Set.of(
        "java", "go", "javascript", "python", "rust", "cpp"
    );

    private Languages() {
        // Prevent instantiation
    }

    /**
     * Checks if a language is supported.
     *
     * @param language The language identifier to check
     * @return true if the language is supported, false otherwise
     */
    public static boolean isSupported(String language) {
        return language != null && SUPPORTED.contains(language.toLowerCase());
    }

    /**
     * Normalizes a language identifier to lowercase.
     *
     * @param language The language identifier to normalize
     * @return Lowercase version of the language, or null if input is null
     */
    public static String normalize(String language) {
        return language != null ? language.toLowerCase() : null;
    }
}
