<p align="center">
  <img src="./.github/codex-infinity-200h.webp" alt="Codex Infinity" height="200" />
</p>
<h1 align="center">Codex Infinity</h1>

<p align="center"><code>npm i -g @codex-infinity/codex-infinity</code></p>

<p align="center"><strong>Codex Infinity</strong> is a smarter coding agent that can run forever.</p>

<p align="center">Based on <a href="https://github.com/openai/codex">OpenAI Codex CLI</a> with autonomous workflow extensions.</p>

---

## What makes Codex Infinity different?

Three arguments turn Codex into a fully autonomous coding agent:

- **`--auto-next-steps`** -- After each response, automatically continues with the next logical steps (including testing)
- **`--auto-next-idea`** -- Generates and implements new improvement ideas for your codebase
- **`--auto-next-goal`** -- When a `/goal` completes, generates and starts the next goal automatically

```shell
# Autonomous coding -- completes tasks then moves to the next one
codex-infinity --auto-next-steps "fix all lint errors and add tests"

# Fully autonomous -- dreams up and implements improvements forever
codex-infinity --auto-next-steps --auto-next-idea

# Goal loop -- finish a goal, then automatically create the next one
codex-infinity --auto-next-goal "/goal improve startup parity with installed Codex"

# Full auto mode with autonomous continuation
codex-infinity --full-auto --auto-next-steps
```

## Quickstart

```shell
npm install -g @codex-infinity/codex-infinity
```

Then run `codex-infinity` to get started.

By default, `codex-infinity` runs on `gpt-5.4`. Override it with `-m/--model`, `-c model=...`, or your `~/.codex/config.toml`.

### Authentication

Run `codex-infinity` and select **Sign in with ChatGPT** to use your Plus, Pro, Team, Edu, or Enterprise plan.

Or use an API key:

```shell
export OPENAI_API_KEY=sk-...
codex-infinity "your prompt"
```

### Model providers (auto-detected)

`codex-infinity` auto-detects which provider to use from the model slug and the API keys present in your environment — no `config.toml` edits required. Export a key and select a matching model with `-m`:

| Provider | Env var | Example models |
|----------|---------|----------------|
| OpenAI | `OPENAI_API_KEY` | `gpt-5.4`, `o3` |
| Cerebras | `CEREBRAS_API_KEY` | `cerebras/gpt-oss-120b`, `cerebras/zai-glm-4.7` |
| OpenPaths | `OPENPATHS_API_KEY` | `openpaths/auto`, `cerebras/gpt-oss-120b`, `composer-2.5` |
| OpenRouter | `OPENROUTER_API_KEY` | `anthropic/claude-opus-4.6`, `google/gemini-3.5-flash` |
| Google Gemini | `GEMINI_API_KEY` | `google/gemini-3.5-flash` |
| Z.AI (Zhipu) | `ZAI_API_KEY` | `glm-4.7`, `z-ai/glm-5` |
| DeepSeek | `DEEPSEEK_API_KEY` | `deepseek/deepseek-v4-flash` |
| Cursor | `CURSOR_API_KEY` | `cursor/composer-2.5` |
| Local (OSS) | — (`--oss`) | LM Studio / Ollama models |

**Cerebras** runs the fast open-weight coding models (`gpt-oss-120b`, `zai-glm-4.7`). A `cerebras/*` model prefers a direct Cerebras key (`CEREBRAS_API_KEY`, `https://api.cerebras.ai`) and otherwise falls back to **OpenPaths** ([openpaths.io](https://openpaths.io)), a router that also serves the Cerebras-hosted models — so a single `OPENPATHS_API_KEY` is enough to reach them. Override the endpoints with `CEREBRAS_BASE_URL` / `OPENPATHS_BASE_URL` if needed.

```shell
# Direct Cerebras
export CEREBRAS_API_KEY=csk-...
codex-infinity -m cerebras/gpt-oss-120b "refactor this module"

# Or via OpenPaths (also serves Cerebras models)
export OPENPATHS_API_KEY=op-...
codex-infinity -m cerebras/zai-glm-4.7 "explain this bug"
```

## CLI flags

| Flag | Description |
|------|-------------|
| `--auto-next-steps` | Auto-continue with next logical steps after each response |
| `--auto-next-idea` | Auto-brainstorm and implement new improvement ideas |
| `--auto-next-goal` | Auto-generate and start a new `/goal` after the current goal completes |
| `--full-auto` | Low-friction sandboxed automatic execution |
| `--yolo` | Skip approvals and sandbox (dangerous) |
| `--yolo2` | Like yolo + disable command timeouts |
| `--yolo3` | Like yolo2 + pass full host environment |
| `--yolo4` | Like yolo3 + stream stdout/stderr directly |
| `-m MODEL` | Select model (e.g. `gpt-5.4`, `o3`) |
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

# Keep goal mode running autonomously
codex-infinity --auto-next-goal "/goal improve benchmark coverage"

# Quick one-shot with yolo mode
codex-infinity --yolo "add error handling to all API endpoints"

# Use a specific model
codex-infinity -m gpt-5.4 --auto-next-steps "optimize database queries"

# Use local models
codex-infinity --oss -m llama3 "explain this codebase"
```

## Features

- **Autonomous operation** -- `--auto-next-steps` keeps it working without intervention
- **Idea generation** -- `--auto-next-idea` brainstorms and implements improvements
- **Goal chaining** -- `--auto-next-goal` turns completed `/goal` work into the next objective
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

### Fast dev builds

The Rust workspace has ~90 crates. Speed up compilation with:

```bash
# One-time setup
sudo apt install -y mold          # fast linker
cargo install sccache             # compile cache

# Add to ~/.cargo/config.toml
cat > ~/.cargo/config.toml << 'EOF'
[build]
jobs = 72
rustc-wrapper = "sccache"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
EOF

# Add to shell rc
export SCCACHE_DIR=/path/to/fast/disk/sccache
export SCCACHE_CACHE_SIZE=50G
```

Profile bottlenecks with `cargo build --timings` and check cache hits with `sccache --show-stats`.

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
