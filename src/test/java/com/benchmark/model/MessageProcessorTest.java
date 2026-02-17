package com.benchmark.model;

import com.benchmark.agent.ClaudeAgent;

public class MessageProcessorTest {
    private static final String S = "{\"type\":\"stream_event\",\"event\":{\"type\":\"message_start\",\"message\":{\"id\":\"chatcmpl-4soEXwH8RjX3rQb8f5xBNjjRh8ohcVL8\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"unsloth_Qwen3-Coder-Next-GGUF_UD-Q5_K_XL_Qwen3-Coder-Next-UD-Q5_K_XL-00001-of-00003.gguf\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":43774,\"output_tokens\":0}}},\"session_id\":\"daec3bf2-43d4-4c60-ac4e-16f7d7b798c6\",\"parent_tool_use_id\":null,\"uuid\":\"2122fbb2-230a-443e-b3bd-577b5fb32965\"}";

    public static void main(String[] args) {
        ClaudeAgent.MessageProcessor processor = new ClaudeAgent.MessageProcessor();
        processor.accept(S);
    }
}
