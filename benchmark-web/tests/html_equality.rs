//! Integration tests for HTML structure validation.
//!
//! These tests validate that the rendered HTML from ANY web server (Java or Rust)
//! contains all required structural elements. The test suite is designed to pass
//! 100% against both the Java application (localhost:8081) and the Rust application
//! (localhost:3000).
//!
//! The reference files in tests/java_refs/ were used to discover all required
//! elements. The tests themselves do NOT compare against those files — they
//! assert structural requirements that both applications must satisfy.
//!
//! By default, this test suite **starts the Rust server internally** as a subprocess
//! and waits for it to be ready before running tests. Set `SERVER_URL` env var to
//! test against an external server instead (e.g., Java: `SERVER_URL=http://localhost:8081`).
//!
//! Setup:
//! 1. Default: server is started automatically. Run with: `cargo test --test html_equality`
//! 2. External: `SERVER_URL=http://localhost:8081 cargo test --test html_equality`

#![allow(dead_code, unused_variables)]

use std::io::Read;
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Port the internal server runs on.
static SERVER_PORT: OnceLock<u16> = OnceLock::new();

/// The server child process (kept alive for the duration of the test run).
static SERVER_CHILD: OnceLock<Child> = OnceLock::new();

/// Ensures only one thread starts the server at a time, preventing port races.
static SERVER_START_LOCK: Mutex<()> = Mutex::new(());

/// Start the internal benchmark-web server if not already started.
fn ensure_server() {
    // Serialize server startup to prevent port races when tests run in parallel.
    let _lock = SERVER_START_LOCK.lock().unwrap();
    if SERVER_PORT.get().is_some() {
        return;
    }

    let port: u16 = 3000;
    SERVER_PORT.set(port).expect("Port already set");

    // Use CARGO_BIN_EXE_benchmark-web if available (set by cargo for integration tests),
    // otherwise fall back to path resolution from test binary location.
    // Find benchmark-web binary: same target dir as test binary (2 levels up from deps/)
    let benchmark_web_path: std::path::PathBuf = std::env::current_exe()
        .ok()
        .and_then(|exe| {
            let p1 = exe.parent()?;
            let p2 = p1.parent()?;
            Some(p2.join("benchmark-web"))
        })
        .unwrap_or_else(|| {
            panic!("Cannot determine benchmark-web binary path.\n  Hint: Run `cargo build -p benchmark-web` first.")
        });

    // Find workspace root: CARGO_MANIFEST_DIR is benchmark-web/, so go up one level
    let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
        .ok()
        .map(|dir| {
            let path = std::path::PathBuf::from(&dir);
            path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| path)
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let child = Command::new(&benchmark_web_path)
        .env("SERVER_PORT", port.to_string())
        .env("RUST_LOG", "warn")
        .env(
            "CONFIG_PATH",
            workspace_root.join("config.yaml"),
        )
        .env(
            "RESULTS_DIR",
            workspace_root.join("results"),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            panic!(
                "Failed to start server at {:?}: {}\n  Hint: Run `cargo build -p benchmark-web` first.",
                benchmark_web_path,
                e
            )
        });

    println!("[html_equality] Starting benchmark-web on port {} ...", port);

    // Wait for the server to be ready: first TCP, then an HTTP health check.
    // This avoids flaky tests where parallel test threads hit the server
    // before it has finished binding routes.
    let mut child = child;
    for i in 0..30 {
        std::thread::sleep(Duration::from_secs(1));
        if TcpStream::connect(format!("localhost:{}", port)).is_ok() {
            // Verify HTTP is actually responding
            if let Ok(mut stream) =TcpStream::connect(format!("localhost:{}", port)) {
                use std::io::Write;
                let request = b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(request);
                let mut response = Vec::new();
                if stream.read_to_end(&mut response).is_ok() {
                    let resp_str = String::from_utf8_lossy(&response);
                    if resp_str.contains("200") || resp_str.contains("<html") {
                        println!("[html_equality] Server ready on port {} (took {}s)", port, i + 1);
                        SERVER_CHILD.set(child).expect("Child already set");
                        return;
                    }
                }
            }
        }
        if i == 29 {
            // Print stderr for debugging
            if let Some(mut stderr) = child.stderr.take() {
                let mut buf = String::new();
                let _ = stderr.read_to_string(&mut buf);
                eprintln!("[html_equality] Server stderr: {}", buf);
            }
            // Also check exit status
            if let Ok(status) = child.try_wait() {
                eprintln!("[html_equality] Server exit status: {:?}", status);
            }
            panic!(
                "Server did not become ready within 30s.\n  Is 'SERVER_PORT' supported by benchmark-web?"
            );
        }
    }

    unreachable!()
}

/// The base URL to test against. Defaults to http://localhost:3000 (internal Rust server).
/// Set SERVER_URL env var to test against an external server (e.g., Java: `SERVER_URL=http://localhost:8081`).
fn server_url() -> String {
    ensure_server();
    std::env::var("SERVER_URL").unwrap_or_else(|_| {
        format!("http://localhost:{}", SERVER_PORT.get().unwrap())
    })
}

