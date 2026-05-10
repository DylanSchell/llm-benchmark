package io.schell.llm.benchmark.config;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.dataformat.yaml.YAMLFactory;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Utility class for loading configuration from YAML files.
 */
public class ConfigLoader {
    private static final Logger logger = LoggerFactory.getLogger(ConfigLoader.class);

    private static final ObjectMapper mapper = new ObjectMapper(new YAMLFactory());

    /**
     * Loads configuration from a YAML file.
     *
     * @param configPath Path to the configuration file
     * @return Loaded configuration object
     * @throws IOException if the file cannot be read or validation fails
     */
    public static Config load(Path configPath) throws IOException {
        if (!Files.exists(configPath)) {
            throw new IOException("Configuration file not found: " + configPath);
        }

        String content = Files.readString(configPath.toAbsolutePath());

        // Set default values for null fields
        Config config = mapper.readValue(content, Config.class);
        setDefaults(config);

        // Validate configuration
        try {
            config.validate();
            logger.info("Configuration loaded and validated from: {}", configPath);
        } catch (ConfigurationException e) {
            throw new IOException("Configuration validation failed: " + e.getMessage(), e);
        }

        return config;
    }

    private static void setDefaults(Config config) {
        if (config.getDocker() == null) {
            config.setDocker(new DockerConfig());
        }
        if (config.getExercise() == null) {
            config.setExercise(new ExerciseConfig());
        }
        if (config.getClaude() == null) {
            config.setClaude(new ClaudeConfig());
        }
        if (config.getOutput() == null) {
            config.setOutput(new OutputConfig());
        }
    }
}
