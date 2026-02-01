use crate::agent::AgentResult;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Serialize)]
pub struct ResultSummary {
    pub timestamp: String,
    pub agent: String,
    pub language: String,
    pub total_exercises: usize,
    pub successful: usize,
    pub failed: usize,
    pub success_rate: String,
    pub results: Vec<AgentResult>,
}

pub struct ResultSaver;

impl ResultSaver {
    pub fn new() -> Self {
        Self
    }

    pub fn save_result(
        &self,
        result: &AgentResult,
        agent_name: &str,
        results_dir: &Path,
    ) -> Result<PathBuf, std::io::Error> {
        fs::create_dir_all(results_dir)?;

        let filename = format!(
            "result_{}_{}_{}.json",
            agent_name, result.language, result.exercise_name
        );
        let result_file = results_dir.join(&filename);

        let json = serde_json::to_string_pretty(result)?;
        fs::write(&result_file, json)?;

        info!("Result saved to: {:?}", result_file);

        if let Some(ref trace) = result.trace {
            if !trace.is_empty() {
                let trace_filename = format!(
                    "trace_{}_{}_{}.html",
                    agent_name, result.language, result.exercise_name
                );
                let trace_file = results_dir.join(&trace_filename);
                fs::write(&trace_file, trace)?;
            }
        }

        Ok(result_file)
    }

    pub fn save_results(
        &self,
        results: &[AgentResult],
        agent_name: &str,
        language: &str,
        results_dir: &Path,
    ) -> Result<PathBuf, std::io::Error> {
        fs::create_dir_all(results_dir)?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("results_{}_{}_{}.json", agent_name, language, timestamp);
        let result_file = results_dir.join(&filename);

        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.len() - successful;
        let success_rate = if results.is_empty() {
            "0.0%".to_string()
        } else {
            format!("{:.1}%", (successful * 100) as f64 / results.len() as f64)
        };

        let summary = ResultSummary {
            timestamp: chrono::Utc::now().to_rfc3339(),
            agent: agent_name.to_string(),
            language: language.to_string(),
            total_exercises: results.len(),
            successful,
            failed,
            success_rate,
            results: results.to_vec(),
        };

        let json = serde_json::to_string_pretty(&summary)?;
        fs::write(&result_file, json)?;

        info!("Results saved to: {:?}", result_file);

        for result in results {
            if let Some(ref trace) = result.trace {
                if !trace.is_empty() {
                    let trace_filename = format!(
                        "trace_{}_{}_{}.html",
                        agent_name, result.language, result.exercise_name
                    );
                    let trace_file = results_dir.join(&trace_filename);
                    fs::write(&trace_file, trace)?;
                }
            }
        }

        Ok(result_file)
    }

    pub fn print_summary(&self, results: &[AgentResult]) {
        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.len() - successful;
        let success_rate = if results.is_empty() {
            0.0
        } else {
            (successful * 100) as f64 / results.len() as f64
        };

        println!("\n=== Benchmark Summary ===");
        println!("Exercises run: {}", results.len());
        println!("Tests passed: {} ({:.1}%)", successful, success_rate);
        println!("Tests failed: {}", failed);

        if failed > 0 {
            println!("\nFailed exercises:");
            for result in results.iter().filter(|r| !r.success) {
                println!("  - {}", result.exercise_name);
                if !result.output.is_empty() {
                    println!("    Output:\n{}", self.indent_output(&result.output, 6));
                }
            }
        }
    }

    fn indent_output(&self, output: &str, indent: usize) -> String {
        let indent_str = " ".repeat(indent);
        output
            .lines()
            .map(|line| format!("{}{}", indent_str, line))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for ResultSaver {
    fn default() -> Self {
        Self::new()
    }
}
