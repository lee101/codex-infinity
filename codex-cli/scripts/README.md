# npm releases

## Codex Infinity local publish

To publish a new `@codex-infinity/codex-infinity` package with the current Rust
binaries:

```bash
cd codex-cli
npm run deploy
```

This builds `codex-rs`, copies `target/release/codex` and
`target/release/codex-code-mode-host` into
`codex-cli/vendor/x86_64-unknown-linux-gnu/codex/`, bumps the package patch
version, verifies the Linux x64 vendor payload, and runs `npm publish --access
public`.

Preview the package without publishing:

```bash
cd codex-cli
npm run deploy:dry-run
```

Use `scripts/deploy_new_binary.sh --no-bump` only when `package.json` already
contains an unpublished version.

## OpenAI release staging

Use the staging helper in the repo root to generate npm tarballs for a release. For
example, to stage the CLI, responses proxy, and SDK packages for version `0.6.0`:

```bash
./scripts/stage_npm_packages.py \
  --release-version 0.6.0 \
  --package codex \
  --package codex-responses-api-proxy \
  --package codex-sdk
```

This downloads the required native package archive artifacts, hydrates `vendor/` for
each package, and writes tarballs to `dist/npm/`.

When `--package codex` is provided, the staging helper builds the lightweight
`@openai/codex` meta package plus all platform-native `@openai/codex` variants
that are later published under platform-specific dist-tags.

The npm variants omit the standalone `codex-exec-server` executable and its
`execServer` manifest entry to keep uploads within npm's package size limit.
They retain `codex exec-server` through the CLI, the code-mode host, and the
platform sandbox helpers. Canonical release archives retain the standalone
executor; npm staging changes only the package copy.

Direct `build_npm_package.py` invocations are still useful for package-specific
debugging, but native packages expect `--vendor-src` to point at a prehydrated
`vendor/` tree. Release packaging should use `scripts/stage_npm_packages.py`
