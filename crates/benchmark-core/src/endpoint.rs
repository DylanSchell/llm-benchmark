//! Choosing the inference endpoint, and telling the container about it.
//!
//! Two settings describe where the model lives, and they are separate on purpose (see the
//! README): `inference_endpoint` is the *host-side* base URL the dashboard asks for its model
//! list, while `docker.environment` is what the agent running *inside the container* uses.
//! On a machine serving a model locally both should work without being configured, and both
//! should agree on the port once one is found.
//!
//! This module implements the two halves of that:
//!
//! 1. [`resolve_endpoints`] adopts the configured endpoint when there is one; otherwise it
//!    probes [`LOCAL_ENDPOINT_CANDIDATES`] and adopts the first that answers.
//! 2. When the adopted endpoint is on this machine, the container-facing default is derived
//!    from it by replacing the loopback host with `host.docker.internal` — the name a
//!    container uses for its host — keeping the port and path.

use std::collections::HashMap;
use std::time::Duration;

use benchmark_types::config::{Config, DockerConfig};
use tracing::info;

/// The local ports probed when no `inference_endpoint` is configured, in priority order.
///
/// 8000 is uvicorn/FastAPI and vLLM; 8080 is Ollama's OpenAI-compatible server and this
/// project's documented default; 9931 is used by some local runners. Only these are probed —
/// the list is deliberately short so that startup is not a port scan.
pub const LOCAL_ENDPOINT_CANDIDATES: [&str; 3] = [
    "http://localhost:8000/v1",
    "http://localhost:8080/v1",
    "http://localhost:9931/v1",
];

/// How long one candidate has to answer. All candidates are probed concurrently, so this is
/// also the worst case this adds to startup.
const PROBE_TIMEOUT: Duration = Duration::from_millis(750);

/// Decide which host-side inference endpoint to use, and record it on `config`.
///
/// - If `inference_endpoint` is set, it is adopted as-is and nothing is probed.
/// - Otherwise every [`LOCAL_ENDPOINT_CANDIDATES`] entry is asked for `GET {candidate}/models`
///   and the first (in priority order) that answers is adopted.
/// - If none answers, the endpoint is left unset, so the dashboard uses its built-in model
///   list rather than being pointed at a port that is known to be dead.
///
/// When an endpoint is adopted and it is on this machine, the container's default
/// `OPENAI_BASE_URL` is derived from it: same port and path, `host.docker.internal` as the
/// host. An `OPENAI_BASE_URL` the user set explicitly is never touched.
///
/// Returns the endpoint that will be used, if any.
pub async fn resolve_endpoints(config: &mut Config) -> Option<String> {
    resolve_with(config, &LOCAL_ENDPOINT_CANDIDATES, PROBE_TIMEOUT).await
}

async fn resolve_with(
    config: &mut Config,
    candidates: &[&str],
    timeout: Duration,
) -> Option<String> {
    let endpoint = match config.inference_endpoint.clone() {
        Some(configured) => {
            info!("Using configured inference endpoint: {}", configured);
            Some(configured)
        }
        None => probe_endpoints(candidates, timeout).await,
    };

    if let Some(ref endpoint) = endpoint {
        config.inference_endpoint = Some(endpoint.clone());
        apply_container_endpoint_default(&mut config.docker, endpoint);
    }

    endpoint
}

/// Ask every candidate for `GET {base}/models` and return the first (in candidate order) that
/// answers with a success status.
async fn probe_endpoints(candidates: &[&str], timeout: Duration) -> Option<String> {
    let client = reqwest::Client::new();
    let mut probes = tokio::task::JoinSet::new();

    for (index, base) in candidates.iter().enumerate() {
        let client = client.clone();
        let base = base.to_string();
        probes.spawn(async move {
            let url = format!("{}/models", base.trim_end_matches('/'));
            // A plain 2xx is not enough: port 8000 in particular is full of FastAPI apps that
            // answer something. Accept only a body the consumer can use, which is exactly what
            // `ExerciseRunner::fetch_models` reads — JSON whose `data` is an array.
            let answered = match client.get(&url).timeout(timeout).send().await {
                Ok(response) if response.status().is_success() => response
                    .json::<serde_json::Value>()
                    .await
                    .ok()
                    .and_then(|body| body.get("data").map(serde_json::Value::is_array))
                    .unwrap_or(false),
                _ => false,
            };
            (index, answered)
        });
    }

    // Collect by candidate index, so the outcome does not depend on which probe happened to
    // finish first: priority order is the point of the list.
    let mut answered = [false; LOCAL_ENDPOINT_CANDIDATES.len()];
    while let Some(result) = probes.join_next().await {
        if let Ok((index, true)) = result {
            answered[index] = true;
        }
    }

    let found = answered
        .iter()
        .position(|answered| *answered)
        .map(|index| candidates[index].to_string());

    match &found {
        Some(endpoint) => info!(
            "No inference endpoint configured; a model server answered at {}",
            endpoint
        ),
        None => info!(
            "No inference endpoint configured and nothing answered on {}; the dashboard will \
             use its built-in model list",
            candidates.join(", ")
        ),
    }

    found
}

