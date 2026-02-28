package com.benchmark.agent;

import com.benchmark.agent.handlers.*;
import com.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Registry for language handlers.
 * Provides lookup and management of language-specific handlers.
 */
public class LanguageHandlerRegistry {
    private static final Logger logger = LoggerFactory.getLogger(LanguageHandlerRegistry.class);

    private final Map<String, LanguageHandler> handlers = new HashMap<>();

    public LanguageHandlerRegistry() {
        // Register all built-in handlers
        register(new JavaHandler());
        register(new GoHandler());
        register(new JavaScriptHandler());
        register(new PythonHandler());
        register(new RustHandler());
        register(new CppHandler());

        logger.info("Registered {} language handlers", handlers.size());
    }

    /**
     * Registers a language handler.
     */
    public void register(LanguageHandler handler) {
        handlers.put(handler.getLanguage().toLowerCase(), handler);
    }

    /**
     * Gets the handler for a specific language.
     *
     * @param language The language name
     * @return The handler, or null if not found
     */
    public LanguageHandler getHandler(String language) {
        return handlers.get(language.toLowerCase());
    }

    /**
     * Gets the handler for an exercise.
     *
     * @param exercise The exercise
     * @return The handler, or null if not found
     */
    public LanguageHandler getHandler(Exercise exercise) {
        return getHandler(exercise.getLanguage());
    }

    /**
     * Checks if a handler exists for the given language.
     */
    public boolean hasHandler(String language) {
        return handlers.containsKey(language.toLowerCase());
    }

    /**
     * Gets all registered languages.
     */
    public List<String> getSupportedLanguages() {
        return List.copyOf(handlers.keySet());
    }
}
