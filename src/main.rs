//! Unified launcher for all benchmark commands.
//!
//! Usage:
//!   llm-benchmark run         - Run benchmarks (benchmark-cli)
//!   llm-benchmark web         - Start web server (benchmark-web)  
//!   llm-benchmark report      - Generate markdown report (benchmark-reporter)
//!   llm-benchmark token-report - Generate token statistics (benchmark-token-report)

mod web;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Unified benchmark launcher - run, web, report, and token-report in one command.
#[derive(Parser, Debug)]
#[command(name = "llm-benchmark", version, about = "LLM Benchmark Launcher")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run benchmarks against an agent (reference, claude, or pi)
    Run(benchmark_cli::RunArgs),

    /// Start the web dashboard server
    Web(WebArgs),

    /// Generate a full markdown report from results
    Report(benchmark_reporter::ReportArgs),

    /// Generate token statistics report
    TokenReport(benchmark_token_report::TokenReportArgs),
}

#[derive(Parser, Debug, Clone)]
#[command(name = "web")]
pub struct WebArgs {
    /// Path to config file (default: config.yaml)
    #[arg(long, default_value = "config.yaml")]
    pub config: String,
    /// Port to run the server on (default: 8081)
    #[arg(long)]
    pub port: Option<u16>,
    /// Results directory override
    #[arg(long)]
    pub results_dir: Option<PathBuf>,
    /// Parallelism level
    #[arg(long)]
    pub parallelism: Option<usize>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(args) => benchmark_cli::run_benchmark(args),
        Commands::Web(args) => {
            tokio::runtime::Runtime::new()?.block_on(web::execute(args))
        }
        Commands::Report(args) => benchmark_reporter::generate_report(&args),
        Commands::TokenReport(args) => benchmark_token_report::generate_token_report(&args),
    }
}
