package com.benchmark.agent;

import com.benchmark.docker.DockerClient;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Unit tests for {@link AgentFactory}.
 */
class AgentFactoryTest {

    private DockerClient createMockDockerClient() {
        // Create a minimal DockerClient for testing
        com.benchmark.config.DockerConfig config = new com.benchmark.config.DockerConfig();
        return new DockerClient(config);
    }

    @Test
    void testCreateReferenceAgent() {
        // When
        ReferenceAgent agent = AgentFactory.createAgent("reference", createMockDockerClient());

        // Then
        assertNotNull(agent);
        assertEquals("reference", agent.getName());
    }

    @Test
    void testCreateClaudeAgent() {
        // When
        ReferenceAgent agent = AgentFactory.createAgent("claude", createMockDockerClient());

        // Then
        assertNotNull(agent);
        assertEquals("claude", agent.getName());
    }

    @Test
    void testCreatePiAgent() {
        // When
        ReferenceAgent agent = AgentFactory.createAgent("pi", createMockDockerClient());

        // Then
        assertNotNull(agent);
        assertEquals("pi", agent.getName());
    }

    @Test
    void testCreateUnknownAgentThrowsException() {
        // When & Then
        assertThrows(IllegalArgumentException.class, () -> {
            AgentFactory.createAgent("unknown-agent", createMockDockerClient());
        });
    }

    @Test
    void testCreateAgentWithEmptyString() {
        // When & Then
        assertThrows(IllegalArgumentException.class, () -> {
            AgentFactory.createAgent("", createMockDockerClient());
        });
    }

    @Test
    void testCreateAgentWithNullString() {
        // When & Then
        assertThrows(IllegalArgumentException.class, () -> {
            AgentFactory.createAgent(null, createMockDockerClient());
        });
    }

    @Test
    void testCreateAgentCaseInsensitive() {
        // When - lowercase works
        ReferenceAgent agent = AgentFactory.createAgent("reference", createMockDockerClient());

        // Then
        assertNotNull(agent);
    }
}
