//! Routes for benchmark-web.
//! All REST API endpoints matching the Java API exactly.

pub mod benchmark;
pub mod compare;
pub mod exercise;
pub mod queue;
pub mod result;
pub mod results;
pub mod scoring;

use crate::assets::Templates;
use axum::{Router, Extension};
use axum::routing::get;
use benchmark::register as register_benchmark;
use compare::register as register_compare;
use exercise::register as register_exercise;
use queue::register as register_queue;
use result::register as register_result;
use results::register as register_results;
use scoring::register as register_scoring;
use std::sync::Arc;
use tera::Tera;

/// Application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    pub service: crate::services::BenchmarkService,
    pub shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    pub metrics: crate::metrics::Metrics,
}



/// Shared Tera template engine.
#[derive(Clone)]
pub struct TemplateEngine {
    pub tera: Arc<Tera>,
}

impl TemplateEngine {
    /// Build the shared template engine.
    ///
    /// Templates are compiled into the binary. Setting `TEMPLATES_DIR` loads them from a
    /// directory instead, which is what makes template edits visible without a rebuild.
    pub fn new() -> Result<Self, tera::Error> {
        match std::env::var("TEMPLATES_DIR") {
            Ok(dir) => Self::from_dir(&dir),
            Err(_) => Self::from_embedded(),
        }
    }

    /// Build from the templates embedded in the binary — the production path.
    pub fn from_embedded() -> Result<Self, tera::Error> {
        let mut sources: Vec<(String, String)> = Vec::new();
        for name in Templates::iter() {
            let file = Templates::get(name.as_ref()).expect("iter() yields only embedded entries");
            let source = std::str::from_utf8(&file.data)
                .map_err(|e| tera::Error::msg(format!("template {name} is not valid UTF-8: {e}")))?
                .to_owned();
            sources.push((name.into_owned(), source));
        }

        // Register every name before any template is compiled. `add_raw_template`
        // resolves `{% extends %}` immediately, so adding `compare.tera` before
        // `layout.tera` would fail on the inheritance; this bulk form defers it.
        let templates: Vec<(&str, &str)> = sources
            .iter()
            .map(|(name, source)| (name.as_str(), source.as_str()))
            .collect();
        let mut tera = Tera::default();
        tera.add_raw_templates(templates)?;

        Ok(Self::finish(tera))
    }

    /// Build from a directory on disk (`TEMPLATES_DIR`), for development.
    pub fn from_dir(dir: &str) -> Result<Self, tera::Error> {
        let tera = Tera::new(&format!("{dir}/**/*.tera"))?;
        Ok(Self::finish(tera))
    }

    /// Register the custom filters and wrap the engine for sharing.
    fn finish(mut tera: Tera) -> Self {
        // Add custom filter for formatting large numbers with K/M/G suffixes
        tera.register_filter("format_number", |value: &tera::Value, _args: &std::collections::HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            let num = match value {
                tera::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        i
                    } else if let Some(f) = n.as_f64() {
                        f as i64
                    } else {
                        return Ok(tera::Value::String("0".to_string()));
                    }
                }
                tera::Value::String(s) => s.parse::<i64>().unwrap_or(0),
                _ => 0,
            };
            
            let abs = num.unsigned_abs();
            let formatted = if abs >= 1_000_000_000 {
                format!("{:.1}G", num as f64 / 1_000_000_000.0)
            } else if abs >= 1_000_000 {
                format!("{:.1}M", num as f64 / 1_000_000.0)
            } else if abs >= 1_000 {
                format!("{:.1}K", num as f64 / 1_000.0)
            } else {
                num.to_string()
            };
            
            Ok(tera::Value::String(formatted))
        });
        
        Self { tera: Arc::new(tera) }
    }

    pub fn render(&self, template: &str, context: &tera::Context) -> String {
        match self.tera.render(template, context) {
            Ok(html) => html,
            Err(e) => {
                tracing::error!("Template rendering error in '{}': {}", template, e);
                "<h1>Internal Server Error</h1><p>Something went wrong. Please try again later.</p>".to_string()
            }
        }
    }
}

/// Build the complete router with all routes.
/// Uses Extension<AppState> layer to pass state to handlers.
pub fn build_router(state: AppState, templates: TemplateEngine) -> Router<()> {
    let metrics_state = state.clone();
    Router::new()
        .route("/metrics", get(move || {
            let m = metrics_state.metrics.clone();
            async move { m.render() }
        }))
        .merge(register_benchmark(Router::new()))
        .merge(register_compare(Router::new()))
        .merge(register_exercise(Router::new()))
        .merge(register_queue(Router::new()))
        .merge(register_result(Router::new()))
        .merge(register_results(Router::new()))
        .merge(register_scoring(Router::new()))
        .layer(Extension(state))
        .layer(Extension(templates))
}

/// Build the complete router with all routes, using a provided shutdown flag.
pub fn build_router_with_shutdown(
    service: crate::services::BenchmarkService,
    templates: TemplateEngine,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Router<()> {
    let state = AppState {
        service,
        shutdown_flag,
        metrics: crate::metrics::Metrics::new(),
    };
    build_router(state, templates)
}
