package com.benchmark.web;

import com.benchmark.web.service.QueueProcessor;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.ComponentScan;

/**
 * Spring Boot application entry point for the web interface.
 * This runs alongside the existing CLI functionality.
 * All beans are defined in WebConfig - no manual registration needed.
 */
@SpringBootApplication
@ComponentScan(basePackages = {"com.benchmark.web", "com.benchmark.config"})
public class WebBenchmarkRunner {
    private static final Logger logger = LoggerFactory.getLogger(WebBenchmarkRunner.class);

    /**
     * Main entry point for web mode.
     * Starts the Spring Boot application.
     *
     * @param args Command line arguments (should include --server.port from caller)
     */
    public static void runWebMode(String[] args) {
        logger.info("Starting web interface...");

        SpringApplication app = new SpringApplication(WebBenchmarkRunner.class);
        var context = app.run(args);

        // Extract port from args for logging
        int port = 8081; // default
        for (String arg : args) {
            if (arg.startsWith("--server.port=")) {
                try {
                    port = Integer.parseInt(arg.substring("--server.port=".length()));
                } catch (NumberFormatException e) {
                    // Use default
                }
                break;
            }
        }

        logger.info("Web interface started successfully on port " + port);

        // Register shutdown hook to ensure clean termination
        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            logger.info("Shutdown signal received, shutting down web interface...");
            try {
                // First, signal QueueProcessor to stop accepting new work
                QueueProcessor queueProcessor = context.getBean(QueueProcessor.class);
                if (queueProcessor != null) {
                    logger.info("Signaling queue processor to shut down...");
                    // Use reflection to call the @PreDestroy method since it's not public
                    try {
                        var shutdownMethod = queueProcessor.getClass().getDeclaredMethod("shutdown");
                        shutdownMethod.setAccessible(true);
                        shutdownMethod.invoke(queueProcessor);
                    } catch (Exception e) {
                        logger.warn("Could not signal queue processor shutdown: {}", e.getMessage());
                    }
                }

                // Close the Spring context (this triggers @PreDestroy on all beans)
                logger.info("Closing Spring application context...");
                context.close();
                
                // Wait a bit for threads to terminate
                Thread.sleep(2000);
                
                logger.info("Web interface shut down complete");
            } catch (Exception e) {
                logger.error("Error during shutdown: {}", e.getMessage(), e);
            }
        }, "web-shutdown-hook"));
    }
}
