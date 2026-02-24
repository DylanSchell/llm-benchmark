package com.benchmark.web;

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
     * @param args Command line arguments (passed from main runner)
     */
    public static void runWebMode(String[] args) {
        logger.info("Starting web interface...");

        // Parse port from args - default to 8081 (as in application.properties)
        int port = 8081;
        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--web") && i + 1 < args.length) {
                try {
                    int potentialPort = Integer.parseInt(args[++i]);
                    if (potentialPort > 0 && potentialPort < 65536) {
                        port = potentialPort;
                    } else {
                        i--;
                    }
                } catch (NumberFormatException e) {
                    i--;
                }
            } else if (args[i].equals("--port") && i + 1 < args.length) {
                int potentialPort = Integer.parseInt(args[++i]);
                if (potentialPort > 0 && potentialPort < 65536) {
                    port = potentialPort;
                } else {
                    i--;
                }
            }
        }

        final int webPort = port;

        // Build args with server.port override
        java.util.List<String> argList = new java.util.ArrayList<>();
        argList.add("--server.port=" + webPort);
        for (String arg : args) {
            if (!arg.equals("--web") && !arg.equals("--port")) {
                try {
                    Integer.parseInt(arg); // Skip port values
                } catch (NumberFormatException e) {
                    argList.add(arg);
                }
            }
        }

        SpringApplication app = new SpringApplication(WebBenchmarkRunner.class);
        var context = app.run(argList.toArray(new String[0]));

        logger.info("Web interface started successfully on port " + webPort);

        // Register shutdown hook to ensure clean termination
        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            logger.info("Shutting down web interface...");
            context.close();
        }));
    }
}
