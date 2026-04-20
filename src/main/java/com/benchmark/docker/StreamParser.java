package com.benchmark.docker;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;

/**
 * Wraps the output stream from a Docker container and parses JSON events to
 * detect Bash tool call boundaries. When a Bash tool call starts, it notifies
 * a {@link CommandWatchdog} to start a per-command timer. When the tool call
 * finishes, it notifies the watchdog to cancel the timer.
 *
 * <p>This class is designed to work with both Claude Code's stream-json output
 * format and Pi's JSON event stream format.</p>
 *
 * <p>Usage: wrap this around the existing output callback in
 * {@link DockerClient#runCommandWithLimitsAndVolume}.</p>
 */
public class StreamParser implements Consumer<String> {
    private static final Logger logger = LoggerFactory.getLogger(StreamParser.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private final Consumer<String> downstream;
    private final CommandWatchdog watchdog;

    /**
     * Creates a new StreamParser.
     *
     * @param downstream the original output consumer (e.g. ClaudeMessageProcessor)
     * @param watchdog   the watchdog to notify on Bash tool call boundaries
     */
    public StreamParser(Consumer<String> downstream, CommandWatchdog watchdog) {
        this.downstream = downstream;
        this.watchdog = watchdog;
    }

    @Override
    public void accept(String line) {
        if (downstream != null) {
            downstream.accept(line);
        }

        // Only parse non-empty lines that look like JSON
        String trimmed = line.trim();
        if (trimmed.isEmpty() || trimmed.charAt(0) != '{') {
            return;
        }

        try {
            JsonNode root = MAPPER.readTree(trimmed);

            // ── Claude Code stream-json format ──────────────────────────
            // Tool call: {"type":"assistant","message":{"content":[{...,"type":"tool_use","name":"Bash",...}]}}
            // Tool result: {"type":"user","message":{"content":[{...,"type":"tool_result",...}]}}
            parseClaudeFormat(root);

            // ── Pi agent format ─────────────────────────────────────────
            // Tool call: {"type":"message","message":{"content":[{...,"type":"toolCall","name":"bash",...}]}}
            // Tool result: {"type":"message","message":{"role":"toolResult",...}}
            parsePiFormat(root);

            // ── Pi tool_execution_start / tool_execution_end ─────────────
            // {"type":"tool_execution_start","toolName":"Bash","args":{"command":"..."}}
            // {"type":"tool_execution_end","toolName":"Bash"}
            parsePiToolExecutionEvents(root);

        } catch (Exception e) {
            // Not valid JSON or parsing error — ignore. The downstream consumer
            // already received the raw line.
        }
    }

    /**
     * Detects Claude Code tool_use (Bash) and tool_result events in the
     * assistant/user message format.
     */
    private void parseClaudeFormat(JsonNode root) {
        String type = root.has("type") ? root.get("type").asText() : null;

        // Assistant message with tool_use
        if ("assistant".equals(type)) {
            JsonNode message = root.get("message");
            if (message != null) {
                JsonNode content = message.get("content");
                if (content != null && content.isArray()) {
                    for (JsonNode item : content) {
                        if (item.has("type") && "tool_use".equals(item.get("type").asText())) {
                            String name = item.has("name") ? item.get("name").asText() : "";
                            if ("Bash".equals(name)) {
                                String command = extractCommand(item);
                                if (command != null && !command.isEmpty()) {
                                    logger.debug("Claude Bash tool call started: {}", command.substring(0, Math.min(command.length(), 100)));
                                    watchdog.onToolCallStarted(command);
                                }
                            }
                        }
                    }
                }
            }
        }

        // User message with tool_result (signals the end of a tool call)
        if ("user".equals(type)) {
            JsonNode message = root.get("message");
            if (message != null) {
                JsonNode content = message.get("content");
                if (content != null && content.isArray()) {
                    for (JsonNode item : content) {
                        if (item.has("type") && "tool_result".equals(item.get("type").asText())) {
                            // All pending tool calls for this message batch are done.
                            // We can't know exactly which one finished, so we cancel
                            // the oldest pending timer (FIFO ordering).
                            cancelOldestTimer();
                        }
                    }
                }
            }
        }
    }

    /**
     * Detects Pi agent toolCall events in the message format.
     */
    private void parsePiFormat(JsonNode root) {
        if (!root.has("type") || !root.get("type").isTextual()) {
            return;
        }
        String type = root.get("type").asText();

        // Pi message with toolCall
        if ("message".equals(type)) {
            JsonNode message = root.get("message");
            if (message == null) return;

            JsonNode content = message.get("content");
            if (content == null) return;

            // Handle array or single content
            List<JsonNode> itemsList = new java.util.ArrayList<>();
            if (content.isArray()) {
                content.forEach(itemsList::add);
            } else {
                itemsList.add(content);
            }
            JsonNode[] items = itemsList.toArray(new JsonNode[0]);

            for (JsonNode item : items) {
                if (!item.isObject()) continue;

                // toolCall inside assistant message
                if (item.has("type") && "toolCall".equals(item.get("type").asText())) {
                    String name = item.has("name") ? item.get("name").asText() : "";
                    if ("bash".equalsIgnoreCase(name)) {
                        JsonNode args = item.get("arguments");
                        if (args != null && args.has("command")) {
                            String command = args.get("command").asText();
                            logger.debug("Pi bash tool call started: {}", command.substring(0, Math.min(command.length(), 100)));
                            watchdog.onToolCallStarted(command);
                        }
                    }
                }

                // toolResult inside assistant message (Pi sometimes puts tool results here)
                if (item.has("type") && "toolResult".equals(item.get("type").asText())) {
                    cancelOldestTimer();
                }
            }

            // Also check for toolResult role directly
            if (message.has("role") && "toolResult".equals(message.get("role").asText())) {
                cancelOldestTimer();
            }
        }
    }

    /**
     * Detects Pi's explicit tool_execution_start / tool_execution_end events.
     * These are top-level event types, not nested in messages.
     */
    private void parsePiToolExecutionEvents(JsonNode root) {
        String type = root.has("type") ? root.get("type").asText() : null;

        if ("tool_execution_start".equals(type)) {
            String toolName = root.has("toolName") ? root.get("toolName").asText() : "";
            if ("Bash".equals(toolName) || "bash".equalsIgnoreCase(toolName)) {
                JsonNode args = root.get("args");
                if (args != null && args.has("command")) {
                    String command = args.get("command").asText();
                    logger.debug("Pi tool_execution_start (Bash): {}", command.substring(0, Math.min(command.length(), 100)));
                    watchdog.onToolCallStarted(command);
                }
            }
        }

        if ("tool_execution_end".equals(type)) {
            String toolName = root.has("toolName") ? root.get("toolName").asText() : "";
            if ("Bash".equals(toolName) || "bash".equalsIgnoreCase(toolName)) {
                cancelOldestTimer();
            }
        }
    }

    /**
     * Extracts the command string from a tool_use node.
     * Handles both Claude Code's "input.command" and Pi's "arguments.command".
     */
    private String extractCommand(JsonNode toolUseNode) {
        JsonNode input = toolUseNode.has("input") ? toolUseNode.get("input") : null;
        if (input == null) {
            input = toolUseNode.has("arguments") ? toolUseNode.get("arguments") : null;
        }
        if (input != null && input.has("command")) {
            return input.get("command").asText();
        }
        return null;
    }

    /**
     * Cancels the oldest pending watchdog timer (FIFO). This is a best-effort
     * approach since we don't have exact tool call IDs in the stream for all formats.
     */
    private void cancelOldestTimer() {
        // The CommandWatchdog doesn't expose its timer map, so we use a sentinel
        // approach: we'll add a cancelAll method or use a different strategy.
        // For now, we'll signal the watchdog that a tool call finished by using
        // a special marker. Actually, let's change the approach: we'll track
        // a simple counter and match timers by index.
        //
        // Simpler approach: just cancel the first available timer.
        // We need to expose a cancelOldest method on CommandWatchdog.
        watchdog.cancelOldestTimer();
    }
}
