//! Token statistics report binary.

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = benchmark_token_report::TokenReportArgs::parse();
    benchmark_token_report::generate_token_report(&args)
}
