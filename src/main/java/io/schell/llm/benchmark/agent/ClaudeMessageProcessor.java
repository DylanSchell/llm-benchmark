package io.schell.llm.benchmark.agent;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.util.function.Consumer;

public class ClaudeMessageProcessor implements Consumer<String> {
    private final ObjectMapper objectMapper = new ObjectMapper();
    private final Consumer<String> outputConsumer;

    public ClaudeMessageProcessor(Consumer<String> outputConsumer) {
        this.outputConsumer = outputConsumer;
    }

    @Override
    public void accept(String s) {
        try {
            JsonNode jsonNode = objectMapper.readTree(s);
            if (jsonNode.has("type")) {
                String type = jsonNode.get("type").asText();
                switch (type) {
                    case "stream_event" -> {
                        JsonNode event = jsonNode.get("event");
                        if (event.has("type")) {
                            switch (event.get("type").asText()) {
                                case "message_start" -> {
                                    JsonNode message = event.get("message");
                                    JsonNode content = message.get("content");
                                    if (content.isArray()) {
                                        for (JsonNode item : content) {
                                            String contentType = item.get("type").asText();
                                            if (contentType.equals("thinking")) {
                                                print(item.get("thinking").asText());
                                            } else if (contentType.equals("text")) {
                                                print(item.get("text").asText());
                                            } else {
                                                print(s);
                                            }
                                        }
                                        return;
                                    } else {
                                        String contentType = content.get("type").asText();
                                        if (contentType.equals("thinking")) {
                                            print(content.get("thinking").asText());
                                            return;
                                        } else if (contentType.equals("message")) {
                                            // print(content.get("message"));
                                            // JsonNode messageContent = content.get("content");
                                            // for now ignore these, they seem to be empty most of the time
                                            return;
                                        } else {
                                            println(s);
                                            return;
                                        }
                                    }
                                }
                                case "message_delta" -> {
                                    JsonNode delta = event.get("delta");
                                    // stop_reason=tool_use?
                                    // println(delta.get("delta").asText());
                                    return;
                                }
                                case "content_block_delta" -> {
                                    // {"type":"stream_event","event":{"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"\":"}},"session_id":"a03a9304-8ecb-4204-9c24-2201e8dda43e","parent_tool_use_id":null,"uuid":"b39c1d17-df4b-4a4a-877b-f7e0d754f45e"}
                                    JsonNode delta = event.get("delta");
                                    String deltaType = delta.get("type").asText();
                                    switch (deltaType) {
                                        case "thinking_delta" -> {
                                            print(delta.get("thinking").asText());
                                            return;
                                        }
                                        case "input_json_delta" -> {
                                            // print(delta.get("partial_json").asText());
                                            return;
                                        }
                                        case "signature_delta" -> {
                                            // we do not need to see this in the console log
                                            return;
                                        }
                                        case "text_delta" -> {
                                            print(delta.get("text").asText());
                                            return;
                                        }
                                        default -> {
                                            println(s);
                                            return;
                                        }
                                    }
                                }
                                case "content_block_start" -> {
                                    // ignore
                                    return;
                                }
                                case "content_block_stop" -> {
                                    // ignore
                                    return;
                                }
                                case "message_stop" -> {
                                    // ignore
                                    // TODO: this is probably where we want to put in the newlines
                                    println("");
                                    return;
                                }
                                default -> {
                                    print(s);
                                    return;
                                }
                            }
                        } else {
                            println(s);
                            return;
                        }
                    }
                    case "assistant" -> {
                        JsonNode message = jsonNode.get("message");
                        JsonNode content = message.get("content");
                        if (content.isArray()) {
                            for (JsonNode item : content) {
                                String itemType = item.get("type").asText();
                                switch (itemType) {
                                    case "thinking" -> {
                                        print(item.get("thinking").asText());
                                    }
                                    case "tool_use" -> {
                                        render_tool_use(item);
                                    }
                                    case "text" -> {
                                        print(item.get("text").asText());
                                    }
                                    default -> {
                                        print(s);
                                    }
                                }
                            }
                            return;
                        } else {
                            String itemType = content.get("type").asText();
                            switch (itemType) {
                                case "thinking" -> {
                                    print(content.get("thinking").asText());
                                    return;
                                }
                                case "tool_use" -> {
                                    render_tool_use(content);
                                    return;
                                }
                                case "text" -> {
                                    print(content.get("text").asText());
                                    return;
                                }
                                default -> {
                                    println(s);
                                    return;
                                }
                            }
                        }
                    }
                    case "user" -> {
                        JsonNode message = jsonNode.get("message");
                        JsonNode content = message.get("content");
                        if (content.isArray()) {
                            for (JsonNode item : content) {
                                String itemType = item.get("type").asText();
                                switch (itemType) {
                                    case "text" -> {
                                        println(item.get("text").asText());
                                        return;
                                    }
                                    case "tool_result" -> {
                                        if (item.has("content")) {
                                            JsonNode tool_result_content = item.get("content");
                                            if (tool_result_content.isTextual()) {
                                                String tool_result_string = tool_result_content.asText();
                                                String tool_result_with_newlines = tool_result_string.replaceAll("\\n", "\n");
                                                println("tool_result:\n" + tool_result_with_newlines);
                                                return;
                                            }
                                        }
                                        println(s);
                                        return;
                                    }
                                    default -> {
                                        println(s);
                                        return;
                                    }
                                }
                            }
                        } else {
                            String itemType = content.get("type").asText();
                            switch (itemType) {
                                case "text" -> {
                                    println(content.get("text").asText());
                                    return;
                                }
                                case "tool_result" -> {
                                    println("tool_result: " + content.get("content").toString());
                                    return;
                                }
                                default -> {
                                    println(s);
                                    return;
                                }
                            }
                        }
                        println(s);
                        return;
                    }
                    case "system" -> {
                        // ignore system message for now
                        println(s);
                        return;
                    }
                    case "result" -> {
//                            String subType = jsonNode.get("subtype").asText();
//                            boolean isError = jsonNode.get("is_error").asBoolean();
//                            long duration_ms = jsonNode.get("duration_ms").asLong();
//                            long duration_api_ms = jsonNode.get("duration_api_ms").asLong();
//                            int num_turns = jsonNode.get("num_turns").asInt();
//                            String result = jsonNode.get("result").asText();
//                            double total_cost_usd = jsonNode.get("total_cost_usd").asDouble();
//                            JsonNode usage = jsonNode.get("usage");
//                            long input_tokens = usage.get("input_tokens").asLong();
//                            long output_tokens = usage.get("output_tokens").asLong();
//                            JsonNode modelUsage = jsonNode.get("modelUsage");
//                            String permission_denials = jsonNode.get("permission_denials").asText();
                        println("Result: " + jsonNode.toPrettyString());
                    }
                    default -> {
                        println(s);
                        return;
                    }
                }
            }
        } catch (JsonProcessingException e) {
            // not a json log, probably a warning from claude, so we just output it
        }
        println(s);
    }

