//! Template rendering tests to catch template errors before deployment.
//! Run with: cargo test --test template_rendering

use tera::Tera;

#[test]
fn test_dashboard_template_compiles() {
    // Test that the dashboard template can be loaded and compiled without errors
    let mut tera = Tera::new("templates/**/*.tera").expect("Failed to load templates");
    
    // Try to compile the dashboard template
    let result = tera.get_template("dashboard.tera");
    assert!(result.is_ok(), "Dashboard template failed to load: {:?}", result.err());
}

#[test]
fn test_run_template_compiles() {
    let mut tera = Tera::new("templates/**/*.tera").expect("Failed to load templates");
    
    let result = tera.get_template("run.tera");
    assert!(result.is_ok(), "Run template failed to load: {:?}", result.err());
}

#[test]
fn test_dashboard_renders_with_minimal_context() {
    use serde_json::json;
    let mut tera = Tera::new("templates/**/*.tera").expect("Failed to load templates");
    
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

    let result = tera.render("dashboard.tera", &ctx);
    assert!(result.is_ok(), "Dashboard template failed to render with minimal context: {:?}", result.err());
}

#[test]
fn test_run_template_renders_with_models() {
    let mut tera = Tera::new("templates/**/*.tera").expect("Failed to load templates");
    
    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Test");
    ctx.insert("models", &vec!["model1".to_string(), "model2".to_string()]);

    let result = tera.render("run.tera", &ctx);
    assert!(result.is_ok(), "Run template failed to render: {:?}", result.err());
    
    let html = result.unwrap();
    assert!(html.contains("Start New Benchmark"));
}
