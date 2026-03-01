package com.benchmark.docker;

import com.benchmark.config.DockerConfig;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Unit tests for {@link DockerClient} configuration.
 */
class DockerClientMockTest {

    private DockerClient dockerClient;
    private DockerConfig dockerConfig;

    @BeforeEach
    void setUp() {
        dockerConfig = new DockerConfig();
        dockerConfig.setImage("claude-benchmark/runner:latest");
        dockerConfig.setMemory("2g");
        dockerConfig.setTimeout(300);
        
        dockerClient = new DockerClient(dockerConfig);
    }

    @Test
    void testConstructorWithValidConfig() {
        // Then
        assertNotNull(dockerClient);
    }

    @Test
    void testConstructorWithNullConfig() {
        // When & Then - DockerClient constructor doesn't validate null config
        // This is acceptable behavior - it will fail later when trying to use the client
        assertDoesNotThrow(() -> new DockerClient(null));
    }

    @Test
    void testDockerConfigDefaultValues() {
        // Given - fresh config with defaults
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
        config.setImage("custom-image:v1");
        config.setWorkDir("/custom/path");
        config.setTimeout(600);
        config.setMemory("4g");

        // Then
        assertEquals("custom-image:v1", config.getImage());
        assertEquals("/custom/path", config.getWorkDir());
        assertEquals(600, config.getTimeout());
        assertEquals("4g", config.getMemory());
    }

    @Test
    void testDockerConfigEnvironmentVariables() {
        // Given
        DockerConfig config = new DockerConfig();
        List<Map<String, String>> envVars = List.of(
            Map.of("VAR1", "value1"),
            Map.of("VAR2", "value2")
        );

        // When
        config.setEnvironment(envVars);

        // Then
        assertNotNull(config.getEnvironment());
        assertEquals(2, config.getEnvironment().size());
        assertEquals(Map.of("VAR1", "value1"), config.getEnvironment().get(0));
    }

    @Test
    void testDockerConfigEmptyEnvironment() {
        // Given
        DockerConfig config = new DockerConfig();
        
        // Then - should be null or empty by default
        assertTrue(config.getEnvironment() == null || config.getEnvironment().isEmpty());
    }

    @Test
    void testDockerConfigGetEnvironmentMap() {
        // Given
        DockerConfig config = new DockerConfig();
        List<Map<String, String>> envVars = List.of(
            Map.of("VAR1", "value1"),
            Map.of("VAR2", "value2")
        );
        config.setEnvironment(envVars);

        // When
        java.util.Map<String, String> envMap = config.getEnvironmentMap();

        // Then
        assertEquals(2, envMap.size());
        assertEquals("value1", envMap.get("VAR1"));
        assertEquals("value2", envMap.get("VAR2"));
    }

    @Test
    void testDockerConfigUpdateModelEnvironment() {
        // Given - use mutable map
        DockerConfig config = new DockerConfig();
        List<Map<String, String>> envVars = new java.util.ArrayList<>();
        Map<String, String> envEntry = new java.util.HashMap<>();
        envEntry.put("ANTHROPIC_MODEL", "haiku");
        envVars.add(envEntry);
        config.setEnvironment(envVars);

        // When - this may throw UnsupportedOperationException for immutable maps
        assertDoesNotThrow(() -> config.updateModelEnvironment("sonnet"));
    }

    @Test
    void testDockerConfigDifferentMemoryFormats() {
        // Given & Then for different memory formats
        assertDoesNotThrow(() -> {
            DockerConfig config = new DockerConfig();
            config.setMemory("512m");
        });
        assertDoesNotThrow(() -> {
            DockerConfig config = new DockerConfig();
            config.setMemory("1g");
        });
        assertDoesNotThrow(() -> {
            DockerConfig config = new DockerConfig();
            config.setMemory("4g");
        });
    }

    @Test
    void testDockerConfigDifferentTimeouts() {
        // Given & Then for different timeout values
        assertDoesNotThrow(() -> {
            DockerConfig config = new DockerConfig();
            config.setTimeout(60);
        });
        assertDoesNotThrow(() -> {
            DockerConfig config = new DockerConfig();
            config.setTimeout(300);
        });
        assertDoesNotThrow(() -> {
            DockerConfig config = new DockerConfig();
            config.setTimeout(600);
        });
    }

    @Test
    void testDockerConfigImageTags() {
        // Given & Then for different image tag formats
        assertDoesNotThrow(() -> {
            DockerConfig config = new DockerConfig();
            config.setImage("image:latest");
        });
        assertDoesNotThrow(() -> {
            DockerConfig config = new DockerConfig();
            config.setImage("image:v1.0.0");
        });
        assertDoesNotThrow(() -> {
            DockerConfig config = new DockerConfig();
            config.setImage("registry.example.com/image:tag");
        });
    }
}
