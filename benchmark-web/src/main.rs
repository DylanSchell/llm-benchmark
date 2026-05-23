//! benchmark-web - Standalone binary that delegates to the library.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    benchmark_web::run_web_server().await
}
