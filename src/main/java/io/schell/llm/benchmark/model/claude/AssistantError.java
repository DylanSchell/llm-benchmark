package io.schell.llm.benchmark.model.claude;

import com.fasterxml.jackson.annotation.JsonIgnore;
import java.util.HashMap;
import java.util.Map;

/**
 * Represents the {@code error} field that may appear in an assistant entry.
 * Its exact structure is not known, so additional properties are stored in a map.
 */
public class AssistantError {
    @JsonIgnore
    private Map<String, Object> additionalProperties = new HashMap<>();

    }
