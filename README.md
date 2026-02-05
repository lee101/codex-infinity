<p align="center">
  <img src="./.github/codex-infinity-200h.webp" alt="Codex Infinity" height="200" />
</p>

<h1 align="center">Codex Infinity</h1>

<p align="center"><code>npm i -g @codex-infinity/codex-infinity</code></p>

<p align="center"><strong>Codex Infinity</strong> is a smarter coding agent that can run forever.</p>

---

## What makes Codex Infinity different?

Codex Infinity takes automation to the next level. With two simple arguments, it can continuously work on your codebase:

- **`--auto-next-steps`** - Automatically continues working on the next logical steps after completing a task
- **`--auto-next-idea`** - Generates and works on new ideas to improve your codebase

```shell
# Run codex-infinity with automatic continuation
codex-infinity --auto-next-steps

# dream up and implement new ideas for fully autonomous coding
codex-infinity --auto-next-steps --auto-next-idea
```

## Quickstart

### Installation

```shell
npm install -g @codex-infinity/codex-infinity
```

Then simply run `codex-infinity` to get started.

### Authentication

Run `codex-infinity` and select **Sign in with ChatGPT** to use your Plus, Pro, Team, Edu, or Enterprise plan.

You can also use Codex Infinity with an API key.

## Features

- **Autonomous operation** - Set it and let it run
- **Smart task continuation** - Knows what to do next
- **Idea generation** - Can brainstorm and implement improvements
- **Local execution** - Runs entirely on your machine or an [infinite cloud](https://codex-infinity.com/)
- **AnyLLM** - bring any llm or your OpenAI codex max plan

## Development

### Build from source (Rust CLI)

```bash
cd codex-rs
cargo build --release -p codex-cli

# Run the TUI
cargo run --bin codex -- "your prompt here"
```
### Support

This project is supported from trading volume of the [codex memecoin on bags](
https://bags.fm/HAK9cX1jfYmcNpr6keTkLvxehGPWKELXSu7GH2ofBAGS)

### Build TypeScript CLI

```bash
cd codex-cli
npm install
```

### Project structure

- `codex-rs/` - Rust workspace (TUI, core, sandbox, etc.)
- `codex-cli/` - TypeScript CLI wrapper (npm package)
- `sdk/` - TypeScript SDK

## Docs

- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).
