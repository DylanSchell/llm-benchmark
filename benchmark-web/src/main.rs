//! benchmark-web - Rust port of the Java benchmark web application.
//! Axum-based web server with REST API and SSE streaming.

mod models;
mod routes;
mod services;

use services::{
    BenchmarkExecutor, BenchmarkService, QueueProcessor, QueueConfig, ResultService, SessionManager,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::services::ServeDir;

use std::path::PathBuf;
use routes::{AppState, TemplateEngine};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "benchmark_web=debug,benchmark_core=debug,tower_http=debug,axum=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // =============================================================================
    // Configuration
    // =============================================================================

    // Load config.yaml first so we can use its values as defaults
    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.yaml".to_string());
    let config = benchmark_types::config::Config::load(&config_path).ok();

    // SERVER_PORT env var overrides config.yaml; falls back to config value or 8081
    let server_port = std::env::var("SERVER_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .or(config.as_ref().map(|c| c.server.port as u16))
        .unwrap_or(8081);

    let config_parallelism = config.as_ref().map(|c| c.parallelism as usize);

    // PARALLELISM env var overrides config.yaml; falls back to config value or 1
    let parallelism = std::env::var("PARALLELISM")
        .ok()
        .and_then(|v| v.parse().ok())
        .or(config_parallelism)
        .unwrap_or(1);

    if let Some(c) = &config {
        tracing::info!("Loaded config from {}: parallelism={}, server_port={}, benchmark_path={}", config_path, c.parallelism, c.server.port, c.benchmark_path.display());
    } else {
        tracing::warn!("Could not load config from {}: using defaults", config_path);
    }

    // RESULTS_DIR env var overrides config.yaml; falls back to config value or ./results
    let results_dir = std::env::var("RESULTS_DIR")
        .ok()
        .or(config.as_ref().map(|c| c.output.results_dir.to_string_lossy().to_string()))
        .unwrap_or_else(|| "./results".to_string());
    let results_path = PathBuf::from(&results_dir);

    tracing::info!("Configuration: results_dir={}, parallelism={}, port={}", results_dir, parallelism, server_port);

    // =============================================================================
    // Initialize Services
    // =============================================================================

    // SessionManager manages benchmark session lifecycle
    let session_manager = SessionManager::new();

    // ResultService loads and caches result files
    let result_service = ResultService::new(results_path.clone());

    // BenchmarkExecutor handles actual execution
    let executor_config = services::benchmark_executor::ExecutorConfig {
        config_path,
        results_dir_override: if std::env::var("RESULTS_DIR").is_ok() {
            Some(results_path.clone())
        } else {
            None
        },
    };
    let mut benchmark_executor = BenchmarkExecutor::new(executor_config).expect("Failed to create BenchmarkExecutor");
    // Wire up result service so saved results are immediately visible in the in-memory cache
    benchmark_executor.set_result_service(std::sync::Arc::new(result_service.clone()));
    let benchmark_executor = std::sync::Arc::new(benchmark_executor);

    // QueueProcessor manages the benchmark queue
    let queue_config = QueueConfig {
        parallelism,
        ..QueueConfig::default()
    };
    let exercise_runner = benchmark_executor.get_exercise_runner();
    let queue_processor = QueueProcessor::new(
        session_manager.clone(),
        result_service.clone(),
        benchmark_executor.clone(),
        exercise_runner,
        queue_config,
    );
    let _benchmark_executor_for_service = benchmark_executor.clone();

    // Create a separate exercise runner for BenchmarkService (needed for dynamic discovery)
    let exercise_runner_for_service = benchmark_executor.get_exercise_runner();

    // BenchmarkService is the facade
    let benchmark_service = BenchmarkService::new(
        session_manager,
        queue_processor,
        result_service,
        exercise_runner_for_service,
    );

    // Start the queue worker
    benchmark_service.get_queue_processor().start_queue_worker().await;

    // =============================================================================
    // Build Router
    // =============================================================================

    let state = AppState {
        service: benchmark_service.clone(),
    };

    // Initialize template engine
    let templates = TemplateEngine::new();

    let app = routes::build_router(state, templates);

    // =============================================================================
    // Start Server
    // =============================================================================

    let addr = format!("0.0.0.0:{}", server_port);
    tracing::info!("Starting benchmark-web server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");

    // Add static file serving — resolve relative to the binary's crate root
    let static_dir = std::env::var("STATIC_DIR")
        .unwrap_or_else(|_| {
            // Try cargo-managed paths first, then fall back to workspace-relative
            if std::path::Path::new("static/css/style.css").exists() {
                "static".to_string()
            } else if std::path::Path::new("benchmark-web/static/css/style.css").exists() {
                "benchmark-web/static".to_string()
            } else {
                // Fallback: use the directory containing the binary
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|_| "../benchmark-web/static".to_string()))
                    .unwrap_or_else(|| "benchmark-web/static".to_string())
            }
        });
    let app = app.fallback_service(ServeDir::new(&static_dir));

    // Start the server using axum's serve
    let service_clone = benchmark_service.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(service_clone))
        .await
        .expect("Server failed");

    tracing::info!("Server shut down gracefully");
}

/// Handle graceful shutdown - kill containers, abort running tasks, and exit.
async fn shutdown_signal(service: BenchmarkService) {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");

    tracing::info!("Shutdown signal received, killing Docker containers...");
    service.cleanup_containers().await;

    // Drop the queue processor first — this aborts the queue worker task and
    // prevents any new sessions from being created.
    tracing::info!("Stopping queue processor...");
    service.shutdown().await;

    // Drop all session broadcast senders so SSE receivers close immediately.
    // Without this, axum's graceful shutdown waits forever for SSE clients
    // whose recv() is blocked on a channel with no sender to signal closure.
    tracing::info!("Dropping all session broadcasters...");
    let sessions = service.get_all_sessions();
    for (_id, session) in sessions {
        // Cloning the session drops its msg_tx, closing the broadcast channel
        // for any receivers still waiting on recv().
        drop(session);
    }

    tracing::info!("Cleanup complete.");
}