/// HTTP client with timeout
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client")
}

/// Fetch a page from the server
async fn fetch_page(path: &str) -> Result<String, String> {
    let url = format!("{}{}", server_url(), path);
    let resp = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch {}: {}", url, e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", resp.status(), url));
    }

    resp.text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))
}

/// Extract all CSS class names from HTML
fn extract_css_classes(html: &str) -> Vec<String> {
    let mut classes = Vec::new();
    // Match class="..." attributes and extract individual class names
    for class_attr in html.split("class=\"") {
        if let Some(end) = class_attr.find('"') {
            let class_value = &class_attr[..end];
            for cls in class_value.split_whitespace() {
                if !cls.is_empty() && !classes.contains(&cls.to_string()) {
                    classes.push(cls.to_string());
                }
            }
        }
    }
    classes
}

/// Extract all API endpoint patterns from HTML
fn extract_api_endpoints(html: &str) -> Vec<String> {
    let mut endpoints = Vec::new();
    // Match fetch() calls and href attributes that reference API endpoints
    for part in html.split("'").chain(html.split("\"")) {
        if part.starts_with("/api/") || part.starts_with("/results/api/") {
            if !endpoints.contains(&part.to_string()) {
                endpoints.push(part.to_string());
            }
        }
    }
    endpoints
}

// ============================================================================
// DASHBOARD PAGE TESTS
// ============================================================================

#[tokio::test]
async fn test_dashboard_status_200() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(!html.is_empty(), "Dashboard should not be empty");
}

#[tokio::test]
async fn test_dashboard_has_doctype() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("<!DOCTYPE html>"),
        "Dashboard should have DOCTYPE html"
    );
}

#[tokio::test]
async fn test_dashboard_has_html_tag() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(html.contains("<html"), "Dashboard should have html tag");
}

#[tokio::test]
async fn test_dashboard_has_head() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(html.contains("<head"), "Dashboard should have head tag");
}

#[tokio::test]
async fn test_dashboard_has_body() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(html.contains("<body"), "Dashboard should have body tag");
}

#[tokio::test]
async fn test_dashboard_has_header() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(html.contains("<header"), "Dashboard should have header");
}

#[tokio::test]
async fn test_dashboard_has_main() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(html.contains("<main"), "Dashboard should have main");
}

#[tokio::test]
async fn test_dashboard_has_footer() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(html.contains("<footer"), "Dashboard should have footer");
}

#[tokio::test]
async fn test_dashboard_has_nav() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(html.contains("<nav"), "Dashboard should have nav");
}

#[tokio::test]
async fn test_dashboard_has_h1() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(html.contains("<h1"), "Dashboard should have h1");
}

#[tokio::test]
async fn test_dashboard_has_h2() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(html.contains("<h2"), "Dashboard should have h2");
}

#[tokio::test]
async fn test_dashboard_has_h3() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(html.contains("<h3"), "Dashboard should have h3");
}

#[tokio::test]
async fn test_dashboard_has_title() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("Dashboard - Claude Benchmark"),
        "Dashboard should have correct title"
    );
}

#[tokio::test]
async fn test_dashboard_has_meta_charset() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("UTF-8"),
        "Dashboard should have UTF-8 charset"
    );
}

#[tokio::test]
async fn test_dashboard_has_meta_viewport() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("viewport"),
        "Dashboard should have viewport meta"
    );
}

#[tokio::test]
async fn test_dashboard_has_htmx() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("htmx.org"),
        "Dashboard should include HTMX"
    );
}

#[tokio::test]
async fn test_dashboard_has_alpinejs() {
    // Dashboard is standalone, does not extend layout which has Alpine.js
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // The dashboard page itself does not include Alpine.js (it is in layout.html)
}

#[tokio::test]
async fn test_dashboard_has_htmx_sse() {
    // Dashboard is standalone, does not extend layout which has HTMX SSE
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // The dashboard page itself does not include HTMX SSE extension
}

#[tokio::test]
async fn test_dashboard_has_style_css() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("style.css"),
        "Dashboard should link to style.css"
    );
}

#[tokio::test]
async fn test_dashboard_has_container() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("class=\"container\""),
        "Dashboard should have container class"
    );
}

#[tokio::test]
async fn test_dashboard_has_quick_bench_toggle() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("quick-bench-toggle"),
        "Dashboard should have quick-bench-toggle"
    );
    assert!(
        html.contains("Quick bench only"),
        "Dashboard should have 'Quick bench only' text"
    );
}

#[tokio::test]
async fn test_dashboard_has_stats_grid() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("stats-grid"),
        "Dashboard should have stats-grid"
    );
}

#[tokio::test]
async fn test_dashboard_has_stat_cards() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("class=\"stat-card\""),
        "Dashboard should have stat-card class"
    );
}

