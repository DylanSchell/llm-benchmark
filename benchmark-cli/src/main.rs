//! CLI benchmark runner binary.

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = benchmark_cli::RunArgs::parse();
    benchmark_cli::run_benchmark(args)
}
