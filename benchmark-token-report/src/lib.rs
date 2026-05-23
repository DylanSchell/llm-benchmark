//! Token statistics report library.

use clap::Parser;
use std::path::PathBuf;

/// Arguments for a token statistics report.
#[derive(Parser, Debug, Clone)]
#[command(name = "token-report")]
pub struct TokenReportArgs {
    /// Path to the benchmark results directory
    #[arg(short, long, default_value = "../benchmark-results")]
    pub results_dir: PathBuf,

    /// Only include results matching this agent name
    #[arg(short, long)]
    pub agent: Option<String>,

    /// Only include results matching this language
    #[arg(short, long)]
    pub language: Option<String>,

    /// Only include results matching this model
    #[arg(short, long)]
    pub model: Option<String>,

    /// Only include results matching this exercise name
    #[arg(short, long)]
    pub exercise: Option<String>,

    /// Show per-exercise details
    #[arg(short, long)]
    pub details: bool,

    /// Output as JSON instead of a human-readable table
    #[arg(short, long)]
    pub json: bool,
}

/// Generate a token statistics report.
pub fn generate_token_report(args: &TokenReportArgs) -> anyhow::Result<()> {
    internal::run_token_report(args)
}

mod internal;
