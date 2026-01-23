package com.benchmark.model;

import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.core.JsonToken;
import com.fasterxml.jackson.databind.DeserializationContext;
import com.fasterxml.jackson.databind.JsonDeserializer;
import com.fasterxml.jackson.databind.annotation.JsonDeserialize;
import com.fasterxml.jackson.databind.util.StdConverter;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

/**
 * Custom deserializer for the content field which can be:
 * - A plain string
 * - A single content object
 * - An array of content objects
 */
public class ContentListDeserializer extends JsonDeserializer<List<Content>> {

    @Override
    public List<Content> deserialize(JsonParser p, DeserializationContext ctxt) throws IOException {
        JsonToken token = p.currentToken();

        List<Content> result = new ArrayList<>();

        if (token == JsonToken.VALUE_STRING) {
            // Content is a plain string - wrap it as a StringContent
            StringContent stringContent = new StringContent();
            stringContent.setType("text");
            stringContent.setText(p.getValueAsString());
            result.add(stringContent);
        } else if (token == JsonToken.START_OBJECT) {
            // Single content object
            result.add(p.readValueAs(Content.class));
        } else if (token == JsonToken.START_ARRAY) {
            // Array of content objects
            while (p.nextToken() != JsonToken.END_ARRAY) {
                result.add(p.readValueAs(Content.class));
            }
        }

        return result;
    }
}