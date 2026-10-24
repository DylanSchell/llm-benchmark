//! Web command - delegates to the benchmark-web crate.

use benchmark_web::WebOverrides;

use crate::WebArgs;

pub async fn execute(args: WebArgs) -> anyhow::Result<()> {
    // Pass the flags through as data. This used to be four `std::env::set_var` calls that
    // benchmark-web's config loader then read back, which made the real dependency
    // between the two crates invisible to both the reader and the compiler.
    let overrides = WebOverrides {
        config_path: (!args.config.is_empty()).then_some(args.config),
        port: args.port,
        results_dir: args.results_dir,
        parallelism: args.parallelism,
    };

    benchmark_web::run_web_server(overrides).await
}
