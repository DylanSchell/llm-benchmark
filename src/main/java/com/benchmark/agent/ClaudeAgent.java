package com.benchmark.agent;

import com.benchmark.docker.DockerClient;
import com.benchmark.docker.DockerClient.ProcessResult;
import com.benchmark.exercise.Exercise;
import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;
import java.util.stream.Stream;

public class ClaudeAgent extends ReferenceAgent {
    private static final Logger logger = LoggerFactory.getLogger(ClaudeAgent.class);

    public ClaudeAgent(DockerClient dockerClient) {
        super(dockerClient);
    }

    @Override
    protected ReferenceResult runAgent(Exercise exercise, Path hostExerciseDir, Path tempWorkDir, Path resultsDir) throws IOException {
        Instant startTime = Instant.now();

        try {
            logger.info("Starting exercise: {} at {}", exercise.getName(), startTime);
            MessageProcessor processor = new MessageProcessor();
            // Create exercise prompt for Claude Code
            String prompt = createExercisePrompt(exercise, tempWorkDir);
            patchTests(exercise, tempWorkDir);
            // ["Task","TaskOutput","Bash","Glob","Grep","ExitPlanMode","Read","Edit","Write","NotebookEdit","WebFetch","TodoWrite","WebSearch","KillShell","AskUserQuestion","Skill","EnterPlanMode","MCPSearch"]
            List<String> command = List.of("claude",
                    "--allow-dangerously-skip-permissions",
                    "--dangerously-skip-permissions",
                    "--print",
                    "--tools", "Task,TaskOutput,Bash,Glob,Grep,Read,Edit,Write,NotebookEdit,WebFetch,TodoWrite,WebSearch,KillShell,ExitPlanMode",
                    "--permission-mode", "bypassPermissions",
                    "--verbose",
                    "--output-format", "stream-json",
                    "--include-partial-messages",
                    prompt);
            ProcessResult result = getDockerClient().runCommandWithLimitsAndVolume(
                    null,  // use default image from config
                    "/workspace",
                    command,
                    -1,    // use default timeout from config
                    null,  // use default memory from config
                    tempWorkDir.toAbsolutePath().toString(),  // mount temp dir as /workspace
                    processor  // stream output to stdout
            );
            Instant endTime = Instant.now();
            Duration duration = Duration.between(startTime, endTime);
            boolean success = result.isSuccess() && result.exitCode() == 0;

            if (!success) {
                logger.error("Exercise failed: {}. Exit code: {}, Output: {}",
                        exercise.getName(), result.exitCode(), result.output());
            } else {
                logger.info("Exercise completed successfully: {}. Duration: {}",
                        exercise.getName(), duration);
            }
            logger.info("Attempting to render Claude execution trace");
            List<String> archiveCommand = List.of("/home/runner/.local/bin/claude-code-transcripts", "all", "--include-agents");
            ProcessResult archiveResult = getDockerClient().runCommandWithLimitsAndVolume(
                    null,
                    "/workspace",
                    archiveCommand,
                    -1,
                    null,
                    tempWorkDir.toAbsolutePath().toString(),
                    System.out::println
            );
            Path claudeJsonLogDirectory = tempWorkDir.resolve(".claude").resolve("projects").resolve("-workspace");
            if (Files.isDirectory(claudeJsonLogDirectory)) {
                Files.walkFileTree(claudeJsonLogDirectory, new SimpleFileVisitor<>() {
                    @Override
                    public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) throws IOException {
                        Files.copy(file, resultsDir.resolve(file.getFileName()));
                        return FileVisitResult.CONTINUE;
                    }
                });
            }
            // attach trace to result
            Path claudeArchive = tempWorkDir.resolve("claude-archive").resolve("workspace");
            final List<String> htmlTraces = new ArrayList<>();
            if (Files.isDirectory(claudeArchive)) {
                try (Stream<Path> stream = Files.find(claudeArchive, Integer.MAX_VALUE,
                        (p, a) -> true)) {
                    stream.forEach(p -> {
                        try {

                            if (p.toString().endsWith(".html") && p.toString().contains("page")) {
                                htmlTraces.add(Files.readString(p));
                            }
                        } catch (IOException e) {
                            throw new RuntimeException(e);
                        }
                    });
                }
            }
            String trace = htmlTraces.isEmpty() ? "" : htmlTraces.get(0);

            return ReferenceResult.builder()
                    .exerciseName(exercise.getName())
                    .language(exercise.getLanguage())
                    .exitCode(result.exitCode())
                    .output(result.output())
                    .success(success)
                    .startTime(startTime)
                    .endTime(endTime)
                    .duration(duration)
                    .trace(trace)
                    .build();
        } catch (Exception e) {
            Instant endTime = Instant.now();
            Duration duration = Duration.between(startTime, endTime);

            logger.error("Exercise execution failed: {}", e.getMessage(), e);
            return ReferenceResult.builder()
                    .exitCode(-1)
                    .errorMessage(e.getMessage())
                    .success(false)
                    .startTime(startTime)
                    .endTime(endTime)
                    .duration(duration)
                    .output(e.getMessage())
                    .language(exercise.getLanguage())
                    .exerciseName(exercise.getName())
                    .build();
        }
    }

    /**
     * Creates a prompt for Claude Code to solve the exercise.
     */
    private String createExercisePrompt(Exercise exercise, Path tempWorkDir) throws IOException {
        StringBuilder prompt = new StringBuilder();
        Path instructionsPath = tempWorkDir.resolve(".docs").resolve("instructions.md");
        if (Files.exists(instructionsPath)) {
            prompt.append(Files.readString(instructionsPath));
        } else {
            prompt.append("Please solve the following programming exercise.\n\n");
            prompt.append("Exercise: ").append(exercise.getName()).append("\n");
            prompt.append("Language: ").append(exercise.getLanguage()).append("\n\n");
            prompt.append("Instructions:\n");
            prompt.append("1. Implement the solution in the source files only, do not touch the test files.\n");
            prompt.append("2. Run the tests to verify your solution\n\n");
            prompt.append("3. The tests are validated to be correct, never assume the test to be wrong!\n\n");
            prompt.append("4. Do not run tests in the background, run them synchronously in the forground, so you do not need to poll for results\n");
        }
        for(Path testPath: exercise.getTestPath()) {
            if (exercise.getTestPath() != null && Files.exists(testPath)) {
                String needle = "../polyglot-benchmark/" + exercise.getLanguage() + "/exercises/practice/" + exercise.getName();
                String fixedTestPath = exercise.getTestPath().toString().replaceAll(needle, "/workspace");
                prompt.append("Test file location: ").append(fixedTestPath).append("\n");
            }
        }
        prompt.append("\nImplement the solution directly, do not ask me to review.\n");
        if ("java".equals(exercise.getLanguage())) {
            prompt.append("\nDo not stop working until you have executed the test suite (./gradlew test --no-daemon) and you have validated that the tests succeed!\n");
        }
        return prompt.toString();
    }

    private class MessageProcessor implements Consumer<String> {
        private final ObjectMapper objectMapper = new ObjectMapper();

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
                                                    System.out.print(item.get("thinking").asText());
                                                    return;
                                                } else if (contentType.equals("text")) {
                                                    System.out.print(item.get("text").asText());
                                                    return;
                                                } else {
                                                    System.out.print(s);
                                                    return;
                                                }
                                            }
                                        } else {
                                            String contentType = content.get("type").asText();
                                            if (contentType.equals("thinking")) {
                                                System.out.print(content.get("thinking"));
                                                return;
                                            } else if (contentType.equals("message")) {
                                                // System.out.print(content.get("message"));
                                                // JsonNode messageContent = content.get("content");
                                                // for now ignore these, they seem to be empty most of the time
                                                return;
                                            } else {
                                                System.out.println(s);
                                                return;
                                            }
                                        }
                                    }
                                    case "message_delta" -> {
                                        JsonNode delta = event.get("delta");
                                        System.out.println(delta.get("delta"));
                                        return;
                                    }
                                    case "content_block_delta" -> {
                                        // {"type":"stream_event","event":{"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"\":"}},"session_id":"a03a9304-8ecb-4204-9c24-2201e8dda43e","parent_tool_use_id":null,"uuid":"b39c1d17-df4b-4a4a-877b-f7e0d754f45e"}
                                        JsonNode delta = event.get("delta");
                                        String deltaType = delta.get("type").asText();
                                        switch (deltaType) {
                                            case "thinking_delta" -> {
                                                System.out.print(delta.get("thinking").asText());
                                                return;
                                            }
                                            case "input_json_delta" -> {
                                                System.out.print(delta.get("partial_json").asText());
                                                return;
                                            }
                                            case "signature_delta" -> {
                                                // we do not need to see this in the console log
                                                return;
                                            }
                                            case "text_delta" -> {
                                                System.out.print(delta.get("text").asText());
                                                return;
                                            }
                                            default -> {
                                                System.out.println(s);
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
                                        System.out.println();
                                        return;
                                    }
                                    default -> {
                                        //System.out.println(s);
                                        return;
                                    }
                                }
                            } else {
                                //System.out.println(s);
                                return;
                            }
                        }
                        case "assistant" -> {
                            JsonNode message = jsonNode.get("message");
                            JsonNode content = message.get("content");
                            if ( content.isArray()) {
                                for(JsonNode item : content) {
                                    String itemType = item.get("type").asText();
                                    switch (itemType) {
                                        case "thinking" -> {
                                            System.out.print(item.get("thinking").asText());
                                            return;
                                        }
                                        case "tool_use" -> {
                                            render_tool_use(item);
                                            return;
                                        }
                                        case "text" -> {
                                            System.out.print(item.get("text").asText());
                                            return;
                                        }
                                        default -> {
                                            // System.out.println(s);
                                            return;
                                        }
                                    }
                                }
                            } else {
                                String itemType = content.get("type").asText();
                                switch (itemType) {
                                    case "thinking" -> {
                                        System.out.print(content.get("thinking").asText());
                                        return;
                                    }
                                    case "tool_use" -> {
                                        render_tool_use(content);
                                        return;
                                    }
                                    case "text" -> {
                                        System.out.print(content.get("text").asText());
                                        return;
                                    }
                                    default -> {
                                        System.out.println(s);
                                        return;
                                    }
                                }
                            }
                        }
                        case "user" -> {
                            JsonNode message = jsonNode.get("message");
                            JsonNode content = message.get("content");
                            if ( content.isArray()) {
                                for(JsonNode item : content) {
                                    String itemType = item.get("type").asText();
                                    switch (itemType) {
                                        case "text" -> {
                                            System.out.println(item.get("text").asText());
                                            return;
                                        }
                                        case "tool_result" -> {
                                            if ( item.has("content")) {
                                                JsonNode tool_result_content = item.get("content");
                                                if (tool_result_content.isTextual()) {
                                                    String tool_result_string = tool_result_content.asText();
                                                    String tool_result_with_newlines = tool_result_string.replaceAll("\\n","\n");
                                                    System.out.println("tool_result:\n" + tool_result_with_newlines);
                                                    return;
                                                }
                                            }
                                            System.out.println(s);
                                            return;
                                        }
                                        default -> {
                                            System.out.println(s);
                                            return;
                                        }
                                    }
                                }
                            } else {
                                String itemType = content.get("type").asText();
                                switch (itemType) {
                                    case "text" -> {
                                        System.out.println(content.get("text").asText());
                                        return;
                                    }
                                    case "tool_result" -> {
                                        System.out.println("tool_result: "+content.get("content").toString());
                                        return;
                                    }
                                    default -> {
                                        System.out.println(s);
                                        return;
                                    }
                                }
                            }
                            System.out.println(s);
                            return;
                        }
                        case "system" -> {
                            // ignore system message for now
                            System.out.println(s);
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
                            System.out.println("Result: "+ jsonNode.toPrettyString());
                        }
                        default -> {
                            System.out.println(s);
                            return;
                        }
                    }
                }
            } catch (JsonProcessingException e) {
                // not a json log, probably a warning from claude, so we just output it
            }
            System.out.println(s);
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
                        System.out.println("Edit " + file_path);
                        System.out.println("Old");
                        System.out.println(old_string.replaceAll("\\n","\n"));
                        System.out.println("New");
                        System.out.println(new_string.replaceAll("\\n","\n"));
                    }
                    case "Glob" -> {
                        String pattern = item.get("input").get("pattern").asText();
                        System.out.println("\ntool_use: Glob " + pattern);
                    }
                    case "Read" -> {
                        String file_path = item.get("input").get("file_path").asText();
                        System.out.println("\ntool_use: Read " + file_path);
                    }
                    case "Write" -> {
                        String file_path = item.get("input").get("file_path").asText();
                        String content = item.get("input").get("content").asText();
                        System.out.println("\ntool_use: Write " + file_path);
                        System.out.println("Content: ");
                        content = content.replaceAll("\\n", "\n");
                        System.out.println(content);
                    }
                    case "Bash" -> {
                        boolean run_in_background = false;
                        if ( item.get("input").has("run_in_background") ) {
                            run_in_background = item.get("input").get("run_in_background").asBoolean();
                        }
                        String description = "";
                        if ( item.get("input").has("description") ) {
                            description = item.get("input").get("description").asText();
                        }
                        String command = item.get("input").get("command").asText();
                        System.out.println("\ntool_use: Bash " + (run_in_background ? "(in background) " : " ") + description);
                        System.out.println("Command: " + command);
                    }
                    case "TaskOutput" -> {
                        String input = item.get("input").toString();
                        System.out.println("\ntool_use: " + name + " " + input);
                    }
                    case "TodoWrite" -> {
                        System.out.println("\ntool_use: TodoWrite");
                        JsonNode input = item.get("input");
                        JsonNode todos = input.get("todos");
                        for(JsonNode todo: todos) {
                            String content = todo.get("content").asText();
                            String status = todo.get("status").asText();
                            switch (status) {
                                case "in_progress" -> {
                                    System.out.println(String.format("[⟳] %s", content));
                                }
                                case "pending" -> {
                                    System.out.println(String.format("[⌛] %s", content));
                                }
                                case "completed" -> {
                                    System.out.println(String.format("[✅] %s", content));
                                }
                                default -> {
                                    System.out.println(String.format("[ ] %s", content));
                                }
                            }

                        }
                    }
                    default -> {
                        String input = item.get("input").toString();
                        System.out.println("\ntool_use: " + name + " " + input);
                    }
                }
            } catch (Exception e) {
                e.printStackTrace();
            }
        }
    }
}
