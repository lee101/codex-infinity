# Codex App Server Python SDK (Experimental)

Experimental Python SDK for `codex app-server` JSON-RPC v2 over stdio, with a small default surface optimized for real scripts and apps.

The generated wire-model layer is currently sourced from the bundled v2 schema and exposed as Pydantic models with snake_case Python fields that serialize back to the app-server’s camelCase wire format.

## Install

Install the SDK:

Published SDK builds pin an exact `openai-codex-cli-bin` runtime dependency
with the same version as the SDK. For local repo development, either pass
`AppServerConfig(codex_bin=...)` to point at a local build explicitly, or use
the repo examples/notebook bootstrap which installs the pinned runtime package
automatically.

## Quickstart

The SDK reuses your existing Codex authentication when one is already
available:

```python
from codex_app_server import Codex

with Codex() as codex:
    thread = codex.thread_start()
    result = thread.run("Explain this repository in three bullets.")
    print(result.final_response)
```

`thread.run(...)` returns a `TurnResult` containing the final response,
collected items, and token usage.

## Authentication

Existing Codex authentication is reused automatically. To start ChatGPT
browser login explicitly:

```python
from openai_codex import Codex

with Codex() as codex:
    login = codex.login_chatgpt()
    print(login.auth_url)
    print(login.wait().success)
```

For device-code login:

```python
with Codex() as codex:
    login = codex.login_chatgpt_device_code()
    print(login.verification_url, login.user_code)
    login.wait()
```

## Runtime packaging

The repo no longer checks `codex` binaries into `sdk/python`.

```python
with Codex() as codex:
    codex.login_api_key("sk-...")
```

For local repo development, the checked-in `sdk/python-runtime` package is only
a template for staged release artifacts. Editable installs should use an
explicit `codex_bin` override for manual SDK usage; the repo examples and
notebook bootstrap the pinned runtime package automatically.

## Maintainer workflow

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

Pass `--platform-tag ...` to `stage-runtime` when the wheel should be tagged for
a Rust target that differs from the Python build host. The intended one-off
matrix is `macosx_11_0_arm64`, `macosx_10_9_x86_64`,
`musllinux_1_1_aarch64`, `musllinux_1_1_x86_64`, `win_arm64`, and
`win_amd64`.

This supports the CI release flow:

- run `generate-types` before packaging
- stage `openai-codex-app-server-sdk` once with an exact `openai-codex-cli-bin==...` dependency
- stage `openai-codex-cli-bin` on each supported platform runner with the same pinned runtime version
- build and publish `openai-codex-cli-bin` as platform wheels only; do not publish an sdist

## Compatibility and versioning

- Package: `openai-codex-app-server-sdk`
- Runtime package: `openai-codex-cli-bin`
- Python: `>=3.10`
- Target protocol: Codex `app-server` JSON-RPC v2
- Versioning rule: the SDK package version is the underlying Codex runtime version

## Documentation

- `Codex()` is eager and performs startup + `initialize` in the constructor.
- Use context managers (`with Codex() as codex:`) to ensure shutdown.
- Plain strings are accepted anywhere a turn input is accepted; they are
  shorthand for `TextInput(...)`.
- Prefer `thread.run("...")` for the common case. Use `thread.turn(...)` when
  you need streaming, steering, or interrupt control.
- For transient overload, use `codex_app_server.retry.retry_on_overload`.
