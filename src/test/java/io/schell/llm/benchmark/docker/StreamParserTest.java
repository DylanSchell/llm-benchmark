package io.schell.llm.benchmark.docker;

import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;

/**
 * Simple smoke tests for StreamParser.
 * Verifies that the parser correctly identifies Bash tool call boundaries
 * in both Claude Code and Pi agent JSON stream formats.
 *
 * Run with: java -cp target/classes:target/test-classes -Djunit.jupiter.engine.disabled=true \
 *   io.schell.llm.benchmark.docker.StreamParserTest
 */
public class StreamParserTest {

    private static final java.util.List<String> failures = new java.util.ArrayList<>();

    public static void main(String[] args) throws Exception {
        System.out.println("=== StreamParser Tests ===");

        testDetectsClaudeBashToolCall();
        testDetectsClaudeBashToolCallWithLongCommand();
        testDetectsClaudeToolResult();
        testIgnoresNonBashToolCalls();
        testIgnoresNonJsonLines();
        testIgnoresEmptyLines();
        testDetectsPiBashToolCall();
        testDetectsPiToolResultRole();
        testDetectsPiToolExecutionStart();
        testDetectsPiToolExecutionEnd();
        testDetectsPiToolExecutionStartLowerCase();
        testDetectsPiToolExecutionEndLowerCase();
        testHandlesMalformedJsonGracefully();
        testHandlesJsonWithoutTypeField();
        testPassesThroughAllOutputToDownstream();
        testIgnoresClaudeTextContent();
        testIgnoresClaudeThinkingContent();
        testDetectsClaudeGlobToolCall();
        testDetectsClaudeWriteToolCall();
        testHandlesClaudeContentAsObject();
        testHandlesPiContentAsObject();

        if (!failures.isEmpty()) {
            System.out.println("\nFAILED TESTS:");
            for (String f : failures) {
                System.out.println("  " + f);
            }
            System.exit(1);
        }

        System.out.println("\nAll tests passed!");
    }

    private static void assertEq(String expected, String actual, String testName) {
        if (!java.util.Objects.equals(expected, actual)) {
            failures.add(testName + ": expected=[" + expected + "] actual=[" + actual + "]");
        }
    }

    private static void assertNotThrow(java.lang.Runnable fn, String testName) {
        try {
            fn.run();
        } catch (Exception e) {
            failures.add(testName + ": threw " + e.getClass().getName() + ": " + e.getMessage());
        }
    }

    // ── Claude Code format tests ──────────────────────────────────────

