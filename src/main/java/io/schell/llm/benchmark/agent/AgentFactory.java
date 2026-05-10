package io.schell.llm.benchmark.agent;

import io.schell.llm.benchmark.config.Config;
import io.schell.llm.benchmark.docker.DockerClient;

/**
 * Factory interface for creating agent instances.
 * Replaces reflection-based agent creation with a proper factory pattern.
 */
public interface AgentFactory {

    /**
     * Creates an agent instance.
     *
     * @param dockerClient The Docker client to use
     * @return A new agent instance
     */
    ReferenceAgent create(DockerClient dockerClient);

    /**
     * Gets the name of this agent factory.
     *
     * @return The agent name (e.g., "reference", "claude", "pi")
     */
    String getName();

    /**
     * Factory implementation for ReferenceAgent.
     */
    class ReferenceAgentFactory implements AgentFactory {
        @Override
        public ReferenceAgent create(DockerClient dockerClient) {
            return new ReferenceAgent(dockerClient);
        }

        @Override
        public String getName() {
            return "reference";
        }
    }

    /**
     * Factory implementation for ClaudeAgent.
     */
    class ClaudeAgentFactory implements AgentFactory {
        @Override
        public ReferenceAgent create(DockerClient dockerClient) {
            return new ClaudeAgent(dockerClient);
        }

        @Override
        public String getName() {
            return "claude";
        }
    }

    /**
     * Factory implementation for PiAgent.
     */
    class PiAgentFactory implements AgentFactory {
        @Override
        public ReferenceAgent create(DockerClient dockerClient) {
            return new PiAgent(dockerClient);
        }

        @Override
        public String getName() {
            return "pi";
        }
    }

    /**
     * Creates a factory registry with all available agents.
     *
     * @return Map of agent name to factory
     */
    static java.util.Map<String, AgentFactory> createRegistry() {
        var registry = new java.util.HashMap<String, AgentFactory>();
        registry.put("reference", new ReferenceAgentFactory());
        registry.put("claude", new ClaudeAgentFactory());
        registry.put("pi", new PiAgentFactory());
        return registry;
    }

    /**
     * Gets a factory by name from the default registry.
     *
     * @param name The agent name
     * @return The factory, or null if not found
     */
    static AgentFactory getFactory(String name) {
        return createRegistry().get(name);
    }

    /**
     * Creates an agent by name using the default registry.
     * Convenience method that combines lookup and creation.
     *
     * @param name         The agent name
     * @param dockerClient The Docker client to use
     * @return A new agent instance
     * @throws IllegalArgumentException if the agent name is not recognized
     */
    static ReferenceAgent createAgent(String name, DockerClient dockerClient) {
        AgentFactory factory = getFactory(name);
        if (factory == null) {
            throw new IllegalArgumentException(
                    "Unknown agent: " + name + ". Available agents: reference, claude, pi"
            );
        }
        return factory.create(dockerClient);
    }
}
