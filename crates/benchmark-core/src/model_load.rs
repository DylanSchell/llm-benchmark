//! Making sure the model under test is resident *before* an exercise is timed.
//!
//! A benchmark run whose first exercise includes the time to page a few tens of gigabytes of
//! weights off disk is measuring the disk, not the model. llama-swap (the usual server here)
//! loads on demand, so whichever request arrives first pays for the swap — and that request is
//! the agent's first one, inside the measured exercise.
//!
//! [`ModelLoadGate`] closes that gap. Before each exercise it asks the endpoint what it already
//! knows:
//!
//! - `GET {endpoint}/models` — a llama-swap listing carries `status.value` per model, which is
//!   `loaded` or `unloaded`. Any other OpenAI-compatible server answers the same request without
//!   a `status` field, and then the gate has nothing to say and stays out of the way.
//! - `GET {root}/running` — llama-swap's per-upstream state, which distinguishes a model that
//!   is *up* from one that is still *starting* (`/models` reports both as `loaded`).
//!
//! When the model is not up, the gate triggers the load itself with a one-token chat
//! completion — the same request shape any server understands, and the request llama-swap
//! queues until the upstream is ready — and waits for the state to change, up to
//! `model_load_timeout_secs`. Either way the exercise then starts on a warm model and its
//! duration means what it says.
//!
//! Checks are cheap (two small GETs) and idempotent, so they run before *every* exercise rather
//! than once per run: an idle queue, a `unload_after` eviction, or another client on the same
//! server can all leave the model unloaded mid-run.

use std::sync::Arc;
use std::time::{Duration, Instant};

use benchmark_types::config::Config;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// `status.value` for a model llama-swap has an upstream process for — including one that is
/// still coming up, which is why [`running_state`] exists.
const STATUS_LOADED: &str = "loaded";

/// The `/running` state of an upstream that answers requests.
const STATE_READY: &str = "ready";

/// `meta.llamaswap.type` for a model served by another machine (a peer) through this endpoint.
/// Nothing local is loaded for it, so waiting for it to "load" would wait forever.
const KIND_PEER: &str = "peer";

/// The model name the reference agent runs under. It solves exercises with the image's own
/// reference implementation, so no model is served for it.
const REFERENCE_MODEL: &str = "reference";

/// How long to wait between two checks while waiting for a load.
///
/// A swap takes tens of seconds to minutes; this only has to be small enough that readiness is
/// noticed promptly, and large enough that a long wait is not a polling storm.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// How long one status request may hang before it counts as "the endpoint did not answer".
/// A wedged server must not turn a two-millisecond check into a stall.
const QUERY_TIMEOUT: Duration = Duration::from_secs(5);

/// What the endpoint says about one model, as far as this process is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    /// A local upstream for this model is up and answering.
    Ready,
    /// The model list claims an upstream exists but says nothing about whether it answers yet.
    /// Only [`classify`] produces it; [`ModelLoadGate::probe`] settles it to [`LoadState::Ready`]
    /// or [`LoadState::Starting`] with `/running`.
    Loaded,
    /// A local upstream exists but is still coming up.
    Starting,
    /// The model is known to the endpoint and nothing is running for it.
    NotLoaded,
    /// The model is served elsewhere (a peer), so there is nothing here to load.
    Remote,
    /// The endpoint gave no usable answer — unreachable, not llama-swap, or a model it has
    /// never heard of.
    Unknown,
}

/// What waiting did, kept so callers (and tests) can tell "nothing to do" from "gave up".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// No wait was possible or needed: no endpoint, the reference agent, a peer model, or a
    /// server that reports no load state.
    Skipped(&'static str),
    /// The model was already resident; no request was sent.
    AlreadyLoaded,
    /// The model had to be loaded and came up in the given time.
    LoadedAfter(Duration),
    /// The model did not become ready in `model_load_timeout_secs`. The exercise runs anyway —
    /// a slow or broken load is the agent's problem to report, not a reason to skip a run.
    GaveUpAfter(Duration),
}

/// Pre-exercise model-load gate. Cheap to clone; every clone shares one lock and one pool.
#[derive(Clone)]
pub struct ModelLoadGate {
    inner: Arc<Inner>,
}

