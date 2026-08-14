//! benchmark-web - Rust port of the Java benchmark web application.
//! Axum-based web server with REST API and SSE streaming.

mod config;
mod metrics;
mod models;
pub mod routes;
pub mod services;

use config::AppConfig;
use services::{
    BenchmarkExecutor, BenchmarkService, QueueProcessor, QueueConfig, ResultService, SessionManager,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::services::ServeDir;

use routes::TemplateEngine;

/// Run the web server with the given configuration.
/// This function reads configuration from environment variables:
/// - CONFIG_PATH: Path to config.yaml (default: "config.yaml")
/// - SERVER_PORT: Port to listen on (default: 8081)
/// - RESULTS_DIR: Results directory (default: "./results")
/// - PARALLELISM: Parallelism level (default: 1)
pub async fn run_web_server() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "benchmark_web=debug,benchmark_core=debug,tower_http=debug,axum=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize reasoning registry with built-in defaults
    benchmark_types::reasoning::ReasoningRegistry::register_defaults();

    // =============================================================================
    // Configuration
    // =============================================================================

    let app_config = AppConfig::load();

    // =============================================================================
    // Initialize Services
    // =============================================================================

    // SessionManager manages benchmark session lifecycle
    let session_manager = SessionManager::new();

    // ResultService loads and caches result files
    let result_service = ResultService::new(app_config.results_dir.clone());

    // BenchmarkExecutor handles actual execution
    let config_path_str = std::env::var("CONFIG_PATH")
        .ok()
        .or(app_config.config.as_ref().map(|c| c.benchmark_path.to_string_lossy().to_string()))
        .unwrap_or_else(|| "config.yaml".to_string());
    let executor_config = services::benchmark_executor::ExecutorConfig {
        config_path: config_path_str,
        results_dir_override: if std::env::var("RESULTS_DIR").is_ok() {
            Some(app_config.results_dir.clone())
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
        parallelism: app_config.parallelism,
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

    // Create a shared shutdown flag — SSE streams check this to exit promptly
    // during shutdown, even if broadcast channel senders are still alive.
    let shutdown_flag = Arc::new(AtomicBool::new(false));

    // Initialize template engine
    let templates = TemplateEngine::new();

    let state = routes::AppState {
        service: benchmark_service.clone(),
        shutdown_flag: shutdown_flag.clone(),
        metrics: crate::metrics::Metrics::new(),
    };

    let app = routes::build_router(state, templates);

    // =============================================================================
    // Start Server
    // =============================================================================

    let addr = format!("0.0.0.0:{}", app_config.server_port);
    tracing::info!("Starting benchmark-web server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");

    // Add static file serving — resolve relative to the crate root
    let static_dir = std::env::var("STATIC_DIR")
        .unwrap_or_else(|_| {
            // Try multiple paths: env var, current dir, then crate-relative
            if std::path::Path::new("static/css/style.css").exists() {
                "static".to_string()
            } else if std::path::Path::new("benchmark-web/static/css/style.css").exists() {
                "benchmark-web/static".to_string()
            } else if std::path::Path::new("../benchmark-web/static/css/style.css").exists() {
                "../benchmark-web/static".to_string()
            } else {
                "benchmark-web/static".to_string()
            }
        });
    let app = app.fallback_service(ServeDir::new(&static_dir));

    // Start the server using axum's serve with a timeout so we don't hang forever
    let service_clone = benchmark_service.clone();
    let shutdown_flag_clone = shutdown_flag.clone();
    let handle = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(service_clone, shutdown_flag_clone))
        .await;

    // Signal to any stuck SSE streams that they should exit
    shutdown_flag.store(true, Ordering::SeqCst);

    if let Err(e) = handle {
        tracing::error!("Server failed: {}", e);
    }

    tracing::info!("Server shut down gracefully");
    
    Ok(())
}

/// Handle graceful shutdown - kill containers, abort running tasks, and exit.
async fn shutdown_signal(service: BenchmarkService, shutdown_flag: Arc<AtomicBool>) {
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
    tracing::info!("Dropping all session broadcasters...");
    let sessions = service.get_all_sessions();
    for (_id, session) in sessions {
        drop(session);
    }

    // Signal to SSE streams that shutdown is happening so they exit even if
    // broadcast channel senders are still alive (held by SSE handler clones).
    shutdown_flag.store(true, Ordering::SeqCst);

    tracing::info!("Cleanup complete.");
}