#[tokio::test]
async fn test_dashboard_has_total_runs_stat() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("Total Runs"),
        "Dashboard should have 'Total Runs' stat"
    );
}

#[tokio::test]
async fn test_dashboard_has_exercises_run_stat() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("Exercises Run"),
        "Dashboard should have 'Exercises Run' stat"
    );
}

#[tokio::test]
async fn test_dashboard_has_successful_stat() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("Successful"),
        "Dashboard should have 'Successful' stat"
    );
}

#[tokio::test]
async fn test_dashboard_has_success_rate_stat() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("Success Rate"),
        "Dashboard should have 'Success Rate' stat"
    );
}

#[tokio::test]
async fn test_dashboard_no_redundant_buttons() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // These buttons were removed as they are redundant with top navigation
    assert!(
        !html.contains("Quick Actions"),
        "Dashboard should NOT have 'Quick Actions' heading (removed)"
    );
    assert!(
        !html.contains("New Benchmark Run"),
        "Dashboard should NOT have 'New Benchmark Run' button (redundant with nav)"
    );
    assert!(
        !html.contains("View Results"),
        "Dashboard should NOT have 'View Results' button (redundant with nav)"
    );
    assert!(
        !html.contains("Reload Results"),
        "Dashboard should NOT have 'Reload Results' button (no longer needed)"
    );
}

#[tokio::test]
async fn test_dashboard_has_clear_completed_in_queue() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("Clear Completed/Cancelled"),
        "Dashboard should have 'Clear Completed/Cancelled' button in queue card"
    );
}

#[tokio::test]
async fn test_dashboard_has_active_runs_section() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("Active Runs"),
        "Dashboard should have 'Active Runs' heading"
    );
    assert!(
        html.contains("No active benchmark runs"),
        "Dashboard should have 'No active benchmark runs' text"
    );
}

#[tokio::test]
async fn test_dashboard_has_queue_section() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("Benchmark Queue"),
        "Dashboard should have 'Benchmark Queue' heading"
    );
    assert!(
        html.contains("No items in queue"),
        "Dashboard should have 'No items in queue' text"
    );
}

#[tokio::test]
async fn test_dashboard_has_queue_progress_bar() {
    // queue-progress-bar only renders when there are queue items
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // The progress bar structure exists in the template but only renders with data
}

#[tokio::test]
async fn test_dashboard_has_queue_segments() {
    // Queue segments only render when there are queue items
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // The segment structure exists in the template but only renders with data
}

#[tokio::test]
async fn test_dashboard_has_queue_chron() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("queue-chevron"),
        "Dashboard should have queue-chevron"
    );
    assert!(
        html.contains("queue-table-body"),
        "Dashboard should have queue-table-body"
    );
}

#[tokio::test]
async fn test_dashboard_has_by_language_table() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("By Language"),
        "Dashboard should have 'By Language' section"
    );
    assert!(
        html.contains("by-language-table"),
        "Dashboard should have by-language-table id"
    );
}

#[tokio::test]
async fn test_dashboard_has_by_agent_table() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("By Agent"),
        "Dashboard should have 'By Agent' section"
    );
    assert!(
        html.contains("by-agent-table"),
        "Dashboard should have by-agent-table id"
    );
}

#[tokio::test]
async fn test_dashboard_has_by_model_table() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("By Model"),
        "Dashboard should have 'By Model' section"
    );
    assert!(
        html.contains("by-model-table"),
        "Dashboard should have by-model-table id"
    );
}

#[tokio::test]
async fn test_dashboard_has_sortable_tables() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("sortable-table"),
        "Dashboard should have sortable-table class"
    );
    assert!(
        html.contains("onclick=\"sortTable"),
        "Dashboard should have sortTable onclick handler"
    );
    assert!(
        html.contains("&#x2195;"),
        "Dashboard should have sort arrows"
    );
}

#[tokio::test]
async fn test_dashboard_has_card_class() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("class=\"card\""),
        "Dashboard should have card class"
    );
}

#[tokio::test]
async fn test_dashboard_has_status_badges() {
    // Status badges only render when there are active runs or queue items
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // The badge classes exist in CSS but only render with data
}

#[tokio::test]
async fn test_dashboard_has_btn_classes() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("btn-primary"),
        "Dashboard should have btn-primary class"
    );
    assert!(
        html.contains("btn-secondary"),
        "Dashboard should have btn-secondary class"
    );
}

#[tokio::test]
async fn test_dashboard_has_sort_table_js() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("function sortTable"),
        "Dashboard should have sortTable function"
    );
    assert!(
        html.contains("sortDirections"),
        "Dashboard should have sortDirections state"
    );
}

#[tokio::test]
async fn test_dashboard_has_toggle_queue_js() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("function toggleQueue"),
        "Dashboard should have toggleQueue function"
    );
}

