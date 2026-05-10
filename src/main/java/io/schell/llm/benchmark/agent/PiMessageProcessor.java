package io.schell.llm.benchmark.agent;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.util.function.Consumer;

/**
 * Message processor for Pi agent JSON event stream output.
 * Parses and formats Pi events for console display, similar to ClaudeMessageProcessor.
 */
public class PiMessageProcessor implements Consumer<String> {
    private final ObjectMapper objectMapper = new ObjectMapper();
    private final Consumer<String> outputConsumer;

    public PiMessageProcessor(Consumer<String> outputConsumer) {
        this.outputConsumer = outputConsumer;
    }

    @Override
    public void accept(String s) {
        try {
            JsonNode jsonNode = objectMapper.readTree(s);
            String type = jsonNode.has("type") ? jsonNode.get("type").asText() : null;

            if (type == null) {
                println(s);
                return;
            }

            switch (type) {
                case "session" -> {
                    // Session header - show session info
                    String id = jsonNode.has("id") ? jsonNode.get("id").asText() : "unknown";
                    String cwd = jsonNode.has("cwd") ? jsonNode.get("cwd").asText() : ".";
                    int version = jsonNode.has("version") ? jsonNode.get("version").asInt(1) : 1;
                    println(String.format("[Session v%d] ID: %s, CWD: %s", version, id, cwd));
                }

                case "agent_start" -> {
                    println("\n[Agent starting...]");
                }

                case "agent_end" -> {
                    println("\n[Agent finished]");
                }

                case "turn_start" -> {
                    println("\n--- Turn Start ---");
                }

                case "turn_end" -> {
                    println("--- Turn End ---\n");
                }

                case "message_start" -> {
                    handleMessageStart(jsonNode);
                }

                case "message_update" -> {
                    handleMessageUpdate(jsonNode);
                }

                case "message_end" -> {
                    // Message complete, add newline
                    println("");
                }

                case "tool_execution_start" -> {
                    handleToolExecutionStart(jsonNode);
                }

                case "tool_execution_update" -> {
                    handleToolExecutionUpdate(jsonNode);
                }

                case "tool_execution_end" -> {
                    handleToolExecutionEnd(jsonNode);
                }

                case "auto_compaction_start" -> {
                    String reason = jsonNode.has("reason") ? jsonNode.get("reason").asText() : "unknown";
                    println(String.format("\n[Auto-compaction started: %s]", reason));
                }

                case "auto_compaction_end" -> {
                    boolean aborted = jsonNode.has("aborted") && jsonNode.get("aborted").asBoolean();
                    boolean willRetry = jsonNode.has("willRetry") && jsonNode.get("willRetry").asBoolean();
                    if (aborted) {
                        println("[Auto-compaction aborted]");
                    } else if (willRetry) {
                        println("[Auto-compaction failed, will retry]");
                    } else {
                        println("[Auto-compaction completed]");
                    }
                }

                case "auto_retry_start" -> {
                    int attempt = jsonNode.has("attempt") ? jsonNode.get("attempt").asInt() : 1;
                    int maxAttempts = jsonNode.has("maxAttempts") ? jsonNode.get("maxAttempts").asInt() : 3;
                    String errorMessage = jsonNode.has("errorMessage") ? jsonNode.get("errorMessage").asText() : "";
                    println(String.format("\n[Auto-retry %d/%d: %s]", attempt, maxAttempts, errorMessage));
                }

                case "auto_retry_end" -> {
                    boolean success = jsonNode.has("success") && jsonNode.get("success").asBoolean();
                    int attempt = jsonNode.has("attempt") ? jsonNode.get("attempt").asInt() : 1;
                    if (success) {
                        println(String.format("[Auto-retry %d succeeded]", attempt));
                    } else {
                        String finalError = jsonNode.has("finalError") ? jsonNode.get("finalError").asText() : "unknown error";
                        println(String.format("[Auto-retry %d failed: %s]", attempt, finalError));
                    }
                }

                case "model_change" -> {
                    String provider = jsonNode.has("provider") ? jsonNode.get("provider").asText() : "unknown";
                    String modelId = jsonNode.has("modelId") ? jsonNode.get("modelId").asText() : "unknown";
                    println(String.format("\n[Model changed to: %s/%s]", provider, modelId));
                }

                case "thinking_level_change" -> {
                    String thinkingLevel = jsonNode.has("thinkingLevel") ? jsonNode.get("thinkingLevel").asText() : "unknown";
                    println(String.format("\n[Thinking level: %s]", thinkingLevel));
                }

                case "compaction" -> {
                    String summary = jsonNode.has("summary") ? jsonNode.get("summary").asText() : "";
                    int tokensBefore = jsonNode.has("tokensBefore") ? jsonNode.get("tokensBefore").asInt() : 0;
                    println(String.format("\n[Compacted %d tokens: %s]", tokensBefore, summary.substring(0, Math.min(100, summary.length()))));
                }

                case "branch_summary" -> {
                    String summary = jsonNode.has("summary") ? jsonNode.get("summary").asText() : "";
                    println(String.format("\n[Branched: %s]", summary.substring(0, Math.min(100, summary.length()))));
                }

                case "custom_message" -> {
                    String customType = jsonNode.has("customType") ? jsonNode.get("customType").asText() : "extension";
                    String content = jsonNode.has("content") ? jsonNode.get("content").asText() : "";
                    boolean display = jsonNode.has("display") && jsonNode.get("display").asBoolean();
                    if (display) {
                        println(String.format("\n[%s] %s", customType, content));
                    }
                }

                case "bashExecution" -> {
                    handleBashExecution(jsonNode);
                }

                default -> {
                    // Unknown event type, print raw JSON
                    println(s);
                }
            }
        } catch (JsonProcessingException e) {
            // Not valid JSON, print as-is (might be warnings or other output)
            println(s);
        } catch (Exception e) {
            // Handle any parsing errors gracefully
            println("[Error processing event: " + e.getMessage() + "]");
        }
    }

