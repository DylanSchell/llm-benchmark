# Web API

The HTTP surface of `llm-benchmark web`. Everything here is derived from the router in
`benchmark-web/src/routes/` — the `.route(…)` calls are the source of truth, and the route
index below is a 1:1 mirror of them. If the two ever disagree, the code wins and this file
is the bug.

## Running the server

```bash
llm-benchmark web                      # 0.0.0.0:8081
llm-benchmark web --port 8088          # explicit port
```

Port precedence: `--port` → `SERVER_PORT` → `server.port` in `config.yaml` → `8081`. The
server binds `0.0.0.0` and there is **no authentication**: anyone who can reach the port can
schedule runs and read results. Bind it to a trusted network, or put it behind a proxy that
authenticates.

## Two kinds of route

| Kind | Path shape | Returns |
|---|---|---|
| Pages | `/`, `/run`, `/results`, `/scoring`, `/compare`, `/benchmark/{id}` | Server-rendered HTML (`templates/*.tera`, embedded in the binary) |
| JSON APIs | `/api/…`, plus `/results/api/…` | `application/json` |
| HTML fragments | `/recent-results-fragment`, `/results/table-fragment` | JSON bodies rendered into the page by HTMX |

The dashboard is HTMX-driven: the fragments above return JSON that the page inserts, so they
are documented here alongside the `/api/` routes rather than being treated as private.

## Route index

Every route, in router order.

### Pages

| Method | Path | Handler | Purpose |
|---|---|---|---|
| GET | `/` | `dashboard` | Dashboard: recent results, active runs |
| GET | `/run` | `run_form` | Form to schedule benchmark runs |
| GET | `/benchmark/{id}` | `view_benchmark` | Live view of one session, including its SSE stream |
| GET | `/test` | `test_page` | Test/scratch page |
| GET | `/results` | `results_page` | Results browser |
| GET | `/scoring` | `scoring_dashboard` | Model scoring dashboard |
| GET | `/compare` | `compare_page` | Compare two agent–model combinations (`?a=&b=&metric=`) |
| GET | `/exercise-detail` | `exercise_detail` | Exercise detail (legacy) |
| GET | `/results/{agent}/{dir}/{lang}/{ex}` | `result_detail_page` | One result, full detail |
| GET | `/results/{agent}/{dir}/{lang}/{ex}/trace` | `result_detail_trace` | The agent trace for one result |

`{dir}` is the result directory (`{agent}-{model}`, e.g. `pi-qwen3-coder`), kept as a separate
path segment from `{agent}` so the URL stays RESTful while the on-disk layout stays flat.

### Benchmark sessions

| Method | Path | Handler | Returns |
|---|---|---|---|
| GET | `/api/benchmark/{id}/status` | `get_status` | `StatusResponse` |
| GET | `/api/benchmark/{id}/stream` | `stream_output` | Server-sent events (below) |
| POST | `/api/benchmark/{id}/cancel` | `cancel_benchmark` | `CancelResponse` |
| GET | `/api/active-runs` | `get_active_runs` | `{"count": 2}` |
| GET | `/api/active-sessions` | `get_active_sessions` | `{"sessions": […]}` |
| GET | `/api/models` | `fetch_models_endpoint` | `["gpt-oss-20b", …]` |
| GET | `/api/dashboard/completeness` | `get_completeness` | `{"total_exercises": 225, "complete_keys": […]}` |

### Queue

| Method | Path | Handler | Purpose |
|---|---|---|---|
| GET | `/api/benchmark/queue` | `get_queue` | `QueueResponse`: items plus pending/running/completed/failed/cancelled counts, `active_workers`, `parallelism_limit` |
| POST | `/api/benchmark/queue/schedule` | `schedule_batch` | Schedule a batch; body is a form **or** JSON (below) |
| POST | `/api/benchmark/queue/cancel/{id}` | `cancel_queue_item` | Cancel one pending/running item |
| POST | `/api/benchmark/queue/clear` | `clear_pending_queue` | Drop pending items |
| POST | `/api/benchmark/queue/clear-terminal` | `clear_completed_and_cancelled` | Drop finished items |
| POST | `/api/benchmark/queue/retry/{id}` | `retry_queue_item` | Re-queue a failed item |

### Exercises

| Method | Path | Handler | Returns |
|---|---|---|---|
| GET | `/api/exercises` | `get_exercises` | `{"java": ["series", …], "python": […], …}` |
| GET | `/api/languages` | `get_languages` | `["cpp", "go", "java", …]` |
| GET | `/api/exercises/{language}` | `get_exercises_for_language` | `["series", …]` |

These read the exercise set embedded in the binary, not the filesystem.

### Results

| Method | Path | Handler | Returns |
|---|---|---|---|
| POST | `/api/results/refresh` | `refresh` | `{"message": …, "loaded": 13721}` — re-scans the results directory |
| GET | `/api/results/loading-status` | `get_loading_status` | `{"loaded": true, "result_count": 13721}` |
| GET | `/api/individual-results` | `get_individual_results` | `{"results": […], "total": N}` |
| GET | `/recent-results-fragment` | `recent_results_fragment` | `{"results": […]}` |
| GET | `/results/api/results` | `get_results_api` | `ResultsTable`: `{"results": […], "total": N}` |
| GET | `/results/table-fragment` | `table_fragment` | Same shape, for HTMX |
| GET | `/results/api/stats` | `get_stats` | `Statistics` (below) |
| GET | `/results/api/{agent}/{lang}/{ex}` | `get_results_api_agent_lang_ex` | `[{…}]` |
| GET | `/results/{lang}/{ex}` | `get_results_by_lang_ex` | `[{…}]` |