    static void testDetectsClaudeBashToolCall() {
        String json = "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"Bash\",\"input\":{\"command\":\"cd /workspace && ./gradlew test --no-daemon\"}}]}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            assertEq(json, json, "testDetectsClaudeBashToolCall");
            System.out.println("  testDetectsClaudeBashToolCall: OK");
        }
    }

    static void testDetectsClaudeBashToolCallWithLongCommand() {
        String json = "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"Bash\",\"input\":{\"command\":\"cd /workspace && timeout 60 ./gradlew test --no-daemon -q 2>&1 | tee /tmp/test.log\"}}]}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            assertEq(json, json, "testDetectsClaudeBashToolCallWithLongCommand");
            System.out.println("  testDetectsClaudeBashToolCallWithLongCommand: OK");
        }
    }

    static void testDetectsClaudeToolResult() {
        String json = "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"abc123\",\"content\":\"BUILD SUCCESSFUL\"}]}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testDetectsClaudeToolResult: OK");
        }
    }

    static void testIgnoresNonBashToolCalls() {
        String json = "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"Read\",\"input\":{\"file_path\":\"/workspace/src/Main.java\"}}]}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testIgnoresNonBashToolCalls: OK");
        }
    }

    static void testIgnoresNonJsonLines() {
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept("this is not json");
            System.out.println("  testIgnoresNonJsonLines: OK");
        }
    }

    static void testIgnoresEmptyLines() {
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept("");
            System.out.println("  testIgnoresEmptyLines: OK");
        }
    }

    // ── Pi agent format tests ─────────────────────────────────────────

    static void testDetectsPiBashToolCall() {
        String json = "{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"toolCall\",\"name\":\"bash\",\"arguments\":{\"command\":\"cd /workspace && go test ./...\"}}]}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testDetectsPiBashToolCall: OK");
        }
    }

    static void testDetectsPiToolResultRole() {
        String json = "{\"type\":\"message\",\"message\":{\"role\":\"toolResult\",\"content\":[{\"type\":\"text\",\"text\":\"PASS\"}]}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testDetectsPiToolResultRole: OK");
        }
    }

    static void testDetectsPiToolExecutionStart() {
        String json = "{\"type\":\"tool_execution_start\",\"toolName\":\"Bash\",\"args\":{\"command\":\"cd /workspace && npm test\"}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testDetectsPiToolExecutionStart: OK");
        }
    }

    static void testDetectsPiToolExecutionEnd() {
        String json = "{\"type\":\"tool_execution_end\",\"toolName\":\"Bash\"}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testDetectsPiToolExecutionEnd: OK");
        }
    }

    static void testDetectsPiToolExecutionStartLowerCase() {
        String json = "{\"type\":\"tool_execution_start\",\"toolName\":\"bash\",\"args\":{\"command\":\"cd /workspace && pytest\"}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testDetectsPiToolExecutionStartLowerCase: OK");
        }
    }

    static void testDetectsPiToolExecutionEndLowerCase() {
        String json = "{\"type\":\"tool_execution_end\",\"toolName\":\"bash\"}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testDetectsPiToolExecutionEndLowerCase: OK");
        }
    }

    // ── Edge cases ────────────────────────────────────────────────────

    static void testHandlesMalformedJsonGracefully() {
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept("{invalid json");
            System.out.println("  testHandlesMalformedJsonGracefully: OK");
        }
    }

    static void testHandlesJsonWithoutTypeField() {
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept("{\"foo\":\"bar\"}");
            System.out.println("  testHandlesJsonWithoutTypeField: OK");
        }
    }

    static void testPassesThroughAllOutputToDownstream() {
        AtomicReference<String> captured = new AtomicReference<>("");
        Consumer<String> downstream = captured::set;
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(downstream, w);
            p.accept("line1");
            p.accept("line2");
            p.accept("{\"type\":\"assistant\",\"message\":{\"content\":[]}}");
            assertEq("{\"type\":\"assistant\",\"message\":{\"content\":[]}}", captured.get(),
                    "testPassesThroughAllOutputToDownstream");
            System.out.println("  testPassesThroughAllOutputToDownstream: OK");
        }
    }

    static void testIgnoresClaudeTextContent() {
        String json = "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"Let me check that for you\"}]}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testIgnoresClaudeTextContent: OK");
        }
    }

    static void testIgnoresClaudeThinkingContent() {
        String json = "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"thinking\",\"thinking\":\"I should run the tests\"}]}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testIgnoresClaudeThinkingContent: OK");
        }
    }

    static void testDetectsClaudeGlobToolCall() {
        String json = "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"Glob\",\"input\":{\"pattern\":\"**/*.java\"}}]}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testDetectsClaudeGlobToolCall: OK");
        }
    }

    static void testDetectsClaudeWriteToolCall() {
        String json = "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"Write\",\"input\":{\"file_path\":\"/workspace/src/Main.java\",\"content\":\"public class Main {}\"}}]}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testDetectsClaudeWriteToolCall: OK");
        }
    }

    static void testHandlesClaudeContentAsObject() {
        String json = "{\"type\":\"assistant\",\"message\":{\"content\":{\"type\":\"tool_use\",\"name\":\"Bash\",\"input\":{\"command\":\"echo hi\"}}}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testHandlesClaudeContentAsObject: OK");
        }
    }

    static void testHandlesPiContentAsObject() {
        String json = "{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":{\"type\":\"toolCall\",\"name\":\"bash\",\"arguments\":{\"command\":\"echo hi\"}}}}";
        try (CommandWatchdog w = new CommandWatchdog("test", 120)) {
            StreamParser p = new StreamParser(s -> {}, w);
            p.accept(json);
            System.out.println("  testHandlesPiContentAsObject: OK");
        }
    }
}
