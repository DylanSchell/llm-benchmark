//! Command-line benchmark runner.
//!
//! Direct port of Java's `CliEntryPoint` + `BenchmarkRunner`.
//!
//! Usage:
//!   # Run all exercises for a single language with the reference agent
//!   cargo run --release -- --language java
//!
//!   # Run a single exercise
//!   cargo run --release -- --language rust --exercise two-fer
//!
//!   # Run with Claude agent and model override
//!   cargo run --release -- --agent claude --model sonnet --language python
//!
//!   # Verbose mode (shows live output)
//!   cargo run --release -- --language java --verbose

mod runner;

use anyhow::Context;
use benchmark_types::config::Config;
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// CLI entry point for the benchmark runner.
#[derive(Parser, Debug)]
#[command(name = "benchmark-cli", version, about = "Run autonomous coding agent benchmarks")]
struct Cli {
    /// Path to config file (default: config.yaml)
    #[arg(long, default_value = "config.yaml")]
    config: String,

    /// Model name override
    #[arg(long)]
    model: Option<String>,

    /// Results directory override
    #[arg(long)]
    results_dir: Option<PathBuf>,

    /// Comma-separated list of languages (default: java)
    #[arg(long, default_value = "java")]
    language: String,

    /// Specific exercise name to run (runs only that exercise)
    #[arg(long)]
    exercise: Option<String>,

    /// Agent to use: reference, claude, or pi (default: reference)
    #[arg(long, default_value = "reference")]
    agent: String,

    /// Show verbose output (live token stream)
    #[arg(long)]
    verbose: bool,

    /// Re-run exercises even if results already exist (increments attempts count)
    #[arg(long)]
    retry: bool,
}

fn main() -> anyhow::Result<()> {
    // Initialize tracing based on verbosity
    let env_filter = if std::env::var("RUST_LOG").is_err() {
        if Cli::try_parse().map(|c| c.verbose).unwrap_or(false) {
            EnvFilter::new("benchmark_cli=debug,benchmark_core=debug,info")
        } else {
            EnvFilter::new("benchmark_cli=info,benchmark_core=warn,info")
        }
    } else {
        EnvFilter::from_default_env()
    };

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .init();

    let cli = Cli::parse();
    run_benchmark(&cli)?;
    Ok(())
}

/// Run the benchmark based on CLI arguments.
fn run_benchmark(cli: &Cli) -> anyhow::Result<()> {
    // Load config
    info!("Loading config from: {}", cli.config);
    let mut config = Config::load(&cli.config).with_context(|| {
        format!("Failed to load config file: {}", cli.config)
    })?;

    // Apply command-line overrides
    if let Some(model) = &cli.model {
        info!("Overriding model from config with: {}", model);
        config.model = Some(model.clone());
    }
    if let Some(results_dir) = &cli.results_dir {
        info!("Overriding results_dir from config with: {:?}", results_dir);
        config.output.results_dir = results_dir.clone();
    }

    let model = config.model.clone().unwrap_or_else(|| "default".to_string());

    // Validate config
    if let Err(e) = config.validate() {
        error!("Configuration error: {}", e);
        std::process::exit(1);
    }

    // Create the runner and execute
    let result = tokio::runtime::Runtime::new()?;
    let retry = cli.retry;
    result.block_on(runner::run(cli, &config, &model, retry))
}