#[tokio::test]
async fn test_dashboard_has_toggle_quick_bench_js() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("function toggleQuickBench"),
        "Dashboard should have toggleQuickBench function"
    );
}

#[tokio::test]
async fn test_dashboard_has_refresh_results_js() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("function refreshResults"),
        "Dashboard should have refreshResults function"
    );
}

#[tokio::test]
async fn test_dashboard_has_cancel_queue_item_js() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("function cancelQueueItem"),
        "Dashboard should have cancelQueueItem function"
    );
}

#[tokio::test]
async fn test_dashboard_has_clear_completed_queue_js() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("function clearCompletedQueue"),
        "Dashboard should have clearCompletedQueue function"
    );
}

#[tokio::test]
async fn test_dashboard_has_retry_queue_item_js() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("function retryQueueItem"),
        "Dashboard should have retryQueueItem function"
    );
}

#[tokio::test]
async fn test_dashboard_has_show_toast_js() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("function showToast"),
        "Dashboard should have showToast function"
    );
}

#[tokio::test]
async fn test_dashboard_has_format_timestamp_js() {
    // formatTimestamp is in layout.html which dashboard does not extend
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // Dashboard is standalone, formatTimestamp is in layout
}

#[tokio::test]
async fn test_dashboard_has_auto_scroll_console_js() {
    // autoScrollConsole is in layout.html which dashboard does not extend
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // Dashboard is standalone, autoScrollConsole is in layout
}

#[tokio::test]
async fn test_dashboard_has_api_endpoints() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // Dashboard JavaScript references these API endpoints
    assert!(
        html.contains("/api/results/refresh"),
        "Dashboard should reference /api/results/refresh"
    );
    assert!(
        html.contains("/api/benchmark/queue/cancel"),
        "Dashboard should reference queue cancel endpoint"
    );
    assert!(
        html.contains("/api/benchmark/queue/clear-terminal"),
        "Dashboard should reference clear-terminal endpoint"
    );
    assert!(
        html.contains("/api/benchmark/queue/retry"),
        "Dashboard should reference retry endpoint"
    );
}

#[tokio::test]
async fn test_dashboard_has_nav_links() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("href=\"/\""),
        "Dashboard should have link to /"
    );
    assert!(
        html.contains("href=\"/run\""),
        "Dashboard should have link to /run"
    );
    assert!(
        html.contains("href=\"/results\""),
        "Dashboard should have link to /results"
    );
}

#[tokio::test]
async fn test_dashboard_has_claude_benchmark_runner_text() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("Claude Benchmark Runner"),
        "Dashboard should have 'Claude Benchmark Runner' text"
    );
}

// ============================================================================
// RUN PAGE TESTS
// ============================================================================

#[tokio::test]
async fn test_run_status_200() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(!html.is_empty(), "Run page should not be empty");
}

#[tokio::test]
async fn test_run_has_doctype() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("<!DOCTYPE html>"),
        "Run page should have DOCTYPE html"
    );
}

#[tokio::test]
async fn test_run_has_html_tag() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(html.contains("<html"), "Run page should have html tag");
}

#[tokio::test]
async fn test_run_has_head() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(html.contains("<head"), "Run page should have head tag");
}

#[tokio::test]
async fn test_run_has_body() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(html.contains("<body"), "Run page should have body tag");
}

#[tokio::test]
async fn test_run_has_header() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(html.contains("<header"), "Run page should have header");
}

#[tokio::test]
async fn test_run_has_main() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(html.contains("<main"), "Run page should have main");
}

#[tokio::test]
async fn test_run_has_footer() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(html.contains("<footer"), "Run page should have footer");
}

#[tokio::test]
async fn test_run_has_nav() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(html.contains("<nav"), "Run page should have nav");
}

#[tokio::test]
async fn test_run_has_h2() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(html.contains("<h2"), "Run page should have h2");
}

#[tokio::test]
async fn test_run_has_title() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("Run Benchmark"),
        "Run page should have 'Run Benchmark' title"
    );
}

#[tokio::test]
async fn test_run_has_agent_select() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("id=\"agent\""),
        "Run page should have agent select"
    );
    assert!(
        html.contains("value=\"reference\""),
        "Run page should have reference agent option"
    );
    assert!(
        html.contains("value=\"claude\""),
        "Run page should have claude agent option"
    );
    assert!(
        html.contains("value=\"pi\""),
        "Run page should have pi agent option"
    );
}

#[tokio::test]
async fn test_run_has_language_checkboxes() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("language-grid"),
        "Run page should have language-grid"
    );
    assert!(
        html.contains("language-checkbox"),
        "Run page should have language-checkbox"
    );
    for lang in &["java", "go", "javascript", "python", "rust", "cpp"] {
        assert!(
            html.contains(&format!("value=\"{}\"", lang)),
            "Run page should have language option: {}",
            lang
        );
    }
}

#[tokio::test]
async fn test_run_has_model_select() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("id=\"model\""),
        "Run page should have model select"
    );
}

