<p align="center">
  <img src="./.github/codex-infinity-200h.webp" alt="Codex Infinity" height="200" />
</p>

<h1 align="center">Codex Infinity</h1>

<p align="center"><code>npm i -g @codex-infinity/codex-infinity</code></p>

<p align="center"><strong>Codex Infinity</strong> is a smarter coding agent that can run forever.</p>

<p align="center">Based on <a href="https://github.com/openai/codex">OpenAI Codex CLI</a> with autonomous workflow extensions.</p>

---

## What makes Codex Infinity different?

Two arguments turn Codex into a fully autonomous coding agent:

- **`--auto-next-steps`** -- After each response, automatically continues with the next logical steps (including testing)
- **`--auto-next-idea`** -- Generates and implements new improvement ideas for your codebase

```shell
# Autonomous coding -- completes tasks then moves to the next one
codex-infinity --auto-next-steps "fix all lint errors and add tests"

# Fully autonomous -- dreams up and implements improvements forever
codex-infinity --auto-next-steps --auto-next-idea

# Full auto mode with autonomous continuation
codex-infinity --full-auto --auto-next-steps
```

## Quickstart

```shell
npm install -g @codex-infinity/codex-infinity
```

Then run `codex-infinity` to get started.

### Authentication

Run `codex-infinity` and select **Sign in with ChatGPT** to use your Plus, Pro, Team, Edu, or Enterprise plan.

Or use an API key:

```shell
export OPENAI_API_KEY=sk-...
codex-infinity "your prompt"
```

## CLI flags

| Flag | Description |
|------|-------------|
| `--auto-next-steps` | Auto-continue with next logical steps after each response |
| `--auto-next-idea` | Auto-brainstorm and implement new improvement ideas |
| `--full-auto` | Low-friction sandboxed automatic execution |
| `--yolo` | Skip approvals and sandbox (dangerous) |
| `--yolo2` | Like yolo + disable command timeouts |
| `--yolo3` | Like yolo2 + pass full host environment |
| `--yolo4` | Like yolo3 + stream stdout/stderr directly |
| `-m MODEL` | Select model (e.g. `gpt-5.3-codex`, `o3`) |
| `--oss` | Use local model provider (LM Studio / Ollama) |
| `--search` | Enable live web search |
| `-i FILE` | Attach image(s) to initial prompt |
| `--cd DIR` | Set working directory |
| `--profile NAME` | Use config profile from config.toml |

## Examples

```shell
# Fix a bug with full autonomy
codex-infinity --full-auto --auto-next-steps "fix the failing test in auth.test.ts"

# Refactor with idea generation
codex-infinity --auto-next-steps --auto-next-idea "refactor the API layer"

# Quick one-shot with yolo mode
codex-infinity --yolo "add error handling to all API endpoints"

# Use a specific model
codex-infinity -m gpt-5.3-codex --auto-next-steps "optimize database queries"

# Use local models
codex-infinity --oss -m llama3 "explain this codebase"
```

## Features

- **Autonomous operation** -- `--auto-next-steps` keeps it working without intervention
- **Idea generation** -- `--auto-next-idea` brainstorms and implements improvements
- **AnyLLM** -- OpenAI, local models via LM Studio/Ollama, bring your own provider
- **Local execution** -- runs entirely on your machine
- **Concise prompts** -- stripped-down system prompts for faster, more focused responses
- **Higher reliability** -- increased retry limits for long-running autonomous sessions

## Development

### Build from source (Rust CLI)

```bash
cd codex-rs
cargo build --release -p codex-tui
./target/release/codex "your prompt here"
```

### Build npm package

```bash
cd codex-cli
npm install
```

### Project structure

- `codex-rs/` -- Rust workspace (TUI, core, sandbox, etc.)
- `codex-cli/` -- npm package wrapper
- `sdk/` -- TypeScript SDK

## Docs

- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)

Based on [OpenAI Codex CLI](https://github.com/openai/codex). Licensed under [Apache-2.0](LICENSE).
