pub mod docker;
pub mod exercise_runner;
pub mod parallel;
pub mod persistence;
pub mod agent;

use std::str::FromStr;
use std::sync::Arc;
use anyhow::Result;
use tracing::info;
use benchmark_types::agent::{Agent, AgentKind};
use benchmark_types::config::Config;
use crate::docker::{DockerClient, DockerConfig};
use crate::exercise_runner::ExerciseRunner;
use crate::agent::{ReferenceAgent, ClaudeAgent, PiAgent};
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

    let agent_kind = AgentKind::from_str(agent_name).map_err(|e| anyhow::anyhow!("{}", e))?;

    let output = &config.output;
    if !output.results_dir.exists() {
        std::fs::create_dir_all(&output.results_dir)?;
    }

    let docker_config = DockerConfig::from(&config.docker);
    let docker_client = DockerClient::new(docker_config);

    info!("Starting benchmark run for language: {} with agent: {}", language, agent_kind);

    let exercise_runner = ExerciseRunner::new(config.clone());

    let results = if let Some(exercise) = exercise_name {
        let agent = make_agent(agent_kind, docker_client.clone());
        match exercise_runner.run_exercise(agent, language, exercise, model, None, &output.results_dir).await {
            Ok(r) => vec![r],
            Err(e) => return Err(anyhow::anyhow!("Exercise failed: {}", e)),
        }
    } else {
        let agent = make_agent(agent_kind, docker_client.clone());
        exercise_runner
            .run_all_exercises(agent, language, &agent_kind.to_string(), model.to_string(), None, output.results_dir.clone(), false)
            .await
    };

    info!("Benchmark run complete. {} exercises processed.", results.len());

    let persister = ResultPersister::new();
    persister.print_summary(&results);
    // Batch save doesn't use retry logic - just saves summary file
    let _ = persister.save_results(&results, &agent_kind.to_string(), model, language, &output.results_dir);

    Ok(())
}

fn make_agent(kind: AgentKind, docker_client: DockerClient) -> Arc<dyn Agent + Send + Sync> {
    match kind {
        AgentKind::Reference => Arc::new(ReferenceAgent::new(docker_client)),
        AgentKind::Claude => Arc::new(ClaudeAgent::new(docker_client)),
        AgentKind::Pi => Arc::new(PiAgent::new(docker_client)),
    }
}
