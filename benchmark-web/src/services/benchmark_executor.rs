//! BenchmarkExecutor - mirrors Java BenchmarkExecutor.java
//! Executes benchmark runs for a given session.
//! Integrates with benchmark-core's ExerciseRunner and ResultPersister.

use crate::models::session::BenchmarkSession;
use crate::models::status::RunStatus;
use crate::services::result_service::ResultService;
use crate::services::session_manager::SessionManager;
use anyhow::{Context, Result};
use benchmark_core::agent::{ReferenceAgent, ClaudeAgent, PiAgent, ClaudeMessageProcessor, PiMessageProcessor};
use benchmark_types::agent::Agent;
use benchmark_core::docker::DockerClient;
use benchmark_core::exercise_runner::ExerciseRunner;
use benchmark_core::persistence::ResultPersister;
use benchmark_types::agent::AgentResult;
use benchmark_types::config::Config;
use std::sync::Arc;
use tracing::{info, error, warn};

/// Configuration for benchmark execution.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Path to config.yaml
    pub config_path: String,
    /// Override results directory (from RESULTS_DIR env var).
    /// If set, overrides output.results_dir from config.yaml.
    pub results_dir_override: Option<std::path::PathBuf>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            config_path: "config.yaml".to_string(),
            results_dir_override: None,
        }
    }
}

/// Executes benchmark runs.
/// Integrates with benchmark-core's ExerciseRunner for actual Docker execution.
#[derive(Clone)]
pub struct BenchmarkExecutor {
    config: ExecutorConfig,
    pub(crate) docker_client: Arc<DockerClient>,
    pub(crate) exercise_runner: Arc<ExerciseRunner>,
    pub(crate) persister: Arc<ResultPersister>,
    config_ref: Arc<Config>,
    result_service: Option<Arc<ResultService>>,
}

impl BenchmarkExecutor {
    /// Create a new BenchmarkExecutor.
    pub fn new(config: ExecutorConfig) -> Result<Self> {
        let config_ref = Arc::new(Config::load(&config.config_path)?);

        let docker_config = benchmark_core::docker::DockerConfig::from(&config_ref.docker);
        let docker_client = Arc::new(DockerClient::new(docker_config));
        let exercise_runner = Arc::new(ExerciseRunner::new_with_docker(config_ref.clone(), Arc::clone(&docker_client)));
        let persister = Arc::new(ResultPersister::new());

        Ok(Self {
            config,
            docker_client,
            exercise_runner,
            persister,
            config_ref,
            result_service: None,
        })
    }

    /// Execute a benchmark run for the given session.
    ///
    /// This method runs exercises for all languages in the session.
    /// Mirrors Java BenchmarkExecutor.execute().
    pub async fn execute(
        &self,
        session: &mut BenchmarkSession,
        session_manager: Option<&SessionManager>,
    ) -> Result<()> {
        // Run the body in an inner function so the finalize block always runs
        // even when execute_inner returns Err. This prevents the session from
        // staying stuck in RUNNING when an error propagates via ?.
        let result = async {
            session.start();
            // Update session manager if provided
            if let Some(sm) = session_manager {
                sm.update_session(session.clone());
            }

            let languages = session.languages.clone();
            let agent_name = session.agent_name.clone();
            let model = session.model.clone(); // Required, never None
            let thinking_level = session.thinking_level.clone();
            let exercise_name = session.exercise_name.clone();

            // Reference agent ignores model — always use "reference" to avoid
            // mixing reference results into model-specific directories.
            let model = if agent_name == "reference" {
                "reference".to_string()
            } else {
                model
            };

            info!(
                "Starting benchmark execution: agent={}, languages={:?}, model={:?}, thinking_level={:?}, exercise={:?}",
                agent_name, languages, model, thinking_level, exercise_name
            );

            // Create output consumer for live streaming to web UI
            let output_consumer = session.make_output_consumer();

            // Create agent based on agent name, wiring up message processors for streaming
            let agent: Arc<dyn Agent + Send + Sync> = match agent_name.as_str() {
                "reference" => {
                    let ref_agent = ReferenceAgent::new((*self.docker_client).clone());
                    ref_agent.set_output_consumer(output_consumer);
                    Arc::new(ref_agent)
                }
                "pi" => {
                    let mut pi_agent = PiAgent::new((*self.docker_client).clone());
                    let pi_processor = PiMessageProcessor::new(Some(output_consumer));
                    pi_agent.set_message_processor(pi_processor);
                    Arc::new(pi_agent)
                }
                _ => {
                    let mut claude_agent = ClaudeAgent::new((*self.docker_client).clone());
                    let claude_processor = ClaudeMessageProcessor::new(Some(output_consumer));
                    claude_agent.set_message_processor(claude_processor);
                    Arc::new(claude_agent)
                }
            };

            let model_str = &model;
            if let Some(ref _exercise) = exercise_name {
                self.execute_single_exercise(session, agent, &languages, model_str, thinking_level.as_deref(), &agent_name).await?;
            } else {
                self.execute_all_exercises(session, agent, &languages, model_str, thinking_level.as_deref(), &agent_name).await?;
            }

            Ok::<(), anyhow::Error>(())
        }.await;

        // Finalize: always run regardless of whether execute_inner succeeded or errored.
        session.emit_output("Benchmark execution completed\n");
        // If the execution returned an error, mark as FAILED.
        if result.is_err() {
            session.status = RunStatus::FAILED;
            let err_msg = format!("{:?}", result.as_ref().unwrap_err());
            session.set_error_message(&err_msg);
            session.emit_output(&format!("Execution error: {}\n", err_msg));
        }
        // Preserve FAILED/CANCELLED status set by inner execution paths.
        // Only transition to COMPLETED if the session is not already in a terminal failure state.
        if session.status != RunStatus::FAILED && session.status != RunStatus::CANCELLED {
            session.complete();
        } else {
            session.finished_at = Some(chrono::Utc::now());
        }
        // Update session manager with final status — ALWAYS, even on error.
        if let Some(sm) = session_manager {
            sm.update_session(session.clone());
        }

        result
    }

