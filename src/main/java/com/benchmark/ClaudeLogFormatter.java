package com.benchmark;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import java.util.stream.Collectors;

public class ClaudeLogFormatter {
    private final Map<String,MessageRenderer> renderers = new HashMap<>();
    private final ObjectMapper mapper = new ObjectMapper();

    public ClaudeLogFormatter() {
        renderers.put("tool_use",new ToolMessageRenderer());
        renderers.put("user", new UserMessageRenderer());
        renderers.put("assistant", new AssistantMessageRenderer());
        renderers.put("queue-operation", new QueueOperationMessageRenderer());
    }

    public void renderLogStream(InputStream messageStream) {
        var jsonLog = new BufferedReader(new InputStreamReader(messageStream)).lines();
        jsonLog.map(this::readLine).forEach(this::renderMessage);
    }

    public JsonNode readLine(String line) {
        try {
            return mapper.readTree(line);
        } catch (IOException e) {
            return null;
        }
    }

    public void renderMessage(JsonNode node) {
        String type = node.get("type").asText();
        MessageRenderer messageRenderer = renderers.get(type);
        if (messageRenderer != null) {
            String timestamp = node.get("timestamp").asText();
            messageRenderer.renderMessage(timestamp, node);
        } else {
            System.out.println("unknown type: " + type);
        }
    }

    public static void main(String[] args) throws InterruptedException, IOException {
        var process = new ProcessBuilder().command("docker", "ps", "--format", "{{.ID}}").start();
        var containerId = new BufferedReader(new InputStreamReader(process.getInputStream())).lines().collect(Collectors.joining("\n"));
        process.waitFor(1000, TimeUnit.MILLISECONDS);

        process = new ProcessBuilder().command("docker", "exec", "-w", "/workspace", containerId, "ls", "/home/runner/.claude/projects/-workspace/").start();
        String jsonLogFile = new BufferedReader(new InputStreamReader(process.getInputStream())).lines().collect(Collectors.joining("\n"));
        process.waitFor(1000, TimeUnit.MILLISECONDS);

        //process = new ProcessBuilder().command("docker", "exec", "-w", "/workspace", containerId, "tail", "-f","-n","+1","/home/runner/.claude/projects/-workspace/" + jsonLogFile).start();
        process = new ProcessBuilder().command("docker", "exec", "-w", "/workspace", containerId, "cat", "/home/runner/.claude/projects/-workspace/" + jsonLogFile).start();
        ClaudeLogFormatter formatter = new ClaudeLogFormatter();
        formatter.renderLogStream(process.getInputStream());
    }

    public interface MessageRenderer {
        default void renderMessage(String timestamp, JsonNode message) {
            println(timestamp,message.toString());
        }
    }

    public static class ToolMessageRenderer implements MessageRenderer {
        public void renderMessage(String timestamp, JsonNode message) {
            println(timestamp,message.toString());
        }
    }

    public static class UserMessageRenderer implements MessageRenderer {
        public void renderMessage(String timestamp, JsonNode message) {
            JsonNode messageField = message.get("message");
            if (messageField != null) {
                if ( messageField.isObject() ) {
                    renderMessage(timestamp, messageField);
                } else if ( messageField.isArray() ) {
                    for(JsonNode item: messageField) {
                        renderMessage(timestamp, item);
                    }
                } else {
                    println(timestamp,messageField.getNodeType().toString());
                }
            } else {
                JsonNode contentField = message.get("content");
                if (contentField != null) {
                    JsonNode roleField = message.get("role");
                    if (roleField != null) {

                    }
                    if (contentField.isTextual()) {
                        println(timestamp,"User: "+ contentField.asText());
                    } else if ( contentField.isArray() ) {
                        for(JsonNode item: contentField) {
                            renderMessage(timestamp, item);
                        }
                    } else {
                        println(timestamp,"content not text or array?");
                    }
                } else {
                    println(timestamp, "Missing content");
                }
            }
        }
    }

    public static class  AssistantMessageRenderer implements MessageRenderer {
        public void renderMessage(String timestamp, JsonNode message) {
            if ( message.isObject() ) {
                JsonNode messageField = message.get("message");
                if ( messageField != null ) {
                    renderMessage(timestamp, messageField);
                } else {
                    JsonNode typeField = message.get("type");
                    JsonNode contentField = message.get("content");
                    if (contentField != null) {
                        if (typeField != null) {
                            renderMessage(timestamp, typeField.asText(), contentField);
                        } else {
                            println(timestamp,"type: "+typeField.asText().toString());
                        }
                    } else {
                        println(timestamp,"Missing type");
                    }
                }
            } else {
                println(timestamp,"Unable to handle assistant message of type "+message.getNodeType());
            }
        }

        private void renderMessage(String timestamp, String type, JsonNode content) {
            if (content.isArray() ) {
                for(JsonNode item: content) {
                    renderMessage(timestamp, type, item);
                }
            } else {
                JsonNode typeField = content.get("type");
                if (typeField != null) {
                    String typeValue = typeField.asText();
                    if ("text".equals(typeValue)) {
                        String value = content.get("text").asText();
                        println(timestamp, "Assistant: " + value);
                    } else if ( "thinking".equals(typeValue)) {
                        String value = content.get("thinking").asText();
                        println(timestamp,"Thinking: " + value);
                    } else if ("tool_use".equals(typeValue)) {
                        JsonNode nameField = content.get("name");
                        if ( nameField != null) {
                            String name = nameField.asText();
                            JsonNode inputField = content.get("input");
                            if ( inputField != null ) {
                                String input = inputField.toString();
                                println(timestamp,"Tool: " + name+" "+input);
                            } else {
                                println(timestamp,"Tool: " + name);
                            }
                        }
                    } else {
                        println(timestamp,"Unknown: " + typeValue);
                    }
                } else {
                    println(timestamp,"Unable to handle assistant message of type "+content.getNodeType());
                }
            }
        }
    }

    private class QueueOperationMessageRenderer implements MessageRenderer {
        public void renderMessage(JsonNode message) {
            String timestamp = message.get("timestamp").asText();
            println(timestamp, "operation: "+message.get("operation").asText());
        }
    }

    public static void println(String timestamp, String message) {
        System.out.println(timestamp+" "+message);
    }
}
