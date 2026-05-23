//! Benchmark report generator library.

use clap::Parser;
use std::path::PathBuf;

/// Arguments for generating a report.
#[derive(Parser, Debug, Clone)]
#[command(name = "report", about = "Generate a full markdown report from results")]
pub struct ReportArgs {
    /// Results directory to analyze (default: ../benchmark-results)
    #[arg(long, default_value = "../benchmark-results")]
    pub results_dir: PathBuf,

    /// Output file path (default: results.md)
    #[arg(long, default_value = "results.md")]
    pub output: String,
}

/// Generate a Markdown report from benchmark results.
pub fn generate_report(args: &ReportArgs) -> anyhow::Result<()> {
    internal::run_report(&args.results_dir, &args.output)
}

mod internal;
