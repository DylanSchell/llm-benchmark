pub mod reference;
pub mod claude;
pub mod pi;
pub mod claude_message_processor;
pub mod pi_message_processor;
pub mod test_patches;
pub mod exercise_files;

pub use reference::ReferenceAgent;
pub use test_patches::{run_patch_tests, run_remove_ignore_annotations, run_replace_xtest, run_remove_disabled_annotations};
pub use claude::ClaudeAgent;
pub use pi::PiAgent;
pub use claude_message_processor::ClaudeMessageProcessor;
pub use pi_message_processor::PiMessageProcessor;
pub use exercise_files::{copy_exercise_files, create_temp_work_dir};

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use benchmark_types::util::recover_poisoned;
    use benchmark_types::agent::Agent;
    use crate::docker::{DockerClient, DockerConfig};
    use std::collections::HashMap;

    fn create_test_docker_client() -> DockerClient {
        let config = DockerConfig {
            image: "test-image:latest".to_string(),
            memory: "1g".to_string(),
            timeout: 300,
            work_dir: "/workspace".to_string(),
            environment: HashMap::new(),
            per_command_timeout: 600,
        };
        DockerClient::new(config)
    }

    #[test]
    fn test_create_reference_agent() {
        let docker_client = create_test_docker_client();
        let agent = ReferenceAgent::new(docker_client);
        assert_eq!(agent.get_name(), "reference");
    }

    #[test]
    fn test_create_claude_agent() {
        let docker_client = create_test_docker_client();
        let agent = ClaudeAgent::new(docker_client);
        assert_eq!(agent.get_name(), "claude");
    }

    #[test]
    fn test_create_pi_agent() {
        let docker_client = create_test_docker_client();
        let agent = PiAgent::new(docker_client);
        assert_eq!(agent.get_name(), "pi");
    }

    #[test]
    fn test_reference_agent_has_output_consumer() {
        let docker_client = create_test_docker_client();
        let agent = ReferenceAgent::new(docker_client);
        // Setting output consumer should not panic
        agent.set_output_consumer(|msg: &str| {
            let _ = msg;
        });
    }

    #[test]
    fn test_reference_agent_emit_output_with_consumer() {
        let docker_client = create_test_docker_client();
        let agent = ReferenceAgent::new(docker_client);

        let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_clone = captured.clone();
        agent.set_output_consumer(move |msg: &str| {
            recover_poisoned(captured_clone.lock()).push(msg.to_string());
        });

        // emit_output is internal, but we can verify the consumer was set
        // by checking the internal state
    }

    #[test]
    fn test_agent_trait_is_object_safe() {
        // Verify that Agent trait can be used as a trait object
        let docker_client = create_test_docker_client();
        let agent: Box<dyn Agent + Send + Sync> = Box::new(ReferenceAgent::new(docker_client));
        assert_eq!(agent.get_name(), "reference");
    }
}
