//! Template rendering tests to catch template errors before deployment.
//!
//! The templates come from the binary, not from the working directory, so this exercises
//! exactly what the server ships and passes from anywhere. Run with:
//! cargo test --test template_rendering

use benchmark_web::routes::TemplateEngine;

/// The production template set: embedded, with the server's own filters registered.
fn engine() -> TemplateEngine {
    TemplateEngine::from_embedded().expect("the embedded templates must load")
}

#[test]
fn test_dashboard_template_compiles() {
    let engine = engine();

    // Try to compile the dashboard template
    let result = engine.tera.get_template("dashboard.tera");
    assert!(result.is_ok(), "Dashboard template failed to load: {:?}", result.err());
}

#[test]
fn test_run_template_compiles() {
    let engine = engine();

    let result = engine.tera.get_template("run.tera");
    assert!(result.is_ok(), "Run template failed to load: {:?}", result.err());
}

#[test]
fn test_dashboard_renders_with_minimal_context() {
    use serde_json::json;
    let engine = engine();

    // Create minimal context that should render without errors
    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Test");
    ctx.insert("stats", &json!({
        "total_runs": 0,
        "total_exercises": 0,
        "successful_exercises": 0,
        "success_rate": 0.0,
        "language_stats": [],
        "agent_stats": [],
        "model_stats": []
    }));
    ctx.insert("active_runs", &0);
    ctx.insert("active_sessions", &Vec::<serde_json::Value>::new());
    ctx.insert("queue_items", &Vec::<serde_json::Value>::new());
    ctx.insert("running_count", &0);
    ctx.insert("pending_count", &0);
    ctx.insert("completed_count", &0);
    ctx.insert("failed_count", &0);
    ctx.insert("cancelled_count", &0);
    ctx.insert("running_width", &"0.0");
    ctx.insert("pending_width", &"0.0");
    ctx.insert("completed_width", &"0.0");
    ctx.insert("failed_width", &"0.0");
    ctx.insert("cancelled_width", &"0.0");
    ctx.insert("quick_bench", &false);

    let result = engine.tera.render("dashboard.tera", &ctx);
    assert!(result.is_ok(), "Dashboard template failed to render with minimal context: {:?}", result.err());
}

#[test]
fn test_run_template_renders_with_models() {
    let engine = engine();

    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Test");
    ctx.insert("models", &vec!["model1".to_string(), "model2".to_string()]);

    let result = engine.tera.render("run.tera", &ctx);
    assert!(result.is_ok(), "Run template failed to render: {:?}", result.err());

    let html = result.unwrap();
    assert!(html.contains("Start New Benchmark"));
}


#[test]
fn test_scoring_template_compiles() {
    let engine = engine();
    let result = engine.tera.get_template("scoring.tera");
    assert!(result.is_ok(), "Scoring template failed to load: {:?}", result.err());
}

#[test]
fn test_scoring_renders_with_minimal_context() {
    use serde_json::json;
    // No filter is registered here on purpose: the engine under test supplies the same
    // `format_number` the server uses, so this renders through the real filter.
    let engine = engine();
    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Scoring");
    ctx.insert("results", &json!([{
        "agent": "pi", "model": "m", "language": "go", "exercise": "x",
        "success": true, "success_rate": 1.0, "speed_score": 0.5, "token_score": 0.5,
        "composite_score": 0.8, "duration": "1s", "output_tokens": 100,
        "input_chars": 200, "output_chars": 300, "detail_url": "/results/..."
    }]));
    ctx.insert("model_scores", &json!([{
        "name": "pi - m", "avg_composite_score": 0.8, "avg_success_rate": 1.0,
        "avg_speed_score": 0.5, "avg_token_score": 0.5, "total_tokens": 100,
        "total_chars": 300, "total_runs": 1
    }]));
    ctx.insert("total_results", &1);
    ctx.insert("filter_language", &None::<String>);
    ctx.insert("filter_agent", &None::<String>);
    ctx.insert("filter_quick", &false);
    let rendered = engine.tera.render("scoring.tera", &ctx);
    assert!(rendered.is_ok(), "Scoring template failed to render: {:?}", rendered.err());
}