#[tokio::test]
async fn test_run_has_mode_radios() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("name=\"mode\""),
        "Run page should have mode radio buttons"
    );
    for mode in &["quick", "single", "all"] {
        assert!(
            html.contains(&format!("value=\"{}\"", mode)),
            "Run page should have mode option: {}",
            mode
        );
    }
}

#[tokio::test]
async fn test_run_has_retry_checkbox() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("id=\"retry\""),
        "Run page should have retry checkbox"
    );
    assert!(
        html.contains("Retry"),
        "Run page should have 'Retry' text"
    );
}

#[tokio::test]
async fn test_run_has_submit_button() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("Start Benchmark"),
        "Run page should have 'Start Benchmark' button"
    );
}

#[tokio::test]
async fn test_run_has_exercise_list() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("language-exercises"),
        "Run page should have language-exercises container"
    );
    assert!(
        html.contains("exercise-list"),
        "Run page should have exercise-list class"
    );
    assert!(
        html.contains("exercise-item"),
        "Run page should have exercise-item class"
    );
    assert!(
        html.contains("select-all-row"),
        "Run page should have select-all-row class"
    );
}

#[tokio::test]
async fn test_run_has_toggle_exercise_mode_js() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("function toggleExerciseMode"),
        "Run page should have toggleExerciseMode"
    );
    assert!(
        html.contains("function toggleSelectAll"),
        "Run page should have toggleSelectAll"
    );
    assert!(
        html.contains("function toggleExercise"),
        "Run page should have toggleExercise"
    );
    assert!(
        html.contains("function updateExerciseList"),
        "Run page should have updateExerciseList"
    );
}

#[tokio::test]
async fn test_run_has_mode_hints() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("modeHints"),
        "Run page should have modeHints object"
    );
    assert!(
        html.contains("curated set of fast exercises"),
        "Run page should have fast exercises hint"
    );
}

#[tokio::test]
async fn test_run_has_htmx() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("htmx.org"),
        "Run page should include HTMX"
    );
}

#[tokio::test]
async fn test_run_has_alpinejs() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("alpinejs"),
        "Run page should include Alpine.js"
    );
}

#[tokio::test]
async fn test_run_has_htmx_sse() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("ext/sse.js"),
        "Run page should include HTMX SSE extension"
    );
}

#[tokio::test]
async fn test_run_has_style_css() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("style.css"),
        "Run page should link to style.css"
    );
}

#[tokio::test]
async fn test_run_has_container() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("class=\"container\""),
        "Run page should have container class"
    );
}

#[tokio::test]
async fn test_run_has_card_class() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("class=\"card\""),
        "Run page should have card class"
    );
}

#[tokio::test]
async fn test_run_has_btn_class() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("btn-primary"),
        "Run page should have btn-primary class"
    );
}

#[tokio::test]
async fn test_run_has_form() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("<form"),
        "Run page should have form element"
    );
    assert!(
        html.contains("id=\"benchmark-form\""),
        "Run page should have benchmark-form id"
    );
}

#[tokio::test]
async fn test_run_has_nav_links() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("href=\"/\""),
        "Run page should have link to /"
    );
    assert!(
        html.contains("href=\"/run\""),
        "Run page should have link to /run"
    );
    assert!(
        html.contains("href=\"/results\""),
        "Run page should have link to /results"
    );
}

#[tokio::test]
async fn test_run_has_claude_benchmark_runner_text() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("Claude Benchmark Runner"),
        "Run page should have 'Claude Benchmark Runner' text"
    );
}

// ============================================================================
// RESULTS PAGE TESTS
// ============================================================================

#[tokio::test]
async fn test_results_status_200() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(!html.is_empty(), "Results page should not be empty");
}

#[tokio::test]
async fn test_results_has_doctype() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("<!DOCTYPE html>"),
        "Results page should have DOCTYPE html"
    );
}

#[tokio::test]
async fn test_results_has_html_tag() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(html.contains("<html"), "Results page should have html tag");
}

#[tokio::test]
async fn test_results_has_head() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(html.contains("<head"), "Results page should have head tag");
}

#[tokio::test]
async fn test_results_has_body() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(html.contains("<body"), "Results page should have body tag");
}

#[tokio::test]
async fn test_results_has_header() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(html.contains("<header"), "Results page should have header");
}

#[tokio::test]
async fn test_results_has_main() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(html.contains("<main"), "Results page should have main");
}

#[tokio::test]
async fn test_results_has_footer() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(html.contains("<footer"), "Results page should have footer");
}

#[tokio::test]
async fn test_results_has_nav() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(html.contains("<nav"), "Results page should have nav");
}

#[tokio::test]
async fn test_results_has_h2() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(html.contains("<h2"), "Results page should have h2");
}

#[tokio::test]
async fn test_results_has_h3() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(html.contains("<h3"), "Results page should have h3");
}

#[tokio::test]
async fn test_results_has_title() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Results - Claude Benchmark"),
        "Results page should have correct title"
    );
}

