//! Routes for benchmark-web.
//! All REST API endpoints matching the Java API exactly.

pub mod benchmark;
pub mod exercise;
pub mod queue;
pub mod result;
pub mod results;

use axum::{Router, Extension};
use benchmark::register as register_benchmark;
use exercise::register as register_exercise;
use queue::register as register_queue;
use result::register as register_result;
use results::register as register_results;
use std::sync::Arc;
use tera::Tera;

/// Application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    pub service: crate::services::BenchmarkService,
}



/// Shared Tera template engine.
#[derive(Clone)]
pub struct TemplateEngine {
    pub tera: Arc<Tera>,
}

impl TemplateEngine {
    pub fn new() -> Self {
        // Templates are in benchmark-web/templates/ relative to workspace root
        // Try multiple paths: env var, current dir, then workspace-relative
        let templates_path = std::env::var("TEMPLATES_DIR")
            .or_else(|_| {
                // Check if templates exist in current directory
                if std::path::Path::new("templates/dashboard.tera").exists() {
                    Ok("templates".to_string())
                } else if std::path::Path::new("benchmark-web/templates/dashboard.tera").exists() {
                    Ok("benchmark-web/templates".to_string())
                } else if std::path::Path::new("../benchmark-web/templates/dashboard.tera").exists() {
                    Ok("../benchmark-web/templates".to_string())
                } else {
                    Err(std::env::VarError::NotPresent)
                }
            })
            .unwrap_or_else(|_| "templates".to_string());
        let tera = match Tera::new(&format!("{}/**/*.tera", templates_path)) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Parsing error(s): {e}");
                std::process::exit(1);
            }
        };
        Self { tera: Arc::new(tera) }
    }

    pub fn render(&self, template: &str, context: &tera::Context) -> String {
        match self.tera.render(template, context) {
            Ok(html) => html,
            Err(e) => {
                eprintln!("Template rendering error: {e}");
                format!("<h1>Error rendering template: {e}</h1>")
            }
        }
    }
}

/// Build the complete router with all routes.
/// Uses Extension<AppState> layer to pass state to handlers.
pub fn build_router(state: AppState, templates: TemplateEngine) -> Router<()> {
    Router::new()
        .merge(register_benchmark(Router::new()))
        .merge(register_exercise(Router::new()))
        .merge(register_queue(Router::new()))
        .merge(register_result(Router::new()))
        .merge(register_results(Router::new()))
        .layer(Extension(state))
        .layer(Extension(templates))
}
