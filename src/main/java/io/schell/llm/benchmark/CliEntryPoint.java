package io.schell.llm.benchmark;

import io.schell.llm.benchmark.agent.ReferenceAgent;
import io.schell.llm.benchmark.config.Config;
import io.schell.llm.benchmark.config.ConfigLoader;
import io.schell.llm.benchmark.docker.DockerClient;
import io.schell.llm.benchmark.exercise.ExerciseResult;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

/**
 * Command-line entry point for the benchmark runner.
 * Handles argument parsing and CLI-specific flow.
 * 
 * Separated from BenchmarkRunner for better separation of concerns.
 */
public class CliEntryPoint {
    private static final Logger logger = LoggerFactory.getLogger(CliEntryPoint.class);

    /**
     * Main entry point for CLI execution.
     */
    public static void main(String[] args) {
        CliArgs cliArgs = parseArguments(args);

        if (cliArgs.webMode()) {
            startWebMode(cliArgs);
        } else {
            runCliBenchmark(cliArgs);
        }
    }

    /**
     * Parse command-line arguments.
     */
    private static CliArgs parseArguments(String[] args) {
        String configFile = "config.yaml";
        boolean webMode = false;
        int webPort = 8081;
        String model = null;
        String resultsDir = null;
        String language = "java";
        String exercise = null;
        String agent = "reference";

        try {
            for (int i = 0; i < args.length; i++) {
                if (args[i].equals("--config") && i + 1 < args.length) {
                    configFile = args[++i];
                } else if (args[i].equals("--web")) {
                    webMode = true;
                    // Check if port is specified as next argument
                    if (i + 1 < args.length) {
                        try {
                            int potentialPort = Integer.parseInt(args[++i]);
                            if (potentialPort > 0 && potentialPort < 65536) {
                                webPort = potentialPort;
                            } else {
                                i--;
                            }
                        } catch (NumberFormatException e) {
                            i--;
                        }
                    }
                } else if (args[i].equals("--port") && i + 1 < args.length) {
                    webPort = Integer.parseInt(args[++i]);
                } else if (args[i].equals("--model") && i + 1 < args.length) {
                    model = args[++i];
                } else if (args[i].equals("--results-dir") && i + 1 < args.length) {
                    resultsDir = args[++i];
                } else if (args[i].equals("--language") && i + 1 < args.length) {
                    language = args[++i];
                } else if (args[i].equals("--exercise") && i + 1 < args.length) {
                    exercise = args[++i];
                } else if (args[i].equals("--agent") && i + 1 < args.length) {
                    agent = args[++i];
                }
            }
        } catch (Exception e) {
            logger.error("Error parsing arguments: {}", e.getMessage());
            printUsage();
            System.exit(1);
        }

        return new CliArgs(configFile, webMode, webPort, model, resultsDir, language, exercise, agent);
    }

    /**
     * Start web mode.
     */
    private static void startWebMode(CliArgs cliArgs) {
        try {
            Path configPath = Paths.get(cliArgs.configFile());
            if (!configPath.toFile().exists()) {
                System.err.printf("%s not found in current directory%n", cliArgs.configFile());
                System.exit(1);
            }

            // Load config first
            Config config = ConfigLoader.load(configPath);

            // Apply command-line overrides
            if (cliArgs.model() != null) {
                config.setModel(cliArgs.model());
                logger.info("Overriding model from config with: {}", cliArgs.model());
            }
            if (cliArgs.resultsDir() != null) {
                config.getOutput().setResultsDir(cliArgs.resultsDir());
                logger.info("Overriding results_dir from config with: {}", cliArgs.resultsDir());
            }

            BenchmarkRunner runner = new BenchmarkRunner(config, new DockerClient(config.getDocker()));

            // Start Spring Boot application
            Class<?> webRunnerClass = Class.forName("io.schell.llm.benchmark.web.WebBenchmarkRunner");
            var method = webRunnerClass.getDeclaredMethod("runWebMode", String[].class);
            method.invoke(null, (Object) new String[]{ "--server.port=" + cliArgs.webPort() });
        } catch (Exception e) {
            System.err.println("Failed to start web interface: " + e.getMessage());
            e.printStackTrace();
            System.exit(1);
        }
    }