    private void render_tool_use(JsonNode item) {
        try {
            String name = item.get("name").asText();
            switch (name) {
                case "Edit" -> {
                    JsonNode input = item.get("input");
                    boolean replace_all = input.get("replace_all").asBoolean();
                    String file_path = input.get("file_path").asText();
                    String old_string = input.get("old_string").asText();
                    String new_string = input.get("new_string").asText();
                    println("Edit " + file_path);
                    println("Old");
                    println(old_string.replaceAll("\\n", "\n"));
                    println("New");
                    println(new_string.replaceAll("\\n", "\n"));
                }
                case "Glob" -> {
                    String pattern = item.get("input").get("pattern").asText();
                    println("\ntool_use: Glob " + pattern);
                }
                case "Read" -> {
                    String file_path = item.get("input").get("file_path").asText();
                    println("\ntool_use: Read " + file_path);
                }
                case "Write" -> {
                    String file_path = item.get("input").get("file_path").asText();
                    var input = item.get("input");

                    if (input.has("content")) {
                        var content = input.get("content").asText();
                        println("\ntool_use: Write " + file_path);
                        println("Content: ");
                        content = content.replaceAll("\\n", "\n");
                        println(content);
                    }

                }
                case "Bash" -> {
                    boolean run_in_background = false;
                    if (item.get("input").has("run_in_background")) {
                        run_in_background = item.get("input").get("run_in_background").asBoolean();
                    }
                    String description = "";
                    if (item.get("input").has("description")) {
                        description = item.get("input").get("description").asText();
                    }
                    String command = item.get("input").get("command").asText();
                    println("\ntool_use: Bash " + (run_in_background ? "(in background) " : " ") + description);
                    println("Command: " + command);
                }
                case "TaskOutput" -> {
                    String input = item.get("input").toString();
                    println("\ntool_use: " + name + " " + input);
                }
                case "TodoWrite" -> {
                    println("\ntool_use: TodoWrite");
                    JsonNode input = item.get("input");
                    JsonNode todos = input.get("todos");
                    for (JsonNode todo : todos) {
                        String content = todo.get("content").asText();
                        String status = todo.get("status").asText();
                        switch (status) {
                            case "in_progress" -> {
                                println(String.format("[⟳] %s", content));
                            }
                            case "pending" -> {
                                println(String.format("[⌛] %s", content));
                            }
                            case "completed" -> {
                                println(String.format("[✅] %s", content));
                            }
                            default -> {
                                println(String.format("[ ] %s", content));
                            }
                        }

                    }
                }
                default -> {
                    String input = item.get("input").toString();
                    println("\ntool_use: " + name + " " + input);
                }
            }
        } catch (Exception e) {
            e.printStackTrace();
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