    /// Execute a single exercise across all selected languages.
    /// Mirrors Java BenchmarkExecutor.executeSingleExercise().
    async fn execute_single_exercise(
        &self,
        session: &mut BenchmarkSession,
        agent: Arc<dyn Agent + Send + Sync>,
        languages: &[String],
        model: &str,
        thinking_level: Option<&str>,
        agent_name: &str,
    ) -> Result<()> {
        let exercise_name = session.exercise_name.clone().unwrap_or_else(|| "unknown".to_string());

        for language in languages {
            // Check for cancellation
            if session.status == RunStatus::CANCELLED {
                session.emit_output("Benchmark cancelled\n");
                return Ok(());
            }

            session.emit_output(&format!(
                "Running exercise: {} for language: {}\n",
                exercise_name, language
            ));

            // Compute timeout override for retries: if the exercise previously
            // succeeded, cap the Docker container timeout to its previous duration
            // so a retry can't waste time spinning. If it previously failed, use
            // the default timeout.
            let timeout_override = if session.retry {
                self.compute_retry_timeout(&agent_name, language, &exercise_name, model)
            } else {
                None
            };

            match self.run_single_exercise(&agent, language, &exercise_name, model, thinking_level, agent_name, timeout_override).await {
                Ok(result) => {
                    session.emit_output(&result.output);

                    // Save result to file
                    if let Err(e) = self.save_single_result(&result, agent_name, language, model) {
                        warn!("Failed to save result: {}", e);
                    }

                    session.increment_completed();

                    if !result.success {
                        session.status = RunStatus::FAILED;
                        let error_msg = format!(
                            "Exercise failed for language {}: {}",
                            language,
                            result.error_message.as_deref().unwrap_or("Unknown error")
                        );
                        session.set_error_message(&error_msg);
                        session.emit_output(&error_msg);
                        return Ok(());
                    }
                }
                Err(e) => {
                    session.status = RunStatus::FAILED;
                    let error_msg = format!("Exercise execution error for {}/{}: {}", exercise_name, language, e);
                    session.set_error_message(&error_msg);
                    session.emit_output(&error_msg);
                    error!("{}", error_msg);
                    return Err(e);
                }
            }
        }

        session.status = RunStatus::COMPLETED;
        session.emit_output("All exercises completed successfully!\n");

        Ok(())
    }

