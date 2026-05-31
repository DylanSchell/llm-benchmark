//! Core benchmark runner logic for CLI mode.
//!
//! Port of Java's `BenchmarkRunner` — orchestrates agent creation, exercise execution,
//! result persistence, and summary output.

use std::sync::Arc;
use tracing::{error, info};
use benchmark_types::agent::AgentResult;
use benchmark_types::config::Config;
use benchmark_core::agent::{ClaudeAgent, ClaudeMessageProcessor, PiAgent, ReferenceAgent};
use benchmark_core::docker::DockerClient;
use benchmark_core::exercise_runner::ExerciseRunner;
use benchmark_core::persistence::ResultPersister;

use crate::RunArgs;

/// Run the full benchmark workflow.
pub async fn run(
    cli: &RunArgs,
    config: &Config,
    model: &str,
    retry: bool,
) -> anyhow::Result<()> {
    // Validate agent name
    match cli.agent.as_str() {
        "reference" | "claude" | "pi" => {}
        other => {
            error!(
                "Unsupported agent: '{}'. Supported agents: reference, claude, pi",
                other
            );
            std::process::exit(1);
        }
    }

    // Check Docker availability
    let docker_config = benchmark_core::docker::DockerConfig {
        image: config.docker.image.clone(),
        memory: Some(config.docker.memory.clone()),
        timeout: Some(config.docker.timeout as u64),
        work_dir: Some(config.docker.work_dir.clone()),
        environment: Some(config.docker.environment_map()),
        per_command_timeout: config.docker.per_command_timeout,
    };
    let docker_client = DockerClient::new(docker_config);

    if !docker_client.is_available().await {
        error!("Docker is not available. Please ensure Docker is running.");
        std::process::exit(1);
    }

    info!(
        "Starting benchmark: agent={}, model={}, language(s)={}",
        cli.agent,
        model,
        cli.language
    );

    // Create exercise runner
    let config_arc = Arc::new(config.clone());
    let exercise_runner = ExerciseRunner::new(config_arc);

    let results_dir = config.output.results_dir.clone();

    // Run exercises
    let results = if let Some(exercise_name) = &cli.exercise {
        // Single exercise mode
        info!("Running single exercise: {}", exercise_name);

        let agent = create_agent(&cli.agent, docker_client.clone(), cli.verbose)?;
        let result = exercise_runner
            .run_exercise(
                agent,
                &cli.language,
                exercise_name,
                model,
                None,
                &results_dir,
            )
            .await
            .map_err(|e| anyhow::anyhow!("Exercise failed: {}", e))?;

        // Print result
        print_single_result(&result);

        // Save individual result (retry=true increments attempts, retry=false overwrites)
        let persister = ResultPersister::new();
        if let Err(e) = persister.save_result(
            &result,
            &cli.agent,
            model,
            &results_dir,
            retry,
        ) {
            tracing::error!("Failed to save result: {}", e);
        }

        vec![result]
    } else {
        // All exercises mode — supports comma-separated languages
        let languages: Vec<String> = cli
            .language
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        if languages.is_empty() {
            anyhow::bail!("No languages specified");
        }

        let mut all_results: Vec<AgentResult> = Vec::new();

        for language in &languages {
            info!("Running all exercises for language: {}", language);

            let agent = create_agent(&cli.agent, docker_client.clone(), cli.verbose)?;

            // If retry mode, run_all_exercises won't skip already-completed exercises
            let results_for_lang = exercise_runner
                .run_all_exercises(
                    agent,
                    language,
                    &cli.agent,
                    model.to_string(),
                    None,
                    results_dir.clone(),
                    retry,
                )
                .await;

            info!(
                "Completed {} exercises for language: {}",
                results_for_lang.len(),
                language
            );

            all_results.extend(results_for_lang);
        }

        // Print summary
        let persister = ResultPersister::new();
        persister.print_summary(&all_results);

        // Save batch results — collect unique languages from results
        let languages_str: Vec<String> = all_results.iter()
            .map(|r| r.language.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        if let Err(e) = persister.save_results(
            &all_results,
            &cli.agent,
            model,
            &languages_str.join(","),
            &results_dir,
            retry,
        ) {
            tracing::error!("Failed to save results: {}", e);
        }

        all_results
    };

    // Exit with appropriate code
    let failed_count = results.iter().filter(|r| !r.success).count();
    if failed_count > 0 {
        error!("{} exercise(s) failed", failed_count);
        std::process::exit(1);
    }

    info!("All exercises passed!");
    Ok(())
}

/// Create an agent instance based on the CLI agent name.
fn create_agent(
    agent_name: &str,
    docker_client: DockerClient,
    verbose: bool,
) -> anyhow::Result<Arc<dyn benchmark_types::agent::Agent + Send + Sync>> {
    let agent: Arc<dyn benchmark_types::agent::Agent + Send + Sync> = match agent_name {
        "reference" => {
            let agent = ReferenceAgent::new(docker_client);
            if verbose {
                agent.set_output_consumer(|msg: &str| {
                    print!("{}", msg);
                });
            }
            Arc::new(agent)
        }
        "claude" => {
            let processor = if verbose {
                ClaudeMessageProcessor::new(Some(Box::new(|msg: &str| {
                    print!("{}", msg);
                })))
            } else {
                ClaudeMessageProcessor::new(None)
            };
            let mut agent = ClaudeAgent::new(docker_client);
            agent.set_message_processor(processor);
            Arc::new(agent)
        }
        "pi" => {
            let processor = if verbose {
                benchmark_core::agent::PiMessageProcessor::new(Some(Box::new(|msg: &str| {
                    print!("{}", msg);
                })))
            } else {
                benchmark_core::agent::PiMessageProcessor::new(None)
            };
            let mut agent = PiAgent::new(docker_client);
            agent.set_message_processor(processor);
            Arc::new(agent)
        }
        _ => unreachable!(), // validated in run()
    };
    Ok(agent)
}

/// Print a single exercise result to stdout.
fn print_single_result(result: &AgentResult) {
    println!("\n=== Exercise Result ===");
    println!("Exercise: {}", result.exercise_name);
    println!("Language: {}", result.language);
    println!("Success: {}", result.success);

    // Duration is stored in ms, display as seconds for CLI
    let duration_secs = result.duration_ms as f64 / 1000.0;
    println!("Duration: {:.1}s", duration_secs);

    if !result.success {
        println!("\nOutput:");
        if !result.output.is_empty() {
            // Show up to 20 lines of output, truncated at 200 chars each
            let lines: Vec<&str> = result.output.lines().collect();
            for line in lines.iter().take(20) {
                println!("  {}", line);
            }
            if lines.len() > 20 {
                println!("  ... ({} more lines)", lines.len() - 20);
            }
        }

        if let Some(ref error_msg) = result.error_message {
            println!("\nError: {}", error_msg);
        }
    }
}