/// Default the container's `OPENAI_BASE_URL` from the host-side endpoint, when that endpoint
/// is on this machine. A variable the user set themselves is left alone.
fn apply_container_endpoint_default(docker: &mut DockerConfig, host_endpoint: &str) {
    let Some(container_url) = container_endpoint_for(host_endpoint) else {
        return;
    };

    if docker.environment_map().contains_key("OPENAI_BASE_URL") {
        info!(
            "Keeping the configured OPENAI_BASE_URL for the container (the detected default \
             would have been {})",
            container_url
        );
        return;
    }

    docker.environment.push(HashMap::from([(
        "OPENAI_BASE_URL".to_string(),
        container_url.clone(),
    )]));
    info!("Agent containers will reach the model at {}", container_url);
}

/// The same endpoint as seen from inside a container, or `None` when it is not on this
/// machine.
///
/// Only loopback hosts are rewritten. `host.docker.internal` is how a container names its
/// host, but it is the wrong answer for a server on another machine, which the container can
/// reach directly — and the host-side endpoint may deliberately be a different (for example
/// hosted) server than the one the agent should use. `0.0.0.0` is included because it is how
/// a server listening on every interface is usually written, and it means this machine.
fn container_endpoint_for(host_endpoint: &str) -> Option<String> {
    let url = reqwest::Url::parse(host_endpoint).ok()?;
    match url.host_str()? {
        "localhost" | "127.0.0.1" | "::1" | "[::1]" | "0.0.0.0" => {}
        _ => return None,
    }

    let host_and_port = match url.port() {
        Some(port) => format!("host.docker.internal:{port}"),
        None => "host.docker.internal".to_string(),
    };
    let path = url.path().trim_end_matches('/');
    Some(format!("http://{host_and_port}{path}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A real one-shot HTTP server on an ephemeral port, so the probe is exercised over an
    /// actual socket instead of against a mock.
    async fn serve_model_list() -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut request = [0u8; 1024];
                    let _ = socket.read(&mut request).await;
                    let body = br#"{"data":[{"id":"tiny-model"}]}"#;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(body).await;
                });
            }
        });
        (format!("http://127.0.0.1:{port}/v1"), handle)
    }

    /// A server that accepts the connection and then says nothing, so the probe has to give
    /// up on its own.
    async fn serve_silence() -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });
        (format!("http://127.0.0.1:{port}/v1"), handle)
    }

    /// A server that answers 200 with something that is not a model list — an unrelated app
    /// squatting on a well-known port.
    async fn serve_not_a_model_list() -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut request = [0u8; 1024];
                    let _ = socket.read(&mut request).await;
                    let body = br#"{"status":"ok"}"#;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(body).await;
                });
            }
        });
        (format!("http://127.0.0.1:{port}/v1"), handle)
    }

    fn config_with_environment(entries: Vec<HashMap<String, String>>) -> Config {
        let mut config = Config::default();
        config.docker.environment = entries;
        config
    }

    #[test]
    fn a_loopback_endpoint_becomes_the_container_host_with_the_same_port() {
        assert_eq!(
            container_endpoint_for("http://localhost:8080/v1").as_deref(),
            Some("http://host.docker.internal:8080/v1")
        );
        assert_eq!(
            container_endpoint_for("http://127.0.0.1:9931/v1").as_deref(),
            Some("http://host.docker.internal:9931/v1")
        );
        // A server bound to every interface is still this machine.
        assert_eq!(
            container_endpoint_for("http://0.0.0.0:8000/v1").as_deref(),
            Some("http://host.docker.internal:8000/v1")
        );
        // The path is preserved, so an endpoint without `/v1` stays without it.
        assert_eq!(
            container_endpoint_for("http://localhost:8080").as_deref(),
            Some("http://host.docker.internal:8080")
        );
        // No port in, no port out.
        assert_eq!(
            container_endpoint_for("http://localhost/v1").as_deref(),
            Some("http://host.docker.internal/v1")
        );
    }

    #[test]
    fn only_local_endpoints_are_rewritten() {
        for remote in [
            "http://192.168.1.50:8080/v1",
            "http://api.example.com/v1",
            "https://api.openai.com/v1",
            "not a url",
        ] {
            assert_eq!(
                container_endpoint_for(remote),
                None,
                "{remote} is not on this machine and must not be rewritten"
            );
        }
    }

    #[tokio::test]
    async fn probing_adopts_the_first_candidate_that_answers() {
        let (url, handle) = serve_model_list().await;
        let other = "http://127.0.0.1:1/v1"; // port 1: nothing can be listening

        assert_eq!(
            probe_endpoints(&[&url], PROBE_TIMEOUT).await.as_deref(),
            Some(url.as_str())
        );
        // A dead candidate first must not stop the live one being found.
        assert_eq!(
            probe_endpoints(&[other, &url], PROBE_TIMEOUT).await.as_deref(),
            Some(url.as_str())
        );
        // Priority order decides, even when both answer.
        assert_eq!(
            probe_endpoints(&[&url, other], PROBE_TIMEOUT).await.as_deref(),
            Some(url.as_str())
        );

        handle.abort();
    }

    #[tokio::test]
    async fn probing_gives_up_on_a_server_that_never_answers() {
        let (url, handle) = serve_silence().await;

        assert_eq!(
            probe_endpoints(&[&url], Duration::from_millis(200)).await,
            None,
            "a server that accepts but never responds must not hold up startup"
        );

        handle.abort();
    }

    /// Port 8000 in particular is full of FastAPI apps, so a `200 {"status":"ok"}` must not
    /// be mistaken for a model server and must not mask a real one further down the list.
    #[tokio::test]
    async fn probing_rejects_a_two_hundred_that_is_not_a_model_list() {
        let (squatter, squatter_handle) = serve_not_a_model_list().await;
        let (real, real_handle) = serve_model_list().await;

        assert_eq!(probe_endpoints(&[&squatter], PROBE_TIMEOUT).await, None);
        assert_eq!(
            probe_endpoints(&[&squatter, &real], PROBE_TIMEOUT).await.as_deref(),
            Some(real.as_str()),
            "a squatter must not hide the model server behind it"
        );

        squatter_handle.abort();
        real_handle.abort();
    }

    #[tokio::test]
    async fn a_detected_endpoint_is_recorded_and_given_to_the_container() {
        let (url, handle) = serve_model_list().await;
        let mut config = Config::default();

        let resolved = resolve_with(&mut config, &[&url], PROBE_TIMEOUT).await;

        assert_eq!(resolved.as_deref(), Some(url.as_str()));
        assert_eq!(config.inference_endpoint.as_deref(), Some(url.as_str()));
        // Same server, as the container sees it.
        let port = url.rsplit(':').next().unwrap().trim_end_matches("/v1");
        assert_eq!(
            config.docker.environment_map().get("OPENAI_BASE_URL"),
            Some(&format!("http://host.docker.internal:{port}/v1"))
        );

        handle.abort();
    }

    #[tokio::test]
    async fn nothing_listening_leaves_the_endpoint_unset() {
        let mut config = Config::default();

        let resolved = resolve_with(
            &mut config,
            &["http://127.0.0.1:1/v1"],
            Duration::from_millis(200),
        )
        .await;

        assert_eq!(resolved, None);
        assert_eq!(config.inference_endpoint, None);
        // The static container default is still in place, so an agent is never left without
        // an endpoint even when nothing was detected.
        assert_eq!(
            config.docker.environment_with_defaults().get("OPENAI_BASE_URL"),
            Some(&"http://host.docker.internal:8080/v1".to_string())
        );
    }

    #[tokio::test]
    async fn a_configured_endpoint_is_used_without_probing() {
        let (url, handle) = serve_model_list().await;
        let mut config = Config::default();
        config.inference_endpoint = Some("http://localhost:4321/v1".to_string());

        let resolved = resolve_with(&mut config, &[&url], PROBE_TIMEOUT).await;

        assert_eq!(resolved.as_deref(), Some("http://localhost:4321/v1"));
        assert_eq!(
            config.docker.environment_map().get("OPENAI_BASE_URL"),
            Some(&"http://host.docker.internal:4321/v1".to_string())
        );

        handle.abort();
    }

    #[tokio::test]
    async fn an_explicit_container_endpoint_is_never_overridden() {
        let mut config = config_with_environment(vec![HashMap::from([(
            "OPENAI_BASE_URL".to_string(),
            "http://host.docker.internal:9999/v1".to_string(),
        )])]);
        config.inference_endpoint = Some("http://localhost:8080/v1".to_string());

        resolve_with(&mut config, &[], PROBE_TIMEOUT).await;

        assert_eq!(
            config.docker.environment_map().get("OPENAI_BASE_URL"),
            Some(&"http://host.docker.internal:9999/v1".to_string())
        );
    }

    #[tokio::test]
    async fn a_remote_endpoint_does_not_move_the_container_endpoint() {
        let mut config = Config::default();
        config.inference_endpoint = Some("https://api.example.com/v1".to_string());

        resolve_with(&mut config, &[], PROBE_TIMEOUT).await;

        // Untouched, so the container keeps the local-server default rather than being
        // pointed at a hosted API with the agent's own credentials.
        assert_eq!(config.docker.environment_map().get("OPENAI_BASE_URL"), None);
    }
}
