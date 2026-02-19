#!/bin/bash
set -euo pipefail

REPO_DIR="/home/lee/code/codex"
LOG_DIR="$REPO_DIR/scripts/logs"
LOG_FILE="$LOG_DIR/upstream-sync-$(date +%Y%m%d-%H%M%S).log"
mkdir -p "$LOG_DIR"

exec > >(tee -a "$LOG_FILE") 2>&1
echo "=== Codex Infinity upstream sync started at $(date) ==="

# cron has minimal env -- load what we need
export HOME="/home/lee"
export PATH="$HOME/.bun/bin:$HOME/.cargo/bin:$HOME/.nvm/versions/node/v22.17.0/bin:$HOME/.local/bin:/usr/local/bin:/usr/bin:/bin"

# source API keys from shell profile
[ -f "$HOME/.profile" ] && source "$HOME/.profile" 2>/dev/null || true
[ -f "$HOME/.bashrc" ] && source "$HOME/.bashrc" 2>/dev/null || true

# verify required tools
for cmd in claude cargo npm git; do
    command -v "$cmd" >/dev/null || { echo "FATAL: $cmd not found in PATH"; exit 1; }
done

cd "$REPO_DIR"

# ensure upstream remote exists, then fetch
if ! git remote get-url upstream &>/dev/null; then
    git remote add upstream https://github.com/openai/codex.git
fi
git fetch upstream

UPSTREAM_HEAD=$(git rev-parse upstream/main)
LOCAL_HEAD=$(git rev-parse HEAD)

if [ "$UPSTREAM_HEAD" = "$LOCAL_HEAD" ]; then
    echo "Already up to date with upstream. Nothing to do."
    exit 0
fi

echo "Upstream has new commits (local: ${LOCAL_HEAD:0:8}, upstream: ${UPSTREAM_HEAD:0:8}). Running Claude to merge..."

claude -p --dangerously-skip-permissions --model opus --verbose <<'PROMPT'
You are the automated daily sync bot for Codex Infinity, a fork of openai/codex.
Your job: merge upstream/main into our main branch, preserve all customizations, verify everything works, and deploy.

OUR CUSTOMIZATIONS (preserve ALL of these -- if a merge conflict touches these, keep OURS):

1. CLI flags in codex-rs/tui/src/cli.rs: --auto-next-steps, --auto-next-idea, --yolo2, --yolo3, --yolo4
2. NPM package: @codex-infinity/codex-infinity in codex-cli/package.json (name, description, bin entry)
3. Binary wrapper: codex-cli/bin/codex-infinity.js
4. Concise system prompts: ALL prompt files in codex-rs/core/ (prompt.md, prompt_with_apply_patch_instructions.md, gpt_*.md, etc.) should stay SHORT and focused. We strip the verbose upstream prompt engineering. If upstream changed these, keep our concise versions.
5. Higher retry limits in codex-rs/core/src/model_provider_info.rs (stream_max_retries=60, request_max_retries=100, max=200)
6. README.md: keep our Codex Infinity branded README with the infinity logo, NOT the upstream OpenAI one
7. Logo files: .github/codex-infinity-* must be preserved
8. verify_vendor.js and prepack script in codex-cli/package.json

NEW MODEL HANDLING:
- If upstream added NEW model-specific prompt files (e.g. gpt_5_3_prompt.md or similar), CREATE a concise version matching our style. Look at our existing prompt files for the pattern -- they are short, direct, no verbose chain-of-thought scaffolding.
- If upstream added new model entries in model_provider_info.rs, keep them but apply our higher retry limits.

STEPS (execute in order):
1. Run: git diff HEAD..upstream/main --stat   (understand what changed)
2. Run: git merge upstream/main --no-edit
3. If conflicts: resolve them preserving our customizations. For prompt files, ALWAYS keep ours. For code changes, merge intelligently.
4. Verify our customizations are intact by reading the key files listed above
5. Check for any NEW prompt files from upstream and simplify them
6. Run: cd /home/lee/code/codex/codex-rs && cargo build --release -p codex-tui
7. Run: cd /home/lee/code/codex/codex-rs && cargo test
8. Copy built binary: cp /home/lee/code/codex/codex-rs/target/release/codex /home/lee/code/codex/codex-cli/vendor/x86_64-unknown-linux-gnu/codex/codex
9. Bump patch version in codex-cli/package.json (e.g. 1.1.0 -> 1.1.1)
10. Commit all changes with message: "Codex Infinity vX.Y.Z - merge upstream + maintain custom features"
11. Run: git push origin main
12. Run: cd /home/lee/code/codex/codex-cli && npm publish --access public

SAFETY:
- If cargo build fails, try to fix the issue. If you cannot fix it in 2 attempts, abort: git merge --abort && git checkout main
- If cargo test fails, investigate. If tests fail due to our changes, fix them. If upstream tests are broken, skip npm publish but still push the merge.
- If npm publish fails, log the error but don't fail the script (exit 0 still).
- NEVER force push. NEVER rewrite history.
- If the merge is too complex (>20 conflicting files), abort and log a summary of what needs manual attention.
PROMPT

EXIT_CODE=$?
if [ $EXIT_CODE -ne 0 ]; then
    echo "WARNING: Claude exited with code $EXIT_CODE"
fi

echo "=== Sync completed at $(date) (exit: $EXIT_CODE) ==="

# keep only last 30 logs
ls -t "$LOG_DIR"/upstream-sync-*.log 2>/dev/null | tail -n +31 | xargs -r rm
