use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "benchmark")]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the benchmark
    Run {
        /// Path to config file
        #[arg(short, long, default_value = "config.yaml")]
        config: String,
        /// Programming language
        #[arg(short, long, default_value = "java")]
        language: String,
        /// Specific exercise to run (optional)
        #[arg(short, long)]
        exercise: Option<String>,
        /// Agent to use: reference or claude
        #[arg(short, long, default_value = "reference")]
        agent: String,
    },
    /// Analyze benchmark results
    Analyze {
        /// Path to results directory
        #[arg(short, long)]
        results_dir: Option<String>,
        /// Output file for report
        #[arg(short, long, default_value = "results.md")]
        output: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    match args.command {
        Commands::Run { config, language, exercise, agent } => {
            println!("Running benchmark with config: {}", config);
            println!("Language: {}, Agent: {}", language, agent);
            if let Some(ref ex) = exercise {
                println!("Exercise: {}", ex);
            }
            benchmark::run_benchmark(&config, &language, &agent, exercise.as_deref()).await?;
        }
        Commands::Analyze { results_dir, output } => {
            let results_dir = results_dir.as_deref().unwrap_or("../benchmark-results");
            println!("Analyzing results in: {} -> {}", results_dir, output);
            benchmark::analyze_results(results_dir, &output).await?;
        }
    }

    Ok(())
}
