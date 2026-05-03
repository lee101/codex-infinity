# npm releases

## Codex Infinity local publish

To publish a new `@codex-infinity/codex-infinity` package with the current Rust
binary:

```bash
cd codex-cli
npm run deploy
```

This builds `codex-rs`, copies `target/release/codex` into
`codex-cli/vendor/x86_64-unknown-linux-gnu/codex/codex`, bumps the package patch
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

This downloads the native artifacts once, hydrates `vendor/` for each package, and writes
tarballs to `dist/npm/`.

When `--package codex` is provided, the staging helper builds the lightweight
`@openai/codex` meta package plus all platform-native `@openai/codex` variants
that are later published under platform-specific dist-tags.

If you need to invoke `build_npm_package.py` directly, run
`codex-cli/scripts/install_native_deps.py` first and pass `--vendor-src` pointing to the
directory that contains the populated `vendor/` tree.