struct Inner {
    /// Host-side OpenAI-compatible base URL, `/v1` included, as [`Config`] records it.
    endpoint: Option<String>,
    api_key: Option<String>,
    /// How long to wait for a load before starting the exercise regardless.
    timeout: Duration,
    /// Gap between checks. A field rather than the constant so tests do not wait in real time.
    poll: Duration,
    client: reqwest::Client,
    /// One load at a time. Two exercises asking for two different models at once would make the
    /// server evict one swap to start the other, and both exercises would then be waiting for a
    /// model that is no longer being loaded.
    load_lock: Mutex<()>,
}

impl ModelLoadGate {
    /// Builds a gate over the configured endpoint. A config with no endpoint yields a gate that
    /// does nothing, which is the same answer [`Config`] gives everywhere else.
    pub fn from_config(config: &Config) -> Self {
        Self {
            inner: Arc::new(Inner {
                endpoint: config
                    .inference_endpoint
                    .as_deref()
                    .map(|endpoint| endpoint.trim_end_matches('/').to_string()),
                api_key: config.api_key.clone(),
                timeout: Duration::from_secs(config.model_load_timeout_secs),
                poll: POLL_INTERVAL,
                client: reqwest::Client::new(),
                load_lock: Mutex::new(()),
            }),
        }
    }

    /// Only tests change this: a real swap takes tens of seconds, a test must not.
    #[cfg(test)]
    fn with_poll(mut self, poll: Duration) -> Self {
        if let Some(inner) = Arc::get_mut(&mut self.inner) {
            inner.poll = poll;
        }
        self
    }

    /// Ensure `model` is resident and answering, loading it if the endpoint says it is not.
    ///
    /// Never fails: every outcome, including giving up, lets the exercise proceed.
    pub async fn ensure_ready(&self, model: &str) -> Outcome {
        let Some(endpoint) = self.inner.endpoint.as_deref() else {
            return Outcome::Skipped("no inference endpoint is configured");
        };
        if model.trim().is_empty() || model == REFERENCE_MODEL {
            return Outcome::Skipped("the reference agent does not run a served model");
        }

        // Held for the whole wait, not just the check: see `Inner::load_lock`.
        let _load_guard = self.inner.load_lock.lock().await;

        let initial = self.probe(endpoint, model).await;
        match initial {
            LoadState::Ready => return Outcome::AlreadyLoaded,
            LoadState::Remote => {
                debug!("model '{model}' is served by a peer; nothing to load locally");
                return Outcome::Skipped("the model is served by a peer, not loaded here");
            }
            LoadState::Unknown => {
                debug!(
                    "the endpoint gave no load state for '{model}'; starting the exercise as-is"
                );
                return Outcome::Skipped("the endpoint does not report model load state");
            }
            // `Loaded` cannot come out of `probe`, which settles it; waiting on it would be the
            // safe reading if that ever changes.
            LoadState::Loaded | LoadState::Starting | LoadState::NotLoaded => {}
        }

        let started = Instant::now();
        info!(
            "model '{model}' is {initial:?} on {endpoint}; loading it before the exercise so the \
             swap is not billed to it (up to {}s)",
            self.inner.timeout.as_secs()
        );

        // The request that makes the server load the model. Detached (never aborted, even when
        // the wait ends) because cancelling it can interrupt the very load it started; its own
        // timeout bounds it, and it logs what it learned.
        tokio::spawn(self.clone().warm_up(model.to_string()));

        loop {
            let state = self.probe(endpoint, model).await;
            if state == LoadState::Ready {
                let elapsed = started.elapsed();
                info!("model '{model}' ready after {}s", elapsed.as_secs());
                return Outcome::LoadedAfter(elapsed);
            }

            let elapsed = started.elapsed();
            if elapsed >= self.inner.timeout {
                warn!(
                    "model '{model}' was still {state:?} after {}s; starting the exercise and \
                     letting the first request wait. Raise model_load_timeout_secs if this model \
                     legitimately needs longer.",
                    elapsed.as_secs()
                );
                return Outcome::GaveUpAfter(elapsed);
            }

            debug!("still waiting for model '{model}' ({state:?} after {}s)", elapsed.as_secs());
            tokio::time::sleep(self.inner.poll).await;
        }
    }