    private void handleMessageStart(JsonNode jsonNode) {
        JsonNode message = jsonNode.has("message") ? jsonNode.get("message") : null;
        if (message == null) return;

        String role = message.has("role") ? message.get("role").asText() : "unknown";
        JsonNode content = message.has("content") ? message.get("content") : null;

        switch (role) {
            case "user" -> {
                if (content != null && content.isTextual()) {
                    println("\n[User]: " + content.asText());
                } else if (content != null && content.isArray()) {
                    printUserContent(content);
                }
            }

            case "assistant" -> {
                println("\n[Assistant]:");
                if (content != null && content.isArray()) {
                    for (JsonNode item : content) {
                        String itemType = item.has("type") ? item.get("type").asText() : "";
                        switch (itemType) {
                            case "text" -> {
                                String text = item.has("text") ? item.get("text").asText() : "";
                                print(text);
                            }
                            case "thinking" -> {
                                String thinking = item.has("thinking") ? item.get("thinking").asText() : "";
                                println("\n[Thinking]:");
                                println(thinking);
                            }
                            case "toolCall" -> {
                                handleToolCall(item);
                            }
                        }
                    }
                } else if (content != null && content.isObject()) {
                    String itemType = content.has("type") ? content.get("type").asText() : "";
                    switch (itemType) {
                        case "text" -> {
                            String text = content.has("text") ? content.get("text").asText() : "";
                            print(text);
                        }
                        case "thinking" -> {
                            String thinking = content.has("thinking") ? content.get("thinking").asText() : "";
                            println("\n[Thinking]:");
                            println(thinking);
                        }
                        case "toolCall" -> {
                            handleToolCall(content);
                        }
                    }
                }
            }

            case "toolResult" -> {
                if (content != null && content.isArray()) {
                    printToolResultContent(content);
                } else if (content != null && content.isTextual()) {
                    println("[Tool Result]: " + content.asText());
                }
            }

            case "bashExecution" -> {
                handleBashExecution(message);
            }
        }
    }

    private void handleMessageUpdate(JsonNode jsonNode) {
        JsonNode assistantMessageEvent = jsonNode.has("assistantMessageEvent") ? jsonNode.get("assistantMessageEvent") : null;
        if (assistantMessageEvent == null) return;

        String eventType = assistantMessageEvent.has("type") ? assistantMessageEvent.get("type").asText() : "";

        switch (eventType) {
            case "text_delta" -> {
                String delta = assistantMessageEvent.has("delta") ? assistantMessageEvent.get("delta").asText() : "";
                print(delta);
            }

            case "thinking_delta" -> {
                String delta = assistantMessageEvent.has("delta") ? assistantMessageEvent.get("delta").asText() : "";
                print(delta);
            }

            case "tool_call_delta" -> {
                // Tool call in progress, skip for now
            }

            default -> {
                // Other update types, ignore or print raw
            }
        }
    }

    private void handleToolExecutionStart(JsonNode jsonNode) {
        String toolName = jsonNode.has("toolName") ? jsonNode.get("toolName").asText() : "unknown";
        String toolCallId = jsonNode.has("toolCallId") ? jsonNode.get("toolCallId").asText() : "";
        JsonNode args = jsonNode.has("args") ? jsonNode.get("args") : null;

        println(String.format("\n[Tool: %s] (ID: %s)", toolName, toolCallId));

        if (args != null && !args.isNull()) {
            printToolArgs(toolName, args);
        }
    }

    private void handleToolExecutionUpdate(JsonNode jsonNode) {
        String toolName = jsonNode.has("toolName") ? jsonNode.get("toolName").asText() : "unknown";
        JsonNode partialResult = jsonNode.has("partialResult") ? jsonNode.get("partialResult") : null;

        if (partialResult != null && !partialResult.isNull()) {
            println(String.format("[Tool %s progress]...", toolName));
        }
    }