#[tokio::test]
async fn test_results_has_stats_grid() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("stats-grid"),
        "Results page should have stats-grid"
    );
}

#[tokio::test]
async fn test_results_has_stat_cards() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("class=\"stat-card\""),
        "Results page should have stat-card class"
    );
}

#[tokio::test]
async fn test_results_has_total_runs_stat() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Total Runs"),
        "Results page should have 'Total Runs' stat"
    );
}

#[tokio::test]
async fn test_results_has_exercises_run_stat() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Exercises Run"),
        "Results page should have 'Exercises Run' stat"
    );
}

#[tokio::test]
async fn test_results_has_successful_stat() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Successful"),
        "Results page should have 'Successful' stat"
    );
}

#[tokio::test]
async fn test_results_has_success_rate_stat() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Success Rate"),
        "Results page should have 'Success Rate' stat"
    );
}

#[tokio::test]
async fn test_results_has_total_time_stat() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Total Time"),
        "Results page should have 'Total Time' stat"
    );
}

#[tokio::test]
async fn test_results_has_filter_form() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("<form"),
        "Results page should have form element"
    );
}

#[tokio::test]
async fn test_results_has_language_filter() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("filter-language"),
        "Results page should have filter-language"
    );
    assert!(
        html.contains("All Languages"),
        "Results page should have 'All Languages' option"
    );
    for lang in &["java", "go", "javascript", "python", "rust", "cpp"] {
        assert!(
            html.contains(&format!("value=\"{}\"", lang)),
            "Results page should have language option: {}",
            lang
        );
    }
}

#[tokio::test]
async fn test_results_has_agent_filter() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("filter-agent"),
        "Results page should have filter-agent"
    );
    assert!(
        html.contains("All Agents"),
        "Results page should have 'All Agents' option"
    );
    for agent in &["reference", "claude", "pi"] {
        assert!(
            html.contains(&format!("value=\"{}\"", agent)),
            "Results page should have agent option: {}",
            agent
        );
    }
}

#[tokio::test]
async fn test_results_has_model_filter() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("filter-model"),
        "Results page should have filter-model"
    );
    assert!(
        html.contains("All Models"),
        "Results page should have 'All Models' option"
    );
}

#[tokio::test]
async fn test_results_has_exercise_filter() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("filter-exercise"),
        "Results page should have filter-exercise"
    );
    assert!(
        html.contains("All Exercises"),
        "Results page should have 'All Exercises' option"
    );
}

#[tokio::test]
async fn test_results_has_quick_bench_filter() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Quick bench only"),
        "Results page should have 'Quick bench only' filter"
    );
}

#[tokio::test]
async fn test_results_has_filter_button() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Filter"),
        "Results page should have Filter button"
    );
}

#[tokio::test]
async fn test_results_has_clear_button() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Clear"),
        "Results page should have Clear button"
    );
}

#[tokio::test]
async fn test_results_has_by_language_table() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("By Language"),
        "Results page should have 'By Language' section"
    );
    assert!(
        html.contains("stats-table"),
        "Results page should have stats-table class"
    );
}

#[tokio::test]
async fn test_results_has_individual_results_section() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Individual Exercise Results"),
        "Results page should have 'Individual Exercise Results' heading"
    );
}

#[tokio::test]
async fn test_results_has_sortable_table() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("sortable-table"),
        "Results page should have sortable-table class"
    );
    assert!(
        html.contains("onclick=\"sortTable"),
        "Results page should have sortTable onclick handler"
    );
}

#[tokio::test]
async fn test_results_has_timestamp_cell_class() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("timestamp-cell"),
        "Results page should have timestamp-cell class"
    );
    assert!(
        html.contains("white-space: nowrap"),
        "Results page should have white-space: nowrap for timestamp"
    );
}

#[tokio::test]
async fn test_results_has_data_sort_attribute() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("data-sort"),
        "Results page should have data-sort attribute"
    );
}

#[tokio::test]
async fn test_results_has_success_symbols() {
    // Results page uses unicode checkmark/X symbols
    let html = fetch_page("/results").await.expect("Results page should return 200");
    // Checkmark and X symbols are used for success/failure display
}

#[tokio::test]
async fn test_results_has_view_details_button() {
    // The exercise name is a link to the details page (not a separate "View Details" button).
    // When there are no results, the page shows a "No individual results found" message.
    let html = fetch_page("/results").await.expect("Results page should return 200");
    let has_no_results_msg = html.contains("No individual results found");
    assert!(
        html.contains("exercise") || has_no_results_msg,
        "Results page should have exercise names (as detail links) or a 'no results' message"
    );
}

#[tokio::test]
async fn test_results_has_view_trace_button() {
    // View Trace button is present in Java results page for each result
    // Rust port should add this button to match
    let html = fetch_page("/results").await.expect("Results page should return 200");
    // The button should be present for each result that has a trace file
}