`/results/api/results`, `/results/table-fragment` and `/api/individual-results` accept
`language`, `agent`, `model` and `exercise` filters; the table endpoints also accept
`quick_only`.

### Scoring

| Method | Path | Handler | Returns |
|---|---|---|---|
| GET | `/api/scored-results` | `get_scored_results` | `{"results": […], "total": N, "filters": {…}}` |
| GET | `/api/model-scores` | `get_model_scores` | `{"scores": […], "filters": {…}}` |

Both accept `language`, `agent` and `quick` filters.

### Metrics

| Method | Path | Handler | Returns |
|---|---|---|---|
| GET | `/metrics` | inline closure | Prometheus text format (`MetricsEngine::render()`) |

## `GET /api/benchmark/{id}/status`

```json
{
  "id": "1f0c…",
  "status": "RUNNING",
  "agent": "pi",
  "language": "java",
  "exercise": "series",
  "progress": 12.5,
  "completed_exercises": 3,
  "total_exercises": 24,
  "error_message": "…"
}
```

Both optional fields behave differently, and only `error_message` uses `skip_serializing_if`:
it is **absent** from the JSON unless the run failed, while `exercise` is always present and is
`null` when the session has no single exercise (an all-exercises run). `progress` is
`completed_exercises` as a percentage of `total_exercises`.

An unknown `{id}` returns **HTTP 200** with `{"error": "Session not found"}` — `get_status`
does not set a status code. Check for the `error` key rather than the HTTP status.

## `GET /api/benchmark/{id}/stream` — server-sent events

One stream per session, `data:` payloads are JSON. Events:

| `event:` | When | Payload |
|---|---|---|
| `session` | Once, immediately | Session metadata, so the page can render before the first output |
| `message` | Per line of agent output | The output line |
| `complete` | Once, at the end | Final session state |
| `error` | Instead of the above, when the session is unknown | `{"message": "Session not found"}` |

Each request takes a fresh subscriber from the session's broadcast channel, so several
browsers can watch the same run.

## `POST /api/benchmark/queue/schedule`

The body may be an HTML form (`application/x-www-form-urlencoded`) or JSON — the handler uses
a form extractor that accepts both, including a repeated `languages` field.

| Field | Type | Notes |
|---|---|---|
| `agent` | string | `reference`, `pi` or `claude` |
| `languages` | string or array | Repeated in a form, an array in JSON. Empty means every language |
| `model` | string | Ignored for `reference`, which always records itself as `reference` |
| `thinking_level` | string, optional | pi only: `off`, `minimal`, `low`, `medium`, `high`, `xhigh` |
| `exercise` | string, optional | Which exercise; meaning depends on `mode` |
| `mode` | string, optional | `single` (default), `all`, or `quick` |
| `retry` | bool, optional | Re-run exercises that already have a result |

```json
{"status": "scheduled", "count": 3, "items": [{"id": "…", "status": "PENDING", …}]}
```

## `POST /api/benchmark/{id}/cancel`

```json
{"status": "cancelled", "message": "Benchmark cancelled"}
```

## `/results/api/stats`

```json
{
  "total_runs": 12,
  "total_exercises": 2400,
  "successful_exercises": 2380,
  "success_rate": 99.17,
  "success_rate_formatted": "99.2%",
  "total_duration": 43200.0,
  "total_duration_formatted": "12h 0m 0s",
  "wall_clock_duration": 40000.0,
  "wall_clock_duration_formatted": "11h 6m 40s",
  "total_results": 2400,
  "successful_results": 2380,
  "language_stats": [{"…": "StatItem"}],
  "agent_stats": [{"…": "StatItem"}],
  "model_stats": [{"…": "StatItem"}]
}
```

## Conventions

- **No authentication, no CORS layer, no rate limiting, no API versioning.** The routes are
  exactly as listed; there is no `/api/v1`. This is a single-user dashboard, and the docs
  should not imply otherwise.
- **Errors are usually HTTP 200 with an error object in the body** (`{"error": …}`), because
  most handlers return `Json(…)` without setting a status. The exceptions are
  `result.rs`, which uses `404 NOT_FOUND`, and `queue.rs`, which uses `400 BAD_REQUEST`.
- **Unknown routes** fall through to the static-asset handler, so a typo returns the embedded
  stylesheet's `404`, not JSON.
- **Result payloads are `HashMap<String, String>`-shaped** in several endpoints (the columns
  the results table needs) rather than typed structs, so field sets follow the table's
  columns rather than a schema. `docs/RESULT_FORMAT.md` covers the underlying result files.

## Keeping this current

The route index is a mirror of these calls:

```bash
grep -n '\.route(\|\.nest(' benchmark-web/src/routes/*.rs
```

Response shapes are defined by the `Serialize` structs next to each handler in
`benchmark-web/src/routes/`, and by `benchmark-web/src/services/result_service.rs` for the
result-derived ones. The `docs/` set is checked by hand, not by CI.