    private void handleToolExecutionEnd(JsonNode jsonNode) {
        String toolName = jsonNode.has("toolName") ? jsonNode.get("toolName").asText() : "unknown";
        boolean isError = jsonNode.has("isError") && jsonNode.get("isError").asBoolean();
        JsonNode result = jsonNode.has("result") ? jsonNode.get("result") : null;

        if (isError) {
            println(String.format("[Tool %s failed]", toolName));
        } else {
            println(String.format("[Tool %s completed]", toolName));
        }

        if (result != null && !result.isNull()) {
            printToolResult(toolName, result);
        }
    }

    private void handleBashExecution(JsonNode jsonNode) {
        String command = jsonNode.has("command") ? jsonNode.get("command").asText() : "";
        String output = jsonNode.has("output") ? jsonNode.get("output").asText() : "";
        Integer exitCode = jsonNode.has("exitCode") && !jsonNode.get("exitCode").isNull() 
                ? jsonNode.get("exitCode").asInt() : null;
        boolean cancelled = jsonNode.has("cancelled") && jsonNode.get("cancelled").asBoolean();
        boolean truncated = jsonNode.has("truncated") && jsonNode.get("truncated").asBoolean();

        println(String.format("\n[Bash: %s]", command));

        if (output != null && !output.isEmpty()) {
            // Limit output length for display
            String displayOutput = output.length() > 500 ? output.substring(0, 500) + "...[truncated]" : output;
            println(displayOutput);
        }

        if (exitCode != null) {
            println(String.format("[Exit code: %d]", exitCode));
        }

        if (cancelled) {
            println("[Command cancelled]");
        }

        if (truncated) {
            println("[Output truncated]");
        }
    }

    private void handleToolCall(JsonNode item) {
        String name = item.has("name") ? item.get("name").asText() : "unknown";
        JsonNode arguments = item.has("arguments") ? item.get("arguments") : null;

        println(String.format("[Tool Call: %s]", name));

        if (arguments != null && !arguments.isNull()) {
            printToolArgs(name, arguments);
        }
    }

    private void printToolArgs(String toolName, JsonNode args) {
        switch (toolName) {
            case "Read" -> {
                String filePath = args.has("file_path") ? args.get("file_path").asText() : "";
                println(String.format("  Reading: %s", filePath));
            }

            case "Write" -> {
                String filePath = args.has("file_path") ? args.get("file_path").asText() : "";
                println(String.format("  Writing: %s", filePath));
            }

            case "Edit" -> {
                String filePath = args.has("file_path") ? args.get("file_path").asText() : "";
                println(String.format("  Editing: %s", filePath));
            }

            case "Bash" -> {
                String command = args.has("command") ? args.get("command").asText() : "";
                println(String.format("  Command: %s", command));
            }

            case "Glob" -> {
                String pattern = args.has("pattern") ? args.get("pattern").asText() : "";
                println(String.format("  Pattern: %s", pattern));
            }

            case "Grep" -> {
                String pattern = args.has("pattern") ? args.get("pattern").asText() : "";
                String filePath = args.has("file_path") ? args.get("file_path").asText() : "";
                println(String.format("  Pattern: %s, File: %s", pattern, filePath));
            }

            default -> {
                // Print raw JSON for unknown tools
                println("  Args: " + args.toPrettyString());
            }
        }
    }

    private void printToolResult(String toolName, JsonNode result) {
        if (result.isTextual()) {
            String content = result.asText();
            // Limit output length
            if (content.length() > 1000) {
                println("  Output: " + content.substring(0, 1000) + "...[truncated]");
            } else {
                println("  Output: " + content);
            }
        } else if (result.isObject()) {
            // For complex results, show summary
            println("  Result: " + result.toPrettyString());
        }
    }

    private void printToolResultContent(JsonNode content) {
        for (JsonNode item : content) {
            String itemType = item.has("type") ? item.get("type").asText() : "";
            switch (itemType) {
                case "text" -> {
                    String text = item.has("text") ? item.get("text").asText() : "";
                    println("[Result]: " + text);
                }
                case "image" -> {
                    println("[Result: image]");
                }
            }
        }
    }

    private void printUserContent(JsonNode content) {
        for (JsonNode item : content) {
            String itemType = item.has("type") ? item.get("type").asText() : "";
            switch (itemType) {
                case "text" -> {
                    String text = item.has("text") ? item.get("text").asText() : "";
                    println("[User]: " + text);
                }
                case "image" -> {
                    println("[User: image]");
                }
            }
        }
    }

    private void print(String s) {
        System.out.print(s);
        if (outputConsumer != null) {
            outputConsumer.accept(s);
        }
    }

    private void println(String s) {
        System.out.println(s);
        if (outputConsumer != null) {
            outputConsumer.accept(s + "\n");
        }
    }
}
