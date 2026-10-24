//! Application configuration for benchmark-web, loaded from config.yaml and env vars.

use benchmark_types::config::Config;
use std::path::PathBuf;

/// Per-invocation overrides, from the `llm-benchmark web` flags.
///
/// These used to travel as four `std::env::set_var` calls in the launcher, which the
/// config loader then read back out again: invisible coupling between two crates, and
/// environment mutation in a process that is about to go multithreaded — the reason
/// edition 2024 made `set_var` `unsafe`.
#[derive(Debug, Clone, Default)]
pub struct WebOverrides {
    /// `--config`; `None` keeps the `CONFIG_PATH`/`config.yaml` resolution.
    pub config_path: Option<String>,
    /// `--port`.
    pub port: Option<u16>,
    /// `--results-dir`.
    pub results_dir: Option<PathBuf>,
    /// `--parallelism`.
    pub parallelism: Option<usize>,
}

/// Runtime configuration assembled from explicit overrides, environment variables and
/// `config.yaml`.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// The file the config was read from, so every consumer resolves against the same
    /// path the port and parallelism came from.
    pub config_path: String,
    pub server_port: u16,
    pub parallelism: usize,
    pub results_dir: PathBuf,
    /// `Some` when the results directory came from a flag or `RESULTS_DIR` rather than
    /// from `config.yaml`. The executor distinguishes the two: an explicit directory wins
    /// over the per-run subdirectory it would otherwise derive.
    pub results_dir_override: Option<PathBuf>,
}

impl AppConfig {
    /// Load configuration with explicit overrides applied on top of the environment.
    ///
    /// Precedence is explicit flag, then environment variable, then `config.yaml`, then
    /// the built-in default. The environment stays a supported layer on purpose: the
    /// runner image and `benchmark-web/tests/html_equality.rs` drive the server through
    /// `SERVER_PORT`/`RESULTS_DIR` rather than through any flag.
    pub fn load_with(overrides: &WebOverrides) -> Self {
        let config_path = overrides
            .config_path
            .clone()
            .or_else(|| std::env::var("CONFIG_PATH").ok())
            .unwrap_or_else(|| "config.yaml".to_string());

        // Keep `None` for "no usable config file" so the defaults below stay in force: a
        // missing file is expected (defaults are fine), while a file that exists but
        // cannot be parsed is reported instead of passing silently.
        let config = match Config::load_or_default(&config_path) {
            Ok((config, from_file)) => from_file.then_some(config),
            Err(e) => {
                tracing::warn!("Could not parse config {}: {}", config_path, e);
                None
            }
        };

        // Port: flag, then SERVER_PORT, then config.yaml, then 8081.
        let server_port = overrides
            .port
            .or_else(|| {
                std::env::var("SERVER_PORT")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .or_else(|| config.as_ref().map(|c| c.server.port))
            .unwrap_or(8081);

        // Parallelism: flag, then PARALLELISM, then config.yaml, then 1.
        let parallelism = overrides
            .parallelism
            .or_else(|| {
                std::env::var("PARALLELISM")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .or_else(|| config.as_ref().map(|c| c.parallelism as usize))
            .unwrap_or(1);

        if let Some(c) = &config {
            tracing::info!(
                "Loaded config from {}: parallelism={}, server_port={}",
                config_path,
                c.parallelism,
                c.server.port,
            );
        } else {
            tracing::warn!("Could not load config from {}: using defaults", config_path);
        }

        // Results directory: flag, then RESULTS_DIR, then config.yaml, then ./results.
        let results_dir_override = overrides
            .results_dir
            .clone()
            .or_else(|| std::env::var("RESULTS_DIR").ok().map(PathBuf::from));
        let results_dir = results_dir_override
            .clone()
            .or_else(|| config.as_ref().map(|c| c.output.results_dir.clone()))
            .unwrap_or_else(|| PathBuf::from("./results"));

        tracing::info!(
            "Configuration: results_dir={}, parallelism={}, port={}",
            results_dir.display(),
            parallelism,
            server_port
        );

        Self {
            config_path,
            server_port,
            parallelism,
            results_dir,
            results_dir_override,
        }
    }
}
