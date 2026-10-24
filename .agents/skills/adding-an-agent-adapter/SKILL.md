---
name: adding-an-agent-adapter
description: Add or change an agent adapter under test (pi, claude, reference). Use when changing crates/benchmark-core/src/agent, the Agent trait, streaming message processors, cancellation, or how thinking level reaches the CLI.
---

# Adding an Agent Adapter

An adapter drives one agent CLI inside the runner container and turns its output stream into an
`AgentResult`. The trait is `crates/benchmark-types/src/agent/mod.rs::Agent`; the implementations
are `pi.rs`, `claude.rs` and `reference.rs` in `crates/benchmark-core/src/agent/`.

## What you must supply

| Member | Contract |
| --- | --- |
| `run_exercise_with_timeout` | The real work. `run_exercise` forwards to it with `None`. |
| `run_aoc` | AoC day: no test suite, writes answers to the work dir, validates via `aoc2015::solve`. |
| `get_name` | Must match the key used by `AgentKind::from_str` (`reference`, `claude`, `pi`) — that mapping is how `--agent` resolves. |
| `set_cancellation_token` | Default is a no-op; override it. A run that ignores cancellation burns the whole container timeout after the user pressed stop. |

Errors cross the trait as `AgentError` (a boxed `dyn Error + Send + Sync`). Every caller re-wraps
it into `anyhow` with context; a typed `AgentError` enum is the planned replacement, so keep new
failure information in the message rather than in a side channel.

## Wiring a new adapter in

1. `crates/benchmark-core/src/agent/<name>.rs` — the adapter.
2. `AgentKind` in `benchmark-types/src/agent/mod.rs` — new variant + the `from_str` arm; the error
   text there lists the supported agents, so update it too.
3. `make_agent` in `crates/benchmark-core/src/lib.rs` and `create_agent` in `benchmark-cli/src/runner.rs`,
   plus the dispatch in `benchmark-web/src/services/benchmark_executor.rs`.
4. `docker/agents.env` + `docker/pins.env` if the CLI ships in the runner image — that is a runner
   image change (`bumping-the-runner-image`), including the version bump.

## Streaming and files

- Output arrives as JSON events. Each adapter has a message processor
  (`pi_message_processor.rs`, `claude_message_processor.rs`) that turns events into the streamed
  text the UI shows and the trace file records. Token and turn counts are later recomputed from
  that trace file (`benchmark-web/src/services/result_service/trace.rs`), so the trace format is a
  contract: changing it silently changes reported tokens for old runs too.
- `exercise_files.rs` materialises the exercise into the mounted work dir; `test_patches.rs` strips
  each language's skip marker so the whole suite runs — `#[ignore]` for Rust, `@Disabled` for Java,
  `xtest(` for JavaScript. A new language needs a rule there, or its skipped tests never run and the
  score reports a pass rate measured over a fraction of the suite.

## Thinking level

`ThinkingLevel` (`benchmark-types/src/reasoning.rs`) is the input; `ReasoningRegistry::get_for_model`
decides *how* a model is asked to reason (`reasoning_effort`, `thinking` blocks, nothing), because
the CLIs disagree. Adding a model usually means a registry mapping, not adapter code — see the
`map … reasoning_effort` commits in the log for the shape of that change.

## Checks

```bash
cargo test -p benchmark-core --lib
cargo test --workspace --lib --bins --tests --locked
./build.sh lint
```

A run against a real endpoint is the only end-to-end proof; note in the PR which agent, model and
endpoint you used, because those three define the result.
