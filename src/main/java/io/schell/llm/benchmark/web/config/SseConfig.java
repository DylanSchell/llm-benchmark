package io.schell.llm.benchmark.web.config;

import org.springframework.context.annotation.Configuration;

/**
 * Configuration for SSE (Server-Sent Events) streaming.
 */
@Configuration
public class SseConfig {

    /**
     * SSE connection timeout in milliseconds (5 minutes).
     */
    public static final long TIMEOUT_MS = 300000;

    /**
     * Maximum buffer size for SSE events.
     */
    public static final int MAX_BUFFER_SIZE = 1000;
}