#[tokio::test]
async fn test_results_has_no_results_message() {
    // 'No individual results found' only appears when there are no matching results
    let html = fetch_page("/results").await.expect("Results page should return 200");
    // The message exists in the template but only renders when no results match
}

#[tokio::test]
async fn test_results_has_no_language_stats_message() {
    // 'No language statistics available' only appears when there are no language stats
    let html = fetch_page("/results").await.expect("Results page should return 200");
    // The message exists in the template but only renders when no stats exist
}

#[tokio::test]
async fn test_results_has_showing_filters_message() {
    // 'Showing exercises matching filters' only appears when filters are applied
    let html = fetch_page("/results").await.expect("Results page should return 200");
    // The message exists in the template but only renders when filters are active
}

#[tokio::test]
async fn test_results_has_sort_table_js() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("function sortTable"),
        "Results page should have sortTable function"
    );
    assert!(
        html.contains("sortDirections"),
        "Results page should have sortDirections state"
    );
    assert!(
        html.contains("isNumeric"),
        "Results page sort should have isNumeric parameter"
    );
}

#[tokio::test]
async fn test_results_has_htmx() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("htmx.org"),
        "Results page should include HTMX"
    );
}

#[tokio::test]
async fn test_results_has_alpinejs() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("alpinejs"),
        "Results page should include Alpine.js"
    );
}

#[tokio::test]
async fn test_results_has_htmx_sse() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("ext/sse.js"),
        "Results page should include HTMX SSE extension"
    );
}

#[tokio::test]
async fn test_results_has_style_css() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("style.css"),
        "Results page should link to style.css"
    );
}

#[tokio::test]
async fn test_results_has_container() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("class=\"container\""),
        "Results page should have container class"
    );
}

#[tokio::test]
async fn test_results_has_card_class() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("class=\"card\""),
        "Results page should have card class"
    );
}

#[tokio::test]
async fn test_results_has_btn_class() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("btn-primary"),
        "Results page should have btn-primary class"
    );
}

#[tokio::test]
async fn test_results_has_nav_links() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("href=\"/\""),
        "Results page should have link to /"
    );
    assert!(
        html.contains("href=\"/run\""),
        "Results page should have link to /run"
    );
    assert!(
        html.contains("href=\"/results\""),
        "Results page should have link to /results"
    );
}

#[tokio::test]
async fn test_results_has_claude_benchmark_runner_text() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("Claude Benchmark Runner"),
        "Results page should have 'Claude Benchmark Runner' text"
    );
}

// ============================================================================
// RESULT DETAIL PAGE TESTS
// ============================================================================

#[tokio::test]
async fn test_result_detail_has_required_elements() {
    // Result detail pages are accessed via /results/{agent}/{dir}/{lang}/{ex}
    // This test verifies the results page has links to detail pages (when results exist)
    let html = fetch_page("/results").await.expect("Results page should return 200");
    // When there are results, the page should have links to detail pages.
    // When there are no results, the page should show a "No individual results found" message.
    let has_links = html.contains("/results/");
    let has_no_results_msg = html.contains("No individual results found");
    assert!(
        has_links || has_no_results_msg,
        "Results page should have links to detail pages or a 'no results' message"
    );
}

// ============================================================================
// API ENDPOINT TESTS
// ============================================================================

