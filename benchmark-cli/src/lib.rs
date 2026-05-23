//! Command-line benchmark runner library.
//!
//! This crate provides the benchmark execution logic that can be called from the launcher.

pub mod runner;

use anyhow::Context;
use benchmark_types::config::Config;
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// CLI arguments for running benchmarks.
#[derive(Parser, Debug, Clone)]
#[command(name = "run", version, about = "Run autonomous coding agent benchmarks")]
pub struct RunArgs {
    /// Path to config file (default: config.yaml)
    #[arg(long, default_value = "config.yaml")]
    pub config: String,

    /// Model name override
    #[arg(long)]
    pub model: Option<String>,

    /// Results directory override
    #[arg(long)]
    pub results_dir: Option<PathBuf>,

    /// Comma-separated list of languages (default: java)
    #[arg(long, default_value = "java")]
    pub language: String,

    /// Specific exercise name to run (runs only that exercise)
    #[arg(long)]
    pub exercise: Option<String>,

    /// Agent to use: reference, claude, or pi (default: reference)
    #[arg(long, default_value = "reference")]
    pub agent: String,

    /// Show verbose output (live token stream)
    #[arg(long)]
    pub verbose: bool,

    /// Re-run exercises even if results already exist (increments attempts count)
    #[arg(long)]
    pub retry: bool,
}

/// Run benchmarks with the given arguments.
pub fn run_benchmark(args: RunArgs) -> anyhow::Result<()> {
    // Initialize tracing based on verbosity
    let env_filter = if std::env::var("RUST_LOG").is_err() {
        if args.verbose {
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

    execute(&args)
}

/// Execute benchmarks with the given arguments.
pub fn execute(args: &RunArgs) -> anyhow::Result<()> {
    info!("Loading config from: {}", args.config);
    let mut config = Config::load(&args.config).with_context(|| {
        format!("Failed to load config file: {}", args.config)
    })?;

    // Apply command-line overrides
    if let Some(model) = &args.model {
        info!("Overriding model from config with: {}", model);
        config.model = Some(model.clone());
    }
    if let Some(results_dir) = &args.results_dir {
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
    let retry = args.retry;
    result.block_on(runner::run(args, &config, &model, retry))
}
