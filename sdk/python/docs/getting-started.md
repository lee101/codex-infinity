# Getting Started

This guide gets a published OpenAI Codex Python SDK beta installation running
with a multi-turn thread.

The SDK is experimental. Treat the API, bundled runtime strategy, and packaging details as unstable until the first public release.

Install the SDK:

```bash
pip install openai-codex
```

Requirements:

- Python `>=3.10`
- An existing Codex account session, or one of the login flows below

The SDK installs its compatible `openai-codex-cli-bin` runtime dependency
automatically. While beta releases are the only published SDK releases, this
normal install command selects the latest beta. After a stable release exists,
use `pip install --pre openai-codex` to opt into a newer prerelease.

## 2. Authenticate When Needed

Existing Codex authentication is reused automatically. For ChatGPT browser
login:

```python
from openai_codex import Codex

with Codex() as codex:
    login = codex.login_chatgpt()
    print(login.auth_url)
    print(login.wait().success)
```

For device-code login:

```python
from codex_app_server import Codex

with Codex() as codex:
    thread = codex.thread_start(sandbox=Sandbox.workspace_write)
    result = thread.run("Say hello in one sentence.")

    print("Thread:", thread.id)
    print("Text:", result.final_response)
    print("Items:", len(result.items))
```

`Thread.run(...)` starts a turn, waits for completion, and returns
`TurnResult`. Plain strings are shorthand for `TextInput(...)`.

- `Codex()` started and initialized `codex app-server`.
- `thread_start(...)` created a thread.
- `thread.run("...")` started a turn, consumed events until completion, and returned `TurnResult` with turn metadata, final assistant response, collected items, and usage.
- `result.final_response` is `None` when no final-answer or phase-less assistant message item completes for the turn.
- use `thread.turn(...)` when you need a `TurnHandle` for streaming, steering, interrupting, or turn IDs/status
- one client can have only one active turn consumer (`thread.run(...)`, `TurnHandle.stream()`, or `TurnHandle.run()`) at a time in the current experimental build

## 4. Choose Sandbox Access

Use one enum for the initial thread and later turn overrides:

```python
from openai_codex import Codex, Sandbox

with Codex() as codex:
    thread = codex.thread_start(sandbox=Sandbox.workspace_write)
    thread.run("Make the requested changes.")
    review = thread.run("Review the diff only.", sandbox=Sandbox.read_only)
```

Available presets:

- `Sandbox.read_only`: read files without allowing writes.
- `Sandbox.workspace_write`: read files and write inside the workspace and
  configured writable roots; this is the normal default for workspace work.
- `Sandbox.full_access`: run without filesystem access restrictions.

When `sandbox=` is omitted, Codex uses its configured default. A turn override
also applies to subsequent turns on that thread.

## 5. Continue A Thread

```python
from codex_app_server import Codex

with Codex() as codex:
    thread = codex.thread_start()
    thread.run("Summarize Rust ownership in two bullets.")
    result = thread.run("Now explain it to a Python developer.")
    print(result.final_response)
```

To resume a stored thread later:

```python
with Codex() as codex:
    thread = codex.thread_resume("thr_123")
    print(thread.run("Continue where we left off.").final_response)
```

## 6. Use The Async Client

```python
import asyncio
from codex_app_server import AsyncCodex


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(sandbox=Sandbox.workspace_write)
        result = await thread.run("Continue where we left off.")
        print(result.final_response)


asyncio.run(main())
```

## 7. Get Help

Python's built-in documentation tools cover the curated SDK surface:

```python
from codex_app_server import Codex

help(openai_codex)
help(Codex)
help(CodexConfig)
```

## 6) Generated models

The convenience wrappers live at the package root, but the canonical app-server models live under:

```python
from codex_app_server.generated.v2_all import Turn, TurnStatus, ThreadReadResponse
```

## Developing From This Repository

Contributors working from a checkout can install development dependencies from
the repository:

```bash
cd sdk/python
uv sync --group dev
source .venv/bin/activate
```

## Next Stops

- [API reference](https://github.com/openai/codex/blob/main/sdk/python/docs/api-reference.md)
- [FAQ](https://github.com/openai/codex/blob/main/sdk/python/docs/faq.md)
- [Runnable examples](https://github.com/openai/codex/blob/main/sdk/python/examples/README.md)
