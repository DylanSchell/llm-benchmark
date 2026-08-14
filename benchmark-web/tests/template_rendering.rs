//! Template rendering tests to catch template errors before deployment.
//! Run with: cargo test --test template_rendering

use tera::Tera;

#[test]
fn test_dashboard_template_compiles() {
    // Test that the dashboard template can be loaded and compiled without errors
    let tera = Tera::new("templates/**/*.tera").expect("Failed to load templates");
    
    // Try to compile the dashboard template
    let result = tera.get_template("dashboard.tera");
    assert!(result.is_ok(), "Dashboard template failed to load: {:?}", result.err());
}

#[test]
fn test_run_template_compiles() {
    let tera = Tera::new("templates/**/*.tera").expect("Failed to load templates");
    
    let result = tera.get_template("run.tera");
    assert!(result.is_ok(), "Run template failed to load: {:?}", result.err());
}

#[test]
fn test_dashboard_renders_with_minimal_context() {
    use serde_json::json;
    let tera = Tera::new("templates/**/*.tera").expect("Failed to load templates");
    
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
    let tera = Tera::new("templates/**/*.tera").expect("Failed to load templates");
    
    let mut ctx = tera::Context::new();
    ctx.insert("title", &"Test");
    ctx.insert("models", &vec!["model1".to_string(), "model2".to_string()]);

    let result = tera.render("run.tera", &ctx);
    assert!(result.is_ok(), "Run template failed to render: {:?}", result.err());
    
    let html = result.unwrap();
    assert!(html.contains("Start New Benchmark"));
}


#[test]
fn test_scoring_template_compiles() {
    let tera = Tera::new("templates/**/*.tera").expect("Failed to load templates");
    let result = tera.get_template("scoring.tera");
    assert!(result.is_ok(), "Scoring template failed to load: {:?}", result.err());
}

#[test]
fn test_scoring_renders_with_minimal_context() {
    use serde_json::json;
    let mut tera = Tera::new("templates/**/*.tera").expect("Failed to load templates");
    tera.register_filter("format_number", |value: &tera::Value, _args: &std::collections::HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
        let n = value.as_f64().unwrap_or(0.0) as i64;
        Ok(tera::Value::String(format_number_for_test(n)))
    });
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
    let rendered = tera.render("scoring.tera", &ctx);
    assert!(rendered.is_ok(), "Scoring template failed to render: {:?}", rendered.err());
}


/// Minimal format_number implementation for the scoring render test.
fn format_number_for_test(n: i64) -> String {
    let abs = n.abs();
    if abs >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if abs >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