    /// Ask the endpoint about `model`: the model list first, and `/running` only when the list
    /// claims the model is loaded, since that is the one thing the list cannot settle.
    async fn probe(&self, endpoint: &str, model: &str) -> LoadState {
        let Some(models) = self.get_json(&format!("{endpoint}/models")).await else {
            return LoadState::Unknown;
        };
        match classify(&models, model) {
            LoadState::Loaded => {
                // `/running` hangs off the server root, not off the `/v1` prefix the model list
                // lives behind.
                let root = server_root(endpoint);
                let running = self.get_json(&format!("{root}/running")).await;
                match running_state(running.as_ref(), &serving_ids(&models, model)) {
                    // Only positive evidence of "still coming up" holds the exercise back; an
                    // answer we cannot map (alias, selector, matrix) leaves the list in charge.
                    Some(state) if state != STATE_READY => LoadState::Starting,
                    _ => LoadState::Ready,
                }
            }
            other => other,
        }
    }

    /// The one-token completion that triggers the load. Its status says whether the model
    /// answered; either way the wait loop, not this request, decides when the exercise starts.
    async fn warm_up(self, model: String) {
        let Some(endpoint) = self.inner.endpoint.as_deref() else {
            return;
        };
        let url = format!("{endpoint}/chat/completions");
        let body = serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": "ping" }],
            // One token: the point is the load, not the answer.
            "max_tokens": 1,
            "stream": false,
        });

        let mut request = self
            .inner
            .client
            .post(&url)
            .timeout(self.inner.timeout)
            .json(&body);
        if let Some(api_key) = self.inner.api_key.as_deref().filter(|key| !key.is_empty()) {
            request = request.bearer_auth(api_key);
        }

        match request.send().await {
            Ok(response) if response.status().is_success() => {
                debug!("warm-up request for '{model}' completed the load");
            }
            // A rejected request is not a failed load: the server still swapped the model in
            // before deciding it could not answer, which is why the wait loop keeps polling.
            Ok(response) => debug!(
                "warm-up request for '{model}' was refused with {}; still waiting for the model \
                 to report ready",
                response.status().as_u16()
            ),
            Err(e) => debug!("warm-up request for '{model}' did not complete: {e}"),
        }
    }

    async fn get_json(&self, url: &str) -> Option<Value> {
        let mut request = self.inner.client.get(url).timeout(QUERY_TIMEOUT);
        if let Some(api_key) = self.inner.api_key.as_deref().filter(|key| !key.is_empty()) {
            request = request.bearer_auth(api_key);
        }

        match request.send().await {
            Ok(response) if response.status().is_success() => response.json().await.ok(),
            Ok(response) => {
                debug!("{url} answered {}", response.status().as_u16());
                None
            }
            Err(e) => {
                debug!("{url} did not answer: {e}");
                None
            }
        }
    }
}

/// `status.value` for `model` in a `GET /models` body.
///
/// Every judgement here is "what did the server actually say": a model that is absent, or a
/// listing without `status`, is [`LoadState::Unknown`] rather than a guess that a load is
/// needed — waiting for a model the endpoint has never heard of would wait for nothing.
fn classify(models: &Value, model: &str) -> LoadState {
    let Some(records) = models.get("data").and_then(Value::as_array) else {
        return LoadState::Unknown;
    };
    let Some(record) = records
        .iter()
        .find(|record| record.get("id").and_then(Value::as_str) == Some(model))
    else {
        return LoadState::Unknown;
    };
    if nested_str(record, &["meta", "llamaswap", "type"]) == Some(KIND_PEER) {
        return LoadState::Remote;
    }
    match nested_str(record, &["status", "value"]) {
        Some(STATUS_LOADED) => LoadState::Loaded,
        Some(_) => LoadState::NotLoaded,
        None => LoadState::Unknown,
    }
}

