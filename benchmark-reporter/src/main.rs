//! Benchmark report generator binary.

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = benchmark_reporter::ReportArgs::parse();
    benchmark_reporter::generate_report(&args)
}
