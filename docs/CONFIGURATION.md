# Configuration Reference

The `config.yaml` reference for the Rust implementation. The structs in
`crates/benchmark-types/src/config/mod.rs` are the source of truth: every field there is
listed here with its real default, and any field that is accepted but not yet read is marked
as such rather than described as if it worked.

Nothing in this file is required. With no `config.yaml` at all, every setting falls back to a
default and a local model server is found automatically — see
[Endpoint resolution](#inference_endpoint-and-api_key).

## How configuration is resolved

Precedence, lowest to highest:

| Order | Source | Notes |
|---|---|---|
| 1 | Built-in defaults | Every field has one; see the tables below |
| 2 | `config.yaml` | May be partial — unset keys keep their default |
| 3 | Environment variables | `CONFIG_PATH`, `SERVER_PORT`, `PARALLELISM`, `RESULTS_DIR` |
| 4 | Command-line flags | `llm-benchmark run --config … --model …` |

For `llm-benchmark web` layers 3 and 4 are *the same mechanism*: `--port`, `--results-dir`,
`--parallelism` and `--config` are implemented by setting `SERVER_PORT`, `RESULTS_DIR`,
`PARALLELISM` and `CONFIG_PATH` before the web crate runs (`src/web.rs`). A flag therefore has
exactly the same effect as the environment variable, and the environment variable wins over
`config.yaml`.

### The file itself

The path is `--config`, else `CONFIG_PATH`, else `config.yaml`, resolved relative to the
current working directory (not the binary's location).

| Situation | `llm-benchmark run` | `llm-benchmark web` |
|---|---|---|
| File absent | Warning, built-in defaults | Warning, built-in defaults |
| File present but unparseable | **Error**, exits 1 | Warning, built-in defaults |
| Unknown key (a typo) | **Silently ignored** | **Silently ignored** |

Two consequences worth knowing:

- A malformed file fails loudly on `run` but not on `web`; on `web` a typo in the YAML is
  reported as `Could not parse config …` and then ignored.
- **Unknown keys are not rejected.** There is no `deny_unknown_fields`, so
  `parallelsim: 4` (misspelled) parses cleanly and you get `parallelism: 1`. Nothing warns.
  If a setting appears to have no effect, check the spelling first.
- **`${VAR}` is not substituted.** The file is plain YAML: `api_key: "${MY_KEY}"` sets the
  literal string `${MY_KEY}`. Use the environment variables in the table below instead.

## Complete example

`config.example.yaml` in the repository root is a complete, commented starting point:

```yaml
# LLM Benchmark Configuration
# Copy to config.yaml and customize for your setup.
#
# Every setting is optional: with no config.yaml at all the defaults are used, and a model
# server on this machine is found automatically (the local ports 8000, 8080 and 9931 are
# probed at startup). This file is a starting point, not a requirement.

# Exercises are embedded in the binary; no external checkout is required.
parallelism: 1

# Inference endpoint configuration (OpenAI-compatible API), used for the dashboard's model
# list (GET {inference_endpoint}/models).
# Leave inference_endpoint commented out to probe http://localhost:{8000,8080,9931}/v1 and
# adopt the first that returns a model list. Setting it disables probing entirely.
# inference_endpoint: "http://localhost:8080/v1"
# Bearer token sent to inference_endpoint, if your server checks it.
api_key: ""

# Docker configuration
docker:
  # Name of the custom Docker image with build tools
  image: "ghcr.io/dylanschell/llm-benchmark-runner:latest"
  # Container working directory
  work_dir: "/workspace"
  # Timeout for exercise execution in seconds
  timeout: 3600
  # Timeout for pulling the runner image when it is not present locally (seconds).
  # Deliberately separate from `timeout`: acquiring the image is setup, not benchmark work.
  pull_timeout: 1800
  # Maximum time allowed for any single Bash tool call inside the container (seconds).
  per_command_timeout: 120
  # Memory limit for container (e.g., "2g")
  memory: "2g"
  # Environment variables passed into the runner container — this is what the agent CLI
  # INSIDE the container uses. A local model server normally needs none of them: when
  # inference_endpoint points at localhost, OPENAI_BASE_URL is derived from it (same port on
  # host.docker.internal); otherwise it defaults to http://host.docker.internal:8080/v1.
  # Uncomment to override, e.g. for a hosted provider.
  # environment:
  #   - ANTHROPIC_AUTH_TOKEN: ""
  #   - ANTHROPIC_BASE_URL: "http://host.docker.internal:8080"
  #   - OPENAI_BASE_URL: "http://host.docker.internal:8080/v1"
  #   - OPENAI_API_KEY: ""
  environment: []

# Exercise configuration
exercise:
  language: "java"
  name: ""  # leave empty to run all

# Claude Code CLI configuration
claude:
  cli_path: "/usr/local/bin/claude"
  model: "sonnet"
  extra_args: []

# Output configuration
output:
  results_dir: "../benchmark-results"
  log_level: "INFO"

server:
  port: 8081
```

Note that this example deliberately *overrides* two defaults: `docker.timeout: 3600` (the
default is `300`) and `docker.per_command_timeout: 120` (the default is `600`).

## Reference

### Top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `parallelism` | int | `1` | How many exercises run concurrently |
| `model` | string | *(none)* | Label for the run, used in result directory names. `--model` overrides it. Falls back to `default` when unset |
| `inference_endpoint` | string | *(none — probed)* | Host-side OpenAI-compatible base URL; see below |
| `api_key` | string | *(none)* | Bearer token sent to `inference_endpoint` |

### `server`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `port` | int | `8081` | Dashboard port. `--port` / `SERVER_PORT` override it |

### `docker`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `image` | string | `ghcr.io/dylanschell/llm-benchmark-runner:latest` | Runner image. Must not be empty |
| `work_dir` | string | `/workspace` | Working directory inside the container |
| `timeout` | int | `300` | Per-exercise container timeout, seconds. Minimum 10. Excludes the image pull |
| `pull_timeout` | int | `1800` | Seconds allowed for pulling `image` when it is not present locally, once per machine |
| `memory` | string | `2g` | Container memory limit. Must not be empty |
| `per_command_timeout` | int | `600` | Ceiling for any single Bash tool call inside the container |
| `environment` | list of maps | `[]` | Environment variables for the container; additive, applied over the built-in defaults |

`environment` is a list of one-entry maps rather than a map so the YAML stays order-preserving
and duplicate keys are impossible:

```yaml
docker:
  environment:
    - OPENAI_API_KEY: "sk-…"
    - ANTHROPIC_AUTH_TOKEN: "sk-ant-…"
```

These variables reach the agent CLI *inside* the container, so they are not interchangeable
with the host-side `inference_endpoint`. See
[Two places an endpoint lives](#two-places-an-endpoint-lives).

### `output`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `results_dir` | path | `../benchmark-results` | Where result files are written. Must not be empty |
| `log_level` | string | `INFO` | **Accepted but not read** — see below |

`results_dir` deserves care, because the writer and the reader resolve it differently:

| Process | Resolution order |
|---|---|
| Benchmark executor (writes results) | `RESULTS_DIR` → `output.results_dir` |
| Dashboard result service (reads results) | `RESULTS_DIR` → `output.results_dir` → `./results` |

With no `config.yaml` and no `RESULTS_DIR`, the executor writes to `../benchmark-results`
while the dashboard reads `./results`, so a fresh install shows an empty dashboard until you
set `output.results_dir` explicitly. This is why the README says to set it.

### `inference_endpoint` and `api_key`

This pair is **host-side only**. It is used for exactly one thing: `GET
{inference_endpoint}/models` with `Authorization: Bearer {api_key}`, to populate the
dashboard's model list. It is not what the agent uses — that is `docker.environment`.

`api_key` is sent only when non-empty.

When `inference_endpoint` is absent, the ports `8000`, `8080` and `9931` are probed
concurrently at startup for `{port}/v1/models`, in that order, and the first answer whose JSON
body contains a `data` array is adopted. A plain `200` is not enough — an unrelated web app on
port 8000 is neither adopted nor allowed to mask a real server behind it. Setting
`inference_endpoint` skips probing entirely.

Adopting a local endpoint also sets the container's `OPENAI_BASE_URL` to the same port and
path on `host.docker.internal`, unless you have set `OPENAI_BASE_URL` yourself. Finding
nothing is not an error: the dashboard falls back to a built-in model list and the container
keeps `http://host.docker.internal:8080/v1`.

### Fields that are accepted but have no effect

These are read by nothing. They are kept because the schema is shared with the dashboard's
config type and removing them is a separate change, but setting them today changes no
behaviour:

| Key | Default | Status |
|---|---|---|
| `exercise.language` | `java` | **Not read.** Use `--language` |
| `exercise.name` | *(none)* | **Not read.** Use `--exercise` |
| `exercise.path` | *(none)* | **Not read** (exercises are embedded in the binary) |
| `claude.cli_path` | `/usr/local/bin/claude` | **Not read.** `claude` is invoked from `PATH` inside the container |
| `claude.model` | `sonnet` | **Not read.** Use `--model`, or the top-level `model` |
| `claude.extra_args` | *(none)* | **Not read** |
| `output.log_level` | `INFO` | **Not read.** Use `RUST_LOG` |

The `exercise:` and `claude:` sections in `config.example.yaml` are therefore inert; the CLI
flags named in the table are what control those behaviours.

## Environment variables

| Variable | Read by | Effect |
|---|---|---|
| `CONFIG_PATH` | CLI + web | Config file path; same as `--config`. Default `config.yaml` |
| `SERVER_PORT` | web | Dashboard port; same as `--port`. Beats `server.port` |
| `PARALLELISM` | web | Concurrency; same as `--parallelism`. Beats `parallelism` |
| `RESULTS_DIR` | web | Results directory; same as `--results-dir`. Beats `output.results_dir` |
| `RUST_LOG` | CLI | `tracing` filter override. Default `benchmark_cli=info,benchmark_core=warn,info`; `--verbose` selects `benchmark_cli=debug,benchmark_core=debug,info` |
| `TEMPLATES_DIR` | web | **Development only.** Read templates from a directory instead of the binary |
| `STATIC_DIR` | web | **Development only.** Serve static assets from a directory instead of the binary |
| `LLM_BENCHMARK_EXERCISES_OFFLINE` | build | Build with a warm exercise cache and no network |

`TEMPLATES_DIR` and `STATIC_DIR` exist so the edit-and-reload workflow survives the move to
embedded assets; the default path reads nothing from disk.

## Command-line flags

`llm-benchmark run` (`benchmark-cli`):

| Flag | Default | Meaning |
|---|---|---|
| `--config <path>` | `config.yaml` | Config file |
| `--agent <name>` | `reference` | `reference`, `claude` or `pi` |
| `--language <name>` | `java` | Language to run. The only way to select a language |
| `--exercise <name>` | *(all)* | Run a single exercise |
| `--model <name>` | *(from config)* | Overrides `model` |
| `--results-dir <path>` | *(from config)* | Overrides `output.results_dir` |
| `--retry` | off | Re-run exercises that already have results |
| `--verbose` | off | Live token stream; raises the log filter |

`llm-benchmark web`: `--config`, `--port`, `--results-dir`, `--parallelism` — all optional, all
applied as environment variables.

Also available: `llm-benchmark report` and `llm-benchmark token-report`.

## Validation

`Config::validate()` runs on `run` (not on `web`) and exits 1 with a message. The complete set
of rules:

| Rule | Message |
|---|---|
| `parallelism >= 1` | `parallelism must be at least 1, got: {n}` |
| `docker.image` non-empty | `docker.image is required` |
| `docker.timeout >= 10` | `docker.timeout must be at least 10 seconds, got: {n}` |
| `docker.memory` non-empty | `docker.memory is required` |
| `output.results_dir` non-empty | `output.results_dir is required` |

Nothing else is validated: a nonexistent `results_dir` is created on demand, `docker.memory`
is not checked against Docker's syntax, and `inference_endpoint` is not checked for
reachability.

## Troubleshooting

**A setting has no effect.** Check the spelling — unknown keys are silently ignored, and
`exercise.*`, `claude.*` and `output.log_level` are inert regardless.

**`Failed to load config file`.** `llm-benchmark run` could not read the path. It defaults to
`config.yaml` in the *current directory*; pass `--config` or set `CONFIG_PATH`. A missing file
is not an error (`config.yaml not found — using built-in defaults`), so this message means the
file exists but is unreadable.

**The dashboard is empty but results exist.** The writer and reader resolved different
directories. Set `output.results_dir` (or `RESULTS_DIR`) to the directory holding the
`result_*.json` files.

**The container cannot reach the model server.** Two separate things must both be right: the
host-side `inference_endpoint` (dashboard model list) and the container's `docker.environment`
values (what the agent uses). On Linux, bare Docker Engine does not provide
`host.docker.internal` at all, and a model server bound to `127.0.0.1` is unreachable from a
container even then — bind it to `0.0.0.0`, or point `OPENAI_BASE_URL` at an address the
container can route to.

**An inference endpoint was found that you did not configure.** A server answered on one of
the probed ports with a JSON `data` array. Set `inference_endpoint` explicitly to disable
probing.

## Related documentation

- [README](../README.md) — installation and quick start
- [API](API.md) — the dashboard's HTTP surface
- [Architecture](ARCHITECTURE.md) — how the pieces fit together
- [Result Format](RESULT_FORMAT.md) — what ends up in `results_dir`