/// The ids `/running` might use for `model`: its own id, plus — for an alias or a selector — the
/// ids of the models that actually serve it, since `/running` names the upstream, not the alias
/// the request went in under.
fn serving_ids(models: &Value, model: &str) -> Vec<String> {
    let mut ids = vec![model.to_string()];
    let Some(records) = models.get("data").and_then(Value::as_array) else {
        return ids;
    };
    let Some(record) = records
        .iter()
        .find(|record| record.get("id").and_then(Value::as_str) == Some(model))
    else {
        return ids;
    };

    if let Some(target) = nested_str(record, &["meta", "llamaswap", "modelID"]) {
        ids.push(target.to_string());
    }
    if let Some(targets) = record
        .get("meta")
        .and_then(|meta| meta.get("llamaswap"))
        .and_then(|llamaswap| llamaswap.get("targets"))
        .and_then(Value::as_array)
    {
        ids.extend(
            targets
                .iter()
                .filter_map(Value::as_str)
                .map(|target| target.to_string()),
        );
    }
    ids
}

/// The `state` `/running` reports for any of `ids`, or `None` when it lists none of them —
/// which means the mapping is not ours to read, not that the model is down.
fn running_state(running: Option<&Value>, ids: &[String]) -> Option<String> {
    let entries = running?.get("running").and_then(Value::as_array)?;
    entries
        .iter()
        .find(|entry| {
            entry
                .get("model")
                .and_then(Value::as_str)
                .is_some_and(|name| ids.iter().any(|id| id == name))
        })
        .and_then(|entry| entry.get("state").and_then(Value::as_str))
        .map(str::to_string)
}

/// `{scheme}://host[:port]` for an endpoint that may carry a path prefix, which is where
/// llama-swap serves its non-OpenAI routes (`/running`).
fn server_root(endpoint: &str) -> String {
    match reqwest::Url::parse(endpoint) {
        Ok(url) => format!(
            "{}://{}{}",
            url.scheme(),
            url.host_str().unwrap_or_default(),
            url.port().map(|port| format!(":{port}")).unwrap_or_default()
        ),
        // Not a URL we can take apart: the endpoint itself is the best guess there is.
        Err(_) => endpoint.to_string(),
    }
}