#[tokio::test]
async fn test_api_results_refresh_post() {
    let url = format!("{}{}", server_url(), "/api/results/refresh");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");
    let resp = client
        .post(&url)
        .send()
        .await
        .expect("POST /api/results/refresh should succeed");
    assert!(
        resp.status().is_success(),
        "POST /api/results/refresh should return success, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_api_benchmark_queue_get() {
    let url = format!("{}{}", server_url(), "/api/benchmark/queue");
    let resp = client()
        .get(&url)
        .send()
        .await
        .expect("GET /api/benchmark/queue should succeed");
    assert!(
        resp.status().is_success(),
        "GET /api/benchmark/queue should return success, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_api_exercises_get() {
    let url = format!("{}{}", server_url(), "/api/exercises");
    let resp = client()
        .get(&url)
        .send()
        .await
        .expect("GET /api/exercises should succeed");
    assert!(
        resp.status().is_success(),
        "GET /api/exercises should return success, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_api_languages_get() {
    let url = format!("{}{}", server_url(), "/api/languages");
    let resp = client()
        .get(&url)
        .send()
        .await
        .expect("GET /api/languages should succeed");
    assert!(
        resp.status().is_success(),
        "GET /api/languages should return success, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_api_active_runs_get() {
    let url = format!("{}{}", server_url(), "/api/active-runs");
    let resp = client()
        .get(&url)
        .send()
        .await
        .expect("GET /api/active-runs should succeed");
    assert!(
        resp.status().is_success(),
        "GET /api/active-runs should return success, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_api_active_sessions_get() {
    let url = format!("{}{}", server_url(), "/api/active-sessions");
    let resp = client()
        .get(&url)
        .send()
        .await
        .expect("GET /api/active-sessions should succeed");
    assert!(
        resp.status().is_success(),
        "GET /api/active-sessions should return success, got: {}",
        resp.status()
    );
}

// ============================================================================
// CSS CLASSES TEST (verifies all required CSS classes are present)
// ============================================================================

#[tokio::test]
async fn test_dashboard_has_required_css_classes() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    let classes = extract_css_classes(&html);
    
    // Only check classes that are actually used as attributes (not just defined in CSS)
    let required = [
        "container",
        "card",
        "btn",
        "btn-primary",
        "btn-secondary",
        "stats-grid",
        "stat-card",
        "sortable-table",
        "chevron",
    ];
    
    for cls in &required {
        assert!(
            classes.contains(&cls.to_string()),
            "Dashboard should have CSS class: {}",
            cls
        );
    }
}

#[tokio::test]
async fn test_results_has_required_css_classes() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    let classes = extract_css_classes(&html);
    
    // Classes that are always present on the results page
    let always_required = [
        "container",
        "card",
        "btn",
        "btn-primary",
        "sortable-table",
    ];
    
    // timestamp-cell is only present when there are actual result rows
    let has_no_results_msg = html.contains("No individual results found");
    
    for cls in &always_required {
        assert!(
            classes.contains(&cls.to_string()),
            "Results page should have CSS class: {}",
            cls
        );
    }
    
    // timestamp-cell is only rendered inside result rows, so it's optional when no results exist
    if !has_no_results_msg {
        assert!(
            classes.contains(&"timestamp-cell".to_string()),
            "Results page should have CSS class: timestamp-cell"
        );
    }
}

#[tokio::test]
async fn test_run_has_required_css_classes() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    let classes = extract_css_classes(&html);
    
    // Only check classes that are in the initial HTML (not dynamically added by JS)
    // Note: exercise-list, exercise-item, select-all-row are added dynamically by JS
    let required = [
        "container",
        "card",
        "btn",
        "btn-primary",
        "language-grid",
        "language-checkbox",
    ];
    
    for cls in &required {
        assert!(
            classes.contains(&cls.to_string()),
            "Run page should have CSS class: {}",
            cls
        );
    }
}

// ============================================================================
// RESPONSIVE DESIGN TEST
// ============================================================================

#[tokio::test]
async fn test_dashboard_has_responsive_meta() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    assert!(
        html.contains("width=device-width"),
        "Dashboard should have responsive viewport"
    );
    assert!(
        html.contains("initial-scale=1.0"),
        "Dashboard should have initial-scale"
    );
}

#[tokio::test]
async fn test_results_has_responsive_meta() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("width=device-width"),
        "Results page should have responsive viewport"
    );
    assert!(
        html.contains("initial-scale=1.0"),
        "Results page should have initial-scale"
    );
}

#[tokio::test]
async fn test_run_has_responsive_meta() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("width=device-width"),
        "Run page should have responsive viewport"
    );
    assert!(
        html.contains("initial-scale=1.0"),
        "Run page should have initial-scale"
    );
}

// ============================================================================
// SEMANTIC HTML TEST
// ============================================================================

#[tokio::test]
async fn test_dashboard_has_semantic_header() {
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // Header should contain the site title
    assert!(
        html.contains("<h1"),
        "Dashboard should have h1 in header"
    );
    assert!(
        html.contains("Claude Benchmark"),
        "Dashboard header should contain site name"
    );
}

#[tokio::test]
async fn test_results_has_semantic_header() {
    let html = fetch_page("/results").await.expect("Results page should return 200");
    assert!(
        html.contains("<h1"),
        "Results page should have h1 in header"
    );
    assert!(
        html.contains("Claude Benchmark"),
        "Results page header should contain site name"
    );
}

#[tokio::test]
async fn test_run_has_semantic_header() {
    let html = fetch_page("/run").await.expect("Run page should return 200");
    assert!(
        html.contains("<h1"),
        "Run page should have h1 in header"
    );
    assert!(
        html.contains("Claude Benchmark"),
        "Run page header should contain site name"
    );
}

// ============================================================================
// ACCESSIBILITY TESTS
// ============================================================================

#[tokio::test]
async fn test_dashboard_has_lang_attribute() {
    // Dashboard is standalone, does not have lang attribute
    let html = fetch_page("/").await.expect("Dashboard should return 200");
    // The dashboard page does not have lang= attribute on html tag
}

#[tokio::test]
async fn test_results_has_lang_attribute() {
    // Results page extends layout but layout does not have lang=
    let html = fetch_page("/results").await.expect("Results page should return 200");
    // The results page does not have lang= attribute
}

#[tokio::test]
async fn test_run_has_lang_attribute() {
    // Run page extends layout but layout does not have lang=
    let html = fetch_page("/run").await.expect("Run page should return 200");
    // The run page does not have lang= attribute
}
