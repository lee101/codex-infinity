# FAQ

## Is the Python SDK stable?

`openai-codex` is a public beta. Install it with
`pip install openai-codex`; public APIs may change before `1.0`. While beta
releases are the only published SDK releases, pip selects the latest beta.
After a stable release exists, pass `--pre` to opt into newer prereleases.

## Why does the SDK install a runtime package?

The SDK and runtime packages are versioned independently. Each SDK release
pins and installs one compatible runtime dependency automatically.

## Thread vs turn

- A `Thread` is conversation state.
- A `Turn` is one model execution inside that thread.
- Multi-turn chat means multiple turns on the same `Thread`.

## `run()` vs `stream()`

- `TurnHandle.run()` / `AsyncTurnHandle.run()` is the easiest path. It consumes events until completion and returns the canonical generated app-server `Turn` model.
- `TurnHandle.stream()` / `AsyncTurnHandle.stream()` yields raw notifications (`Notification`) so you can react event-by-event.

Choose `run()` for most apps. Choose `stream()` for progress UIs, custom timeout logic, or custom parsing.

## Sync vs async clients

- `Codex` is the sync public API.
- `AsyncCodex` is an async replica of the same public API shape.
- Prefer `async with AsyncCodex()` for async code. It is the standard path for
  explicit startup/shutdown, and `AsyncCodex` initializes lazily on context
  entry or first awaited API use.

If your app is not already async, stay with `Codex`.

## How do I log in?

- `login_api_key(...)` authenticates immediately with an API key.
- `login_chatgpt()` starts browser login and returns a handle with `auth_url`.
- `login_chatgpt_device_code()` starts device-code login and returns a handle
  with `verification_url` and `user_code`.
- Interactive handles expose `wait()` for the matching
  `account/login/completed` notification and `cancel()` to stop that attempt.
- `account()` reads the current account state, and `logout()` clears it.

## Public kwargs are snake_case

Public API keyword names are snake_case. The SDK still maps them to wire camelCase under the hood.

If you are migrating older code, update these names:

- `approvalPolicy` -> `approval_policy`
- `baseInstructions` -> `base_instructions`
- `developerInstructions` -> `developer_instructions`
- `modelProvider` -> `model_provider`
- `modelProviders` -> `model_providers`
- `sortKey` -> `sort_key`
- `sourceKinds` -> `source_kinds`
- `outputSchema` -> `output_schema`

## How do I choose sandbox access?

Use the same `sandbox=` keyword for threads and turns:

```python
from openai_codex import Sandbox

thread = codex.thread_start(sandbox=Sandbox.workspace_write)
result = thread.run("Review only.", sandbox=Sandbox.read_only)
```

The presets are:

- `Sandbox.read_only`: read files without allowing writes.
- `Sandbox.workspace_write`: the normal default for projects with a recorded trust decision; read files and write inside the workspace and configured writable roots.
- `Sandbox.full_access`: run without filesystem access restrictions.

When `sandbox=` is omitted, Codex uses its configured default. A turn
sandbox override applies to that turn and subsequent turns.

## Why only `thread_start(...)` and `thread_resume(...)`?

The public API keeps only explicit lifecycle calls:

- `thread_start(...)` to create new threads
- `thread_resume(thread_id, ...)` to continue existing threads

This avoids duplicate ways to do the same operation and keeps behavior explicit.

## Why does constructor fail?

`Codex()` is eager: it starts transport and calls `initialize` in `__init__`.

Common causes:

- installation is incomplete and the pinned `openai-codex-cli-bin` dependency is missing
- local `codex_bin` override points to a missing file
- a custom local Codex executable does not support the SDK operation being used

Maintainers stage releases by building the SDK once and the runtime once per
platform with the same pinned runtime version. Publish `openai-codex-cli-bin`
as platform wheels only; do not publish an sdist:

```bash
cd sdk/python
python scripts/update_sdk_artifacts.py generate-types
python scripts/update_sdk_artifacts.py \
  stage-sdk \
  /tmp/codex-python-release/openai-codex-app-server-sdk \
  --codex-version <codex-release-tag-or-pep440-version>
python scripts/update_sdk_artifacts.py \
  stage-runtime \
  /tmp/codex-python-release/openai-codex-cli-bin \
  /path/to/codex \
  --codex-version <codex-release-tag-or-pep440-version>
```

If you are packaging a binary for a different target than the Python build
host, pass `--platform-tag ...` to `stage-runtime`. The intended one-off matrix
is `macosx_11_0_arm64`, `macosx_10_9_x86_64`, `musllinux_1_1_aarch64`,
`musllinux_1_1_x86_64`, `win_arm64`, and `win_amd64`.

## Why does a turn "hang"?

A turn is complete only when `turn/completed` arrives for that turn ID.

- `run()` waits for this automatically.
- With `stream()`, keep consuming notifications until completion.

## How do I retry safely?

Use `retry_on_overload(...)` for transient overload failures (`ServerBusyError`).

Do not blindly retry all errors. For `InvalidParamsError` or
`MethodNotFoundError`, fix the input or use the runtime pinned by the SDK.

## Common pitfalls

- Starting a new thread for every prompt when you wanted continuity.
- Forgetting to `close()` (or not using context managers).
- Assuming `run()` returns extra SDK-only fields instead of the generated `Turn` model.
- Mixing SDK input classes with raw dicts incorrectly.
