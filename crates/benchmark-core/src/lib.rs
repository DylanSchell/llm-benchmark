pub mod docker;
pub mod exercise_runner;
pub mod parallel;
pub mod persistence;
pub mod agent;

use std::sync::Arc;
use anyhow::Result;
use tracing::info;
use benchmark_types::agent::Agent;
use benchmark_types::config::Config;
use crate::docker::{DockerClient, DockerConfig};
use crate::exercise_runner::ExerciseRunner;
use crate::agent::{ReferenceAgent, ClaudeAgent};
use crate::persistence::ResultPersister;

pub async fn run_benchmark(
    config_path: &str,
    language: &str,
    agent_name: &str,
    exercise_name: Option<&str>,
    model: &str,
) -> Result<()> {
    info!("Loading configuration from: {}", config_path);
    let config = Config::load(config_path)?;
    let config = Arc::new(config);

    let output = &config.output;
    if !output.results_dir.exists() {
        std::fs::create_dir_all(&output.results_dir)?;
    }

    let docker_config = DockerConfig {
        image: config.docker.image.clone(),
        memory: Some(config.docker.memory.clone()),
        timeout: Some(config.docker.timeout as u64),
        work_dir: Some(config.docker.work_dir.clone()),
        environment: Some(config.docker.environment_map()),
        per_command_timeout: config.docker.per_command_timeout,
    };
    let docker_client = DockerClient::new(docker_config);

    info!("Starting benchmark run for language: {} with agent: {}", language, agent_name);

    let exercise_runner = ExerciseRunner::new(config.clone());

    let results = if let Some(exercise) = exercise_name {
        let agent: Arc<dyn Agent + Send + Sync> = if agent_name == "reference" {
            Arc::new(ReferenceAgent::new(docker_client.clone()))
        } else {
            Arc::new(ClaudeAgent::new(docker_client.clone()))
        };
        match exercise_runner.run_exercise(agent, language, exercise, model, &output.results_dir).await {
            Ok(r) => vec![r],
            Err(e) => return Err(anyhow::anyhow!("Exercise failed: {}", e)),
        }
    } else {
        if agent_name == "reference" {
            let reference_agent = ReferenceAgent::new(docker_client.clone());
            exercise_runner
                .run_all_exercises(Arc::new(reference_agent), language, agent_name, model.to_string(), output.results_dir.clone(), false)
                .await
        } else {
            let claude_agent = ClaudeAgent::new(docker_client.clone());
            exercise_runner
                .run_all_exercises(Arc::new(claude_agent), language, agent_name, model.to_string(), output.results_dir.clone(), false)
                .await
        }
    };

    info!("Benchmark run complete. {} exercises processed.", results.len());

    let persister = ResultPersister::new();
    persister.print_summary(&results);
    // Batch save doesn't use retry logic - just saves summary file
    let _ = persister.save_results(&results, agent_name, model, language, &output.results_dir, false);

    Ok(())
}

pub async fn analyze_results(_results_dir: &str, _output_path: &str) -> Result<()> {
    // Placeholder — analyzer lives in a separate crate (benchmark-analyzer)
    anyhow::bail!("Analyzer not yet implemented in benchmark-core");
}