    /**
     * Run CLI benchmark.
     */
    private static void runCliBenchmark(CliArgs cliArgs) {
        try {
            Path configPath = Paths.get(cliArgs.configFile());
            if (!configPath.toFile().exists()) {
                System.err.printf("%s not found in current directory%n", cliArgs.configFile());
                System.exit(1);
            }

            // Load config first
            Config config = ConfigLoader.load(configPath);

            // Apply command-line overrides
            if (cliArgs.model() != null) {
                config.setModel(cliArgs.model());
                logger.info("Overriding model from config with: {}", cliArgs.model());
            }
            if (cliArgs.resultsDir() != null) {
                config.getOutput().setResultsDir(cliArgs.resultsDir());
                logger.info("Overriding results_dir from config with: {}", cliArgs.resultsDir());
            }

            BenchmarkRunner runner = new BenchmarkRunner(config, new DockerClient(config.getDocker()));

            if (!runner.isDockerAvailable()) {
                System.err.println("Docker is not available. Please ensure Docker is running.");
                System.exit(1);
            }

            // Create agent
            ReferenceAgent agent;
            try {
                agent = io.schell.llm.benchmark.agent.AgentFactory.createAgent(cliArgs.agent(), runner.getDockerClient());
            } catch (IllegalArgumentException e) {
                System.err.println(e.getMessage());
                System.exit(1);
                return; // Never reached
            }

            if (cliArgs.exercise() != null) {
                // Run single exercise
                ExerciseResult result;

                System.out.println("Running with " + cliArgs.agent() + " agent ...");
                result = runner.runReferenceExercise(agent, cliArgs.language(), cliArgs.exercise());
                printExerciseResult(result);

                // Save result
                runner.saveResult(result, cliArgs.agent(), cliArgs.language());
                System.exit(result.isSuccess() ? 0 : 1);
            } else {
                // Run all exercises
                System.out.println("Running all exercises with " + cliArgs.agent() + " agent ...");

                List<ExerciseResult> results = runner.runAllReferenceExercises(agent, cliArgs.language(), cliArgs.agent());
                runner.printSummary(results);

                // Save results
                runner.saveResults(results, cliArgs.agent(), cliArgs.language());

                long failed = results.stream().filter(r -> !r.isSuccess()).count();
                System.exit(failed > 0 ? 1 : 0);
            }
        } catch (Exception e) {
            logger.error("Failed to run benchmark: {}", e.getMessage(), e);
            System.exit(1);
        }
    }

    /**
     * Print exercise result to console.
     */
    private static void printExerciseResult(ExerciseResult result) {
        System.out.println("\n=== Exercise Result ===");
        System.out.println("Exercise: " + result.getExerciseName());
        System.out.println("Language: " + result.getLanguage());
        System.out.println("Success: " + result.isSuccess());
        System.out.println("Duration: " + result.getDuration());
        if (!result.isSuccess()) {
            System.out.println("\nOutput:");
            printOutput(result.getOutput(), "  ");
        }
    }

    /**
     * Print output with indentation.
     */
    private static void printOutput(String output, String indent) {
        if (output != null && !output.isEmpty()) {
            String[] lines = output.split("\n");
            for (String line : lines) {
                System.out.println(indent + line);
            }
        }
    }

    /**
     * Print usage information.
     */
    private static void printUsage() {
        System.out.println("Usage: java -jar claude-benchmark.jar [options]");
        System.out.println();
        System.out.println("Options:");
        System.out.println("  --config <file>       Config file (default: config.yaml)");
        System.out.println("  --web [port]          Start web interface (optional port, default: 8081)");
        System.out.println("  --port <port>         Web interface port");
        System.out.println("  --model <model>       Model name override");
        System.out.println("  --results-dir <dir>   Results directory override");
        System.out.println("  --language <lang>     Language (default: java)");
        System.out.println("  --exercise <name>     Exercise name (run single exercise)");
        System.out.println("  --agent <name>        Agent name: reference, claude, pi (default: reference)");
    }
}