    /// Execute all exercises for all selected languages.
    /// Mirrors Java BenchmarkExecutor.executeAllExercises().
    async fn execute_all_exercises(
        &self,
        session: &mut BenchmarkSession,
        agent: Arc<dyn Agent + Send + Sync>,
        languages: &[String],
        model: &str,
        thinking_level: Option<&str>,
        agent_name: &str,
    ) -> Result<()> {
        let mut total_exercises: i32 = 0;
        let _successful_exercises: i32 = 0;

        for language in languages {
            // Check for cancellation
            if session.status == RunStatus::CANCELLED {
                session.emit_output("Benchmark cancelled\n");
                return Ok(());
            }

            session.emit_output(&format!(
                "Running all exercises for language: {}\n",
                language
            ));

            let results_dir = self.results_dir();
            let results = self.exercise_runner
                .run_all_exercises(Arc::clone(&agent), language, agent_name, model.to_string(), thinking_level.map(|s| s.to_string()), results_dir, false)
                .await;

            total_exercises += results.len() as i32;
            let language_successful = results.iter().filter(|r| r.success).count() as i32;
            let _successful_exercise_count = language_successful;

            let failed = results.len() as i32 - language_successful;
            if failed > 0 {
                session.status = RunStatus::FAILED;
                session.emit_output(&format!(
                    "{} exercises failed for language {} out of {}",
                    failed, language, results.len()
                ));
            } else {
                session.emit_output(&format!(
                    "All exercises completed successfully for language: {}\n",
                    language
                ));
            }

            // Save individual results
            for result in &results {
                let _ = self.save_single_result(result, agent_name, language, model);
            }
        }

        session.set_total_exercises(total_exercises);

        if session.status != RunStatus::FAILED {
            session.status = RunStatus::COMPLETED;
            session.emit_output("All exercises in all languages completed successfully!\n");
        } else {
            session.set_error_message("Some exercises failed");
        }

        Ok(())
    }

    /// Run a single exercise and return the result.
    async fn run_single_exercise(
        &self,
        agent: &Arc<dyn Agent + Send + Sync>,
        language: &str,
        exercise_name: &str,
        model: &str,
        thinking_level: Option<&str>,
        _agent_name: &str,
        timeout_override_secs: Option<u64>,
    ) -> Result<AgentResult> {
        let results_dir = self.results_dir();
        self.exercise_runner
            .run_exercise_with_timeout(Arc::clone(agent), language, exercise_name, model, thinking_level.map(|s| s.to_string()), &results_dir, timeout_override_secs)
            .await
            .map_err(|e| anyhow::anyhow!("Exercise failed: {}", e))
    }

    /// Computes the Docker container timeout override for a retry.
    ///
    /// If the exercise has a previous successful result, returns
    /// `Some(previous_duration_in_seconds)` so the retry time limit is
    /// exactly the previous execution time. If the exercise previously
    /// failed (or has no result), returns `None` so the default timeout is used.
    fn compute_retry_timeout(
        &self,
        agent_name: &str,
        language: &str,
        exercise_name: &str,
        model: &str,
    ) -> Option<u64> {
        let results_dir = self.results_dir();
        let subdir = format!("{}-{}", agent_name, model);
        let result_file = results_dir.join(&subdir).join(format!(
            "result_{}_{}_{}.json",
            agent_name, language, exercise_name
        ));

        if !result_file.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&result_file).ok()?;
        let value: serde_json::Value = serde_json::from_str(&content).ok()?;
        let success = value.get("success")?.as_bool().unwrap_or(false);

        if !success {
            return None; // previously failed — use default timeout
        }

        let duration_ms = value.get("duration").and_then(|v| {
            if let Some(f) = v.as_f64() {
                Some((f * 1000.0) as u64)
            } else {
                v.as_u64()
            }
        })?;

        // Convert to seconds, minimum 1s
        let timeout_secs = (duration_ms / 1000).max(1);
        tracing::info!(
            "Retry timeout override for {}/{}: {}s (previous duration: {}ms)",
            language, exercise_name, timeout_secs, duration_ms
        );
        Some(timeout_secs)
    }

    /// Save a single exercise result to disk and update the in-memory cache.
    fn save_single_result(
        &self,
        result: &AgentResult,
        agent_name: &str,
        language: &str,
        model: &str,
    ) -> Result<()> {
        let results_dir = self
            .config
            .results_dir_override
            .clone()
            .unwrap_or_else(|| self.config_ref.output.results_dir.clone());
        let saved_path = self.persister
            .save_result(result, agent_name, model, &results_dir)
            .with_context(|| format!("Failed to save result for {}/{}", language, result.exercise_name))?;

        // Update in-memory cache with the newly saved result (replaces any existing entry
        // for the same exercise, avoiding a full cache reload).
        if let Some(ref rs) = self.result_service {
            rs.update_single_result(&saved_path);
        }

        Ok(())
    }

    /// Get the results directory, using RESULTS_DIR override if set.
    pub fn results_dir(&self) -> std::path::PathBuf {
        self.config
            .results_dir_override
            .clone()
            .unwrap_or_else(|| self.config_ref.output.results_dir.clone())
    }

    /// Get the exercise runner (for queue processor use).
    pub fn get_exercise_runner(&self) -> ExerciseRunner {
        ExerciseRunner::new_with_docker(Arc::clone(&self.config_ref), Arc::clone(&self.docker_client))
    }

    /// Set the result service for live cache updates.
    /// When set, each saved result is automatically incorporated into the in-memory cache.
    pub fn set_result_service(&mut self, result_service: Arc<ResultService>) {
        self.result_service = Some(result_service);
    }

}
