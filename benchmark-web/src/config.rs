//! Application configuration for benchmark-web, loaded from config.yaml and env vars.

use std::path::PathBuf;
use benchmark_types::config::Config;

/// Runtime configuration assembled from `config.yaml` and environment variables.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server_port: u16,
    pub parallelism: usize,
    pub results_dir: PathBuf,
    pub config: Option<Config>,
}

impl AppConfig {
    /// Load configuration from config.yaml with environment variable overrides.
    pub fn load() -> Self {
        let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.yaml".to_string());
        let config = Config::load(&config_path).ok();

        // SERVER_PORT env var overrides config.yaml; falls back to config value or 8081
        let server_port = std::env::var("SERVER_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(config.as_ref().map(|c| c.server.port))
            .unwrap_or(8081);

        let config_parallelism = config.as_ref().map(|c| c.parallelism as usize);

        // PARALLELISM env var overrides config.yaml; falls back to config value or 1
        let parallelism = std::env::var("PARALLELISM")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(config_parallelism)
            .unwrap_or(1);

        if let Some(c) = &config {
            tracing::info!(
                "Loaded config from {}: parallelism={}, server_port={}, benchmark_path={}",
                config_path,
                c.parallelism,
                c.server.port,
                c.benchmark_path.display()
            );
        } else {
            tracing::warn!("Could not load config from {}: using defaults", config_path);
        }

        // RESULTS_DIR env var overrides config.yaml; falls back to config value or ./results
        let results_dir = std::env::var("RESULTS_DIR")
            .ok()
            .or(config.as_ref().map(|c| c.output.results_dir.to_string_lossy().to_string()))
            .unwrap_or_else(|| "./results".to_string());
        let results_path = PathBuf::from(&results_dir);

        tracing::info!(
            "Configuration: results_dir={}, parallelism={}, port={}",
            results_dir, parallelism, server_port
        );

        Self {
            server_port,
            parallelism,
            results_dir: results_path,
            config,
        }
    }

}
