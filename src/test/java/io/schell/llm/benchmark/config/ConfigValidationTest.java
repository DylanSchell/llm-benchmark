package io.schell.llm.benchmark.config;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Unit tests for configuration classes.
 */
class ConfigValidationTest {

    @Test
    void testDockerConfigDefaultValues() {
        // Given
        DockerConfig config = new DockerConfig();

        // Then
        assertEquals("claude-benchmark-runner:latest", config.getImage());
        assertEquals("/workspace", config.getWorkDir());
        assertEquals(300, config.getTimeout());
        assertEquals("2g", config.getMemory());
    }

    @Test
    void testDockerConfigSetters() {
        // Given
        DockerConfig config = new DockerConfig();

        // When
        config.setImage("custom:tag");
        config.setWorkDir("/custom");
        config.setTimeout(600);
        config.setMemory("4g");

        // Then
        assertEquals("custom:tag", config.getImage());
        assertEquals("/custom", config.getWorkDir());
        assertEquals(600, config.getTimeout());
        assertEquals("4g", config.getMemory());
    }

    @Test
    void testOutputConfigDefaultValues() {
        // Given
        OutputConfig config = new OutputConfig();

        // Then
        assertEquals("../benchmark-results", config.getResultsDir());
        assertEquals("INFO", config.getLogLevel());
    }

    @Test
    void testOutputConfigSetters() {
        // Given
        OutputConfig config = new OutputConfig();

        // When
        config.setResultsDir("./custom-results");
        config.setLogLevel("DEBUG");

        // Then
        assertEquals("./custom-results", config.getResultsDir());
        assertEquals("DEBUG", config.getLogLevel());
    }

    @Test
    void testConfigDefaultValues() {
        // Given
        Config config = new Config();

        // Then - verify defaults are set (some may be null until loaded from YAML)
        assertEquals("../polyglot-benchmark", config.getBenchmarkPath().toString());
        assertEquals(1, config.getParallelism());
    }

    @Test
    void testConfigSetters() {
        // Given
        Config config = new Config();
        
        DockerConfig dockerConfig = new DockerConfig();
        dockerConfig.setImage("test-image");
        
        OutputConfig outputConfig = new OutputConfig();
        outputConfig.setResultsDir("./test-results");

        // When
        config.setDocker(dockerConfig);
        config.setOutput(outputConfig);
        config.setBenchmarkPath("/custom/path");
        config.setParallelism(8);

        // Then
        assertEquals("test-image", config.getDocker().getImage());
        assertEquals("./test-results", config.getOutput().getResultsDir());
        assertEquals("/custom/path", config.getBenchmarkPath().toString());
        assertEquals(8, config.getParallelism());
    }

    @Test
    void testDockerConfigValidation() throws Exception {
        // Given - valid config
        DockerConfig config = new DockerConfig();
        
        // When & Then - should not throw
        assertDoesNotThrow(config::validate);
    }

    @Test
    void testDockerConfigValidationWithEmptyImage() {
        // Given
        DockerConfig config = new DockerConfig();
        config.setImage("");

        // When & Then
        assertThrows(ConfigurationException.class, config::validate);
    }

    @Test
    void testDockerConfigValidationWithLowTimeout() {
        // Given
        DockerConfig config = new DockerConfig();
        config.setTimeout(5);

        // When & Then
        assertThrows(ConfigurationException.class, config::validate);
    }

    @Test
    void testOutputConfigValidation() throws Exception {
        // Given - valid config
        OutputConfig config = new OutputConfig();
        
        // When & Then - should not throw
        assertDoesNotThrow(config::validate);
    }

    @Test
    void testOutputConfigValidationWithEmptyResultsDir() {
        // Given
        OutputConfig config = new OutputConfig();
        config.setResultsDir("");

        // When & Then
        assertThrows(ConfigurationException.class, config::validate);
    }

    @Test
    void testFullConfigValidationRequiresDocker() {
        // Given - config without docker (default)
        Config config = new Config();
        
        // When & Then - should throw because docker is required
        assertThrows(ConfigurationException.class, () -> config.validate());
    }
}
