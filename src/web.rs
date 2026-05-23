//! Web command - delegates to the benchmark-web crate.

use crate::WebArgs;

pub async fn execute(args: WebArgs) -> anyhow::Result<()> {
    // Set environment variables that benchmark-web reads
    if let Some(port) = args.port {
        std::env::set_var("SERVER_PORT", port.to_string());
    }
    
    if let Some(dir) = args.results_dir {
        std::env::set_var("RESULTS_DIR", dir.to_string_lossy().to_string());
    }
    
    if let Some(parallelism) = args.parallelism {
        std::env::set_var("PARALLELISM", parallelism.to_string());
    }

    // Load config if provided
    if !args.config.is_empty() {
        std::env::set_var("CONFIG_PATH", &args.config);
    }

    // Delegate to benchmark-web crate
    benchmark_web::run_web_server().await
}