/// Follows a path of object keys to a string value.
fn nested_str<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::task::JoinHandle;

    const MODEL: &str = "clef";

    /// A `/models` record in the shape llama-swap writes: `meta.llamaswap` says what kind of
    /// entry it is and `status.value` says whether it is resident. `extra_meta` adds the fields
    /// only some kinds carry (`modelID` for an alias, `targets` for a selector).
    fn record(id: &str, meta: &str, status: Option<&str>) -> String {
        let status = match status {
            Some(status) => format!(",\"status\":{{\"value\":\"{status}\"}}"),
            None => String::new(),
        };
        format!(
            "{{\"id\":\"{id}\",\"object\":\"model\",\"owned_by\":\"llama-swap\",\
             \"meta\":{{\"llamaswap\":{{{meta}}}}}{status}}}"
        )
    }

    fn listing(records: &[String]) -> Value {
        serde_json::from_str(&format!(
            "{{\"data\":[{}],\"object\":\"list\"}}",
            records.join(",")
        ))
        .unwrap()
    }

    fn running(entries: &[&str]) -> Value {
        serde_json::from_str(&format!(
            "{{\"running\":[{}]}}",
            entries
                .iter()
                .map(|state| format!("{{\"model\":\"{MODEL}\",\"state\":\"{state}\"}}"))
                .collect::<Vec<_>>()
                .join(",")
        ))
        .unwrap()
    }

    // ── reading the endpoint ─────────────────────────────────────────────

    #[test]
    fn the_model_list_decides_what_kind_of_wait_this_is() {
        let loaded = listing(&[record(MODEL, r#""type":"model""#, Some("loaded"))]);
        let unloaded = listing(&[record(MODEL, r#""type":"model""#, Some("unloaded"))]);
        assert_eq!(classify(&loaded, MODEL), LoadState::Loaded);
        assert_eq!(classify(&unloaded, MODEL), LoadState::NotLoaded);

        // A model the endpoint has never heard of must not start a wait that can only end in a
        // timeout — the typo (or the stale config) is reported by the agent's own request.
        assert_eq!(classify(&loaded, "clef-typo"), LoadState::Unknown);
        assert_eq!(classify(&serde_json::json!({"object": "list"}), MODEL), LoadState::Unknown);

        // A peer is another machine's model: nothing here loads, so nothing here waits.
        let peer = listing(&[record(
            "anthropic/claude-sonnet-5",
            r#""peerID":"anthropic","type":"peer""#,
            Some("unloaded"),
        )]);
        assert_eq!(
            classify(&peer, "anthropic/claude-sonnet-5"),
            LoadState::Remote,
            "a peer is never 'loaded', and waiting for one would wait forever"
        );

        // Plain llama-server, vLLM and Ollama answer the same request without ever saying what
        // is resident; the gate has no business guessing.
        let plain = listing(&[record(MODEL, "", None)]);
        assert_eq!(classify(&plain, MODEL), LoadState::Unknown);
    }

    /// `/models` says `loaded` for a model that is still starting, and it names the model while
    /// `/running` names the upstream — which for an alias or a selector is a different id.
    #[test]
    fn readiness_is_judged_on_the_upstream_that_serves_the_name() {
        let alias = listing(&[
            record(MODEL, r#""type":"model""#, Some("loaded")),
            record(
                "clef-fast",
                &format!(r#""modelID":"{MODEL}","type":"alias""#),
                Some("loaded"),
            ),
        ]);
        assert_eq!(serving_ids(&alias, "clef-fast"), vec!["clef-fast", MODEL]);

        // Asked about the alias, `/running` still reports the real model — so that is the entry
        // readiness has to be read from.
        assert_eq!(
            running_state(Some(&running(&["ready"])), &serving_ids(&alias, "clef-fast")).as_deref(),
            Some("ready")
        );
        assert_eq!(
            running_state(Some(&running(&["starting"])), &serving_ids(&alias, "clef-fast"))
                .as_deref(),
            Some("starting")
        );

        // `/running` not naming the model is not evidence of anything: the matrix, a selector on
        // another host, or an older server can all answer without it. The list stands.
        assert_eq!(running_state(Some(&running(&[])), &[MODEL.to_string()]), None);
        assert_eq!(running_state(None, &[MODEL.to_string()]), None);
    }

    /// llama-swap serves `/running` at the server root, while the model list the config names
    /// lives behind `/v1`.
    #[test]
    fn the_running_route_hangs_off_the_server_root() {
        assert_eq!(server_root("http://localhost:8080/v1"), "http://localhost:8080");
        assert_eq!(
            server_root("http://127.0.0.1:9931/v1/"),
            "http://127.0.0.1:9931"
        );
        assert_eq!(server_root("https://swap.example.com/v1"), "https://swap.example.com");
        // Something the URL parser cannot read is used as-is rather than mangled.
        assert_eq!(server_root("not a url"), "not a url");
    }

    // ── the gate, against a fake llama-swap ──────────────────────────────

    #[derive(Clone, Copy, Debug)]
    enum Scenario {
        /// The model is loaded and answering before the first question.
        Ready,
        /// `unloaded` for this many model listings, then loaded and answering.
        LoadsAfter(usize),
        /// Loaded and answering, but `/running` says `starting` until this many listings.
        StartingUntil(usize),
        /// Nothing ever comes up.
        Never,
        /// An OpenAI-compatible server that reports no load state at all.
        NoStatus,
        /// The name is served by a peer.
        Peer,
    }

    struct FakeSwap {
        base: String,
        seen: Arc<Mutex<Vec<String>>>,
        handle: JoinHandle<()>,
    }

    impl FakeSwap {
        async fn start(scenario: Scenario) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let seen = Arc::new(Mutex::new(Vec::new()));
            let listings = Arc::new(AtomicUsize::new(0));

            let handle = tokio::spawn({
                let seen = Arc::clone(&seen);
                async move {
                    while let Ok((socket, _)) = listener.accept().await {
                        tokio::spawn({
                            let seen = Arc::clone(&seen);
                            let listings = Arc::clone(&listings);
                            async move {
                                let _ = serve_one_request(socket, scenario, listings, seen).await;
                            }
                        });
                    }
                }
            });

            Self {
                base: format!("http://127.0.0.1:{port}/v1"),
                seen,
                handle,
            }
        }

        async fn requests(&self) -> Vec<String> {
            self.seen.lock().await.clone()
        }

        async fn stop(self) {
            self.handle.abort();
        }
    }

    /// One request per connection, closed after answering — enough for a poll, and it keeps the
    /// fake free of HTTP framing.
    async fn serve_one_request(
        mut socket: TcpStream,
        scenario: Scenario,
        listings: Arc<AtomicUsize>,
        seen: Arc<Mutex<Vec<String>>>,
    ) -> std::io::Result<()> {
        let mut buffer: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 512];
        let head_end = loop {
            if let Some(at) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                break at + 4;
            }
            let read = socket.read(&mut chunk).await?;
            if read == 0 {
                return Ok(());
            }
            buffer.extend_from_slice(&chunk[..read]);
        };

        let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
        let mut request_line = head.lines().next().unwrap_or_default().split_whitespace();
        let (Some(method), Some(path)) = (request_line.next(), request_line.next()) else {
            return Ok(());
        };

        // Read the body off the wire so the client is never left mid-write.
        let content_length = head
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        while buffer.len() < head_end + content_length {
            let read = socket.read(&mut chunk).await?;
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
        }

        seen.lock().await.push(format!("{method} {path}"));

        let (status, body) = match (method, path) {
            ("GET", "/v1/models") => (200, models_body(scenario, listings.fetch_add(1, Ordering::SeqCst) + 1)),
            ("GET", "/running") => (200, running_body(scenario, listings.load(Ordering::SeqCst))),
            ("POST", "/v1/chat/completions") => (200, r#"{"id":"chatcmpl-1","choices":[]}"#.to_string()),
            _ => (404, r#"{"error":"not found"}"#.to_string()),
        };
        reply(&mut socket, status, &body).await
    }

    fn models_body(scenario: Scenario, listings: usize) -> String {
        let record = match scenario {
            Scenario::Ready => record(MODEL, r#""type":"model""#, Some("loaded")),
            Scenario::LoadsAfter(after) => record(
                MODEL,
                r#""type":"model""#,
                Some(if listings <= after { "unloaded" } else { "loaded" }),
            ),
            Scenario::StartingUntil(_) => record(MODEL, r#""type":"model""#, Some("loaded")),
            Scenario::Never => record(MODEL, r#""type":"model""#, Some("unloaded")),
            Scenario::NoStatus => record(MODEL, r#""type":"model""#, None),
            Scenario::Peer => record(
                MODEL,
                r#""peerID":"anthropic","type":"peer""#,
                Some("unloaded"),
            ),
        };
        format!(r#"{{"data":[{record}],"object":"list"}}"#)
    }

    fn running_body(scenario: Scenario, listings: usize) -> String {
        let state = match scenario {
            Scenario::StartingUntil(untill) if listings <= untill => "starting",
            Scenario::Ready | Scenario::LoadsAfter(_) | Scenario::StartingUntil(_) => "ready",
            // Never loaded, and the two scenarios that never ask.
            Scenario::Never | Scenario::NoStatus | Scenario::Peer => "",
        };
        if state.is_empty() {
            return r#"{"running":[]}"#.to_string();
        }
        format!(r#"{{"running":[{{"model":"{MODEL}","state":"{state}"}}]}}"#)
    }

    async fn reply(socket: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
        let head = format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(head.as_bytes()).await?;
        socket.write_all(body.as_bytes()).await
    }

    /// A gate that waits like a test, not like a 200 GB model.
    fn gate_over(base: &str, timeout: Duration) -> ModelLoadGate {
        let config = Config {
            inference_endpoint: Some(base.to_string()),
            api_key: Some("test-key".to_string()),
            model_load_timeout_secs: timeout.as_secs().max(1),
            ..Default::default()
        };
        ModelLoadGate::from_config(&config).with_poll(Duration::from_millis(20))
    }

    async fn warm_requests(server: &FakeSwap) -> usize {
        server
            .requests()
            .await
            .iter()
            .filter(|request| request.starts_with("POST "))
            .count()
    }

    #[tokio::test]
    async fn a_warm_model_costs_one_status_request_and_nothing_else() {
        let server = FakeSwap::start(Scenario::Ready).await;

        let outcome = gate_over(&server.base, Duration::from_secs(5))
            .ensure_ready(MODEL)
            .await;

        assert_eq!(outcome, Outcome::AlreadyLoaded);
        let requests = server.requests().await;
        assert!(
            requests.iter().any(|r| r == "GET /v1/models"),
            "the check itself is the whole cost, got: {requests:?}"
        );
        assert_eq!(warm_requests(&server).await, 0, "a loaded model needs no request");

        server.stop().await;
    }

    /// The point of the whole module: the swap happens here, before the exercise is timed.
    #[tokio::test]
    async fn an_unloaded_model_is_loaded_before_the_exercise_starts() {
        let server = FakeSwap::start(Scenario::LoadsAfter(1)).await;

        let outcome = gate_over(&server.base, Duration::from_secs(5))
            .ensure_ready(MODEL)
            .await;

        assert!(
            matches!(outcome, Outcome::LoadedAfter(_)),
            "expected a completed load, got {outcome:?}"
        );
        assert_eq!(warm_requests(&server).await, 1, "one trigger request is enough");

        server.stop().await;
    }

    /// `loaded` on `/models` only means a process exists; a swap already under way is not up
    /// until `/running` says so.
    #[tokio::test]
    async fn a_model_that_is_still_starting_is_waited_for() {
        let server = FakeSwap::start(Scenario::StartingUntil(2)).await;

        let outcome = gate_over(&server.base, Duration::from_secs(5))
            .ensure_ready(MODEL)
            .await;

        assert!(
            matches!(outcome, Outcome::LoadedAfter(_)),
            "a starting model must not be handed to the agent as ready, got {outcome:?}"
        );

        server.stop().await;
    }

    /// A model that never arrives is a broken benchmark setup, but not a reason to abandon the
    /// run: the exercise goes ahead and its own failure says what happened.
    #[tokio::test]
    async fn waiting_gives_up_rather_than_hanging_the_run() {
        let server = FakeSwap::start(Scenario::Never).await;

        let outcome = gate_over(&server.base, Duration::from_secs(1))
            .ensure_ready(MODEL)
            .await;

        assert!(
            matches!(outcome, Outcome::GaveUpAfter(_)),
            "a load that never finishes must end in a warning, not a stall, got {outcome:?}"
        );

        server.stop().await;
    }

    #[tokio::test]
    async fn a_server_that_reports_no_load_state_is_left_alone() {
        for (scenario, expected) in [
            (Scenario::NoStatus, "does not report"),
            (Scenario::Peer, "peer"),
        ] {
            let server = FakeSwap::start(scenario).await;

            let outcome = gate_over(&server.base, Duration::from_secs(5))
                .ensure_ready(MODEL)
                .await;

            let Outcome::Skipped(reason) = outcome else {
                panic!("{scenario:?} must be skipped, got {outcome:?}");
            };
            assert!(
                reason.contains(expected),
                "expected a skip because of {expected}, got: {reason}"
            );
            assert_eq!(warm_requests(&server).await, 0);

            server.stop().await;
        }
    }

    #[tokio::test]
    async fn nothing_to_ask_about_is_answered_without_a_request() {
        // The reference agent solves exercises with the image's own implementation; the model
        // column says "reference" and no server serves it.
        let dead = "http://127.0.0.1:1/v1";
        let gate = gate_over(dead, Duration::from_secs(5));

        assert!(matches!(
            gate.ensure_ready("reference").await,
            Outcome::Skipped(_)
        ));
        assert!(matches!(
            gate.ensure_ready("   ").await,
            Outcome::Skipped(_)
        ));
        // An endpoint that is not configured at all is the same answer as one that is dead: the
        // gate cannot know whether a model needs loading.
        let no_endpoint = ModelLoadGate::from_config(&Config::default());
        assert!(matches!(
            no_endpoint.ensure_ready(MODEL).await,
            Outcome::Skipped(_)
        ));
        assert!(matches!(
            gate.ensure_ready(MODEL).await,
            Outcome::Skipped(_)
        ));
    }
    /// Read-only check against a real llama-swap, which is the only judge of the payload shapes
    /// this module bets on. Opt-in because it needs a running server, and deliberately incapable
    /// of triggering a load: it asks, it does not wait or warm anything up, so it cannot evict
    /// whatever is resident.
    ///
    /// ```bash
    /// LLM_BENCHMARK_LIVE_ENDPOINT=http://localhost:8080/v1 \
    ///   cargo test -p benchmark-core --lib model_load -- --ignored
    /// ```
    #[tokio::test]
    #[ignore = "requires a running llama-swap; set LLM_BENCHMARK_LIVE_ENDPOINT to opt in"]
    async fn reads_a_real_llama_swap() {
        let Ok(endpoint) = std::env::var("LLM_BENCHMARK_LIVE_ENDPOINT") else {
            panic!("set LLM_BENCHMARK_LIVE_ENDPOINT to the endpoint to interrogate");
        };
        let config = Config {
            inference_endpoint: Some(endpoint.clone()),
            api_key: std::env::var("LLM_BENCHMARK_LIVE_API_KEY").ok(),
            ..Default::default()
        };
        let gate = ModelLoadGate::from_config(&config);
        let models = gate
            .get_json(&format!("{endpoint}/models"))
            .await
            .expect("the endpoint answered /models");

        let resident: Vec<&str> = models["data"]
            .as_array()
            .expect("a llama-swap listing carries a data array")
            .iter()
            .filter(|record| nested_str(record, &["status", "value"]) == Some(STATUS_LOADED))
            .filter_map(|record| record.get("id").and_then(Value::as_str))
            .collect();
        assert!(
            !resident.is_empty(),
            "nothing claims to be loaded; nothing to check readiness against"
        );

        for id in &resident {
            assert_eq!(
                gate.probe(&endpoint, id).await,
                LoadState::Ready,
                "{id} is reported loaded and must read as ready"
            );
        }

        // Something the server lists but is not running: the state that must lead to a load.
        let idle = models["data"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|record| record.get("id").and_then(Value::as_str))
            .find(|id| !resident.contains(id) && !endpoint_is_peer(&models, id))
            .expect("at least one listed model is not loaded");
        assert_eq!(gate.probe(&endpoint, idle).await, LoadState::NotLoaded);

        println!("loaded: {resident:?}, idle: {idle}");
    }

    fn endpoint_is_peer(models: &Value, id: &str) -> bool {
        classify(models, id) == LoadState::Remote
    }

    /// The same, but this one *loads* something, so it needs a second environment variable to
    /// say which model may be brought up and is free to take minutes doing it.
    ///
    /// ```bash
    /// LLM_BENCHMARK_LIVE_ENDPOINT=http://localhost:8080/v1 \
    /// LLM_BENCHMARK_LIVE_LOAD_MODEL=clef \
    ///   cargo test -p benchmark-core --lib model_load -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore = "loads a model on the developer's server; opt in with LLM_BENCHMARK_LIVE_LOAD_MODEL"]
    async fn loads_a_real_model_before_an_exercise() {
        let Ok(endpoint) = std::env::var("LLM_BENCHMARK_LIVE_ENDPOINT") else {
            panic!("set LLM_BENCHMARK_LIVE_ENDPOINT to the endpoint to interrogate");
        };
        let model = std::env::var("LLM_BENCHMARK_LIVE_LOAD_MODEL")
            .expect("set LLM_BENCHMARK_LIVE_LOAD_MODEL to a model that may be loaded");
        let config = Config {
            inference_endpoint: Some(endpoint.clone()),
            api_key: std::env::var("LLM_BENCHMARK_LIVE_API_KEY").ok(),
            ..Default::default()
        };

        // Show what the gate is doing while it waits, so a slow load is legible.
        let _ = tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).try_init();

        let gate = ModelLoadGate::from_config(&config);
        let outcome = gate.ensure_ready(&model).await;
        println!("ensure_ready({model}) over {endpoint} -> {outcome:?}");

        assert!(
            matches!(
                outcome,
                Outcome::AlreadyLoaded | Outcome::LoadedAfter(_)
            ),
            "{model} must end up resident, got {outcome:?}"
        );
        assert_eq!(
            gate.probe(&endpoint, &model).await,
            LoadState::Ready,
            "whatever the wait decided, the model must read as ready afterwards"
        );
    }
}
