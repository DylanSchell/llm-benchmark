pub mod config;
pub mod agent;
pub mod docker;
pub mod exercise;
pub mod model;
pub mod cli;
pub mod analyzer;
pub mod result_saver;

use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use tracing::info;

pub async fn run_benchmark(
    config_path: &str,
    language: &str,
    agent_name: &str,
    exercise_name: Option<&str>,
) -> Result<()> {
    info!("Loading configuration from: {}", config_path);
    let config = config::Config::load(config_path)?;
    let config = Arc::new(config);

    let output = &config.output;
    if !output.results_dir.exists() {
        std::fs::create_dir_all(&output.results_dir)?;
    }

    let docker_config = docker::DockerConfig {
        image: config.docker.image.clone(),
        memory: Some(config.docker.memory.clone()),
        timeout: Some(config.docker.timeout as u64),
        work_dir: Some(config.docker.work_dir.clone()),
        environment: Some(config.docker.environment_map()),
    };
    let docker_client = docker::DockerClient::new(docker_config);

    info!("Starting benchmark run for language: {} with agent: {}", language, agent_name);

    let exercise_runner = exercise::ExerciseRunner::new(config.clone());

    let results = if let Some(exercise) = exercise_name {
        let agent: Arc<Mutex<dyn agent::Agent + Send + Sync>> = if agent_name == "reference" {
            Arc::new(Mutex::new(agent::ReferenceAgent::new(docker_client.clone())))
        } else {
            Arc::new(Mutex::new(agent::ClaudeAgent::new(docker_client.clone())))
        };
        match exercise_runner.run_exercise(agent, language, exercise).await {
            Ok(r) => vec![r],
            Err(e) => return Err(anyhow::anyhow!("Exercise failed: {}", e)),
        }
    } else {
        if agent_name == "reference" {
            let reference_agent = agent::ReferenceAgent::new(docker_client.clone());
            let reference_agent = Arc::new(Mutex::new(reference_agent));
            exercise_runner
                .run_all_exercises(reference_agent, language, agent_name)
                .await
        } else {
            let claude_agent = agent::ClaudeAgent::new(docker_client.clone());
            let claude_agent = Arc::new(Mutex::new(claude_agent));
            exercise_runner
                .run_all_exercises(claude_agent, language, agent_name)
                .await
        }
    };

    info!("Benchmark run complete. {} exercises processed.", results.len());

    let result_saver = result_saver::ResultSaver::new();
    result_saver.print_summary(&results);
    let _ = result_saver.save_results(&results, agent_name, language, &output.results_dir);

    Ok(())
}

pub async fn analyze_results(results_dir: &str, output_path: &str) -> Result<()> {
    let analyzer = analyzer::BenchmarkAnalyzer::new();
    analyzer.analyze(results_dir, output_path)?;
    Ok(())
}
