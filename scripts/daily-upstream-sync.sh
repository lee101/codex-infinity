#!/bin/bash
set -eo pipefail

REPO_DIR="/home/lee/code/codex"
LOG_DIR="$REPO_DIR/scripts/logs"
LOG_FILE="$LOG_DIR/upstream-sync-$(date +%Y%m%d-%H%M%S).log"
mkdir -p "$LOG_DIR"

exec > >(tee -a "$LOG_FILE") 2>&1
echo "=== Codex Infinity upstream sync started at $(date) ==="

export HOME="/home/lee"
export PATH="$HOME/.bun/bin:$HOME/.cargo/bin:$HOME/.nvm/versions/node/v22.17.0/bin:$HOME/.local/bin:/usr/local/bin:/usr/bin:/bin"

set +e
[ -f "$HOME/.profile" ] && source "$HOME/.profile" 2>/dev/null
[ -f "$HOME/.bashrc" ] && source "$HOME/.bashrc" 2>/dev/null
set -e

[ -f "$HOME/.cron-env" ] && source "$HOME/.cron-env"

# unset ANTHROPIC_API_KEY so claude uses its default auth
unset ANTHROPIC_API_KEY

for cmd in claude npm git; do
    command -v "$cmd" >/dev/null || { echo "FATAL: $cmd not found in PATH"; exit 1; }
done

export GIT_SSH_COMMAND="ssh -i $HOME/.ssh/codex_agent_key -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new"

cd "$REPO_DIR"

if ! git remote get-url upstream &>/dev/null; then
    git remote add upstream https://github.com/openai/codex.git
fi
git fetch upstream

UPSTREAM_HEAD=$(git rev-parse upstream/main)
LOCAL_MERGE_BASE=$(git merge-base HEAD upstream/main 2>/dev/null || echo "none")
UPSTREAM_ALREADY_MERGED=$(git merge-base --is-ancestor upstream/main HEAD 2>/dev/null && echo "yes" || echo "no")

if [ "$UPSTREAM_ALREADY_MERGED" = "yes" ]; then
    echo "Already up to date with upstream."
    exit 0
fi

BEHIND_COUNT=$(git rev-list HEAD..upstream/main --count)
echo "Upstream has $BEHIND_COUNT new commits. Running Claude to merge..."

timeout 3600 claude -p --dangerously-skip-permissions --model sonnet --verbose <<'PROMPT'
You are the automated daily sync bot for Codex Infinity, a fork of openai/codex.
Your job: merge upstream/main, preserve all customizations, build, verify, version bump, commit, push, and npm publish.

OUR CUSTOMIZATIONS (preserve ALL of these -- if a merge conflict touches these, keep OURS):

1. Package: @codex-infinity/codex-infinity in codex-cli/package.json (name, description, bin, repository)
2. Entry points: codex-cli/bin/codex-infinity.js, codex-cli/bin/codex-infinite.js (keep our versions)
3. verify_vendor.js: codex-cli/scripts/verify_vendor.js (keep ours)
4. README.md: keep our Pi Infinity branded README, NOT the upstream one
5. scripts/daily-upstream-sync.sh: keep ours
6. Rust CLI flags in codex-rs/tui/src/cli.rs: --auto-next-steps, --auto-next-idea, --yolo2, --yolo3, --yolo4
7. Rust AutoNext prompts in codex-rs/tui/src/chatwidget.rs: AUTO_NEXT_STEPS_PROMPTS, AUTO_NEXT_IDEA_PROMPTS
8. Model family entries in codex-rs/core/src/model_family.rs: gpt-5.2/5.3/5.4 with concise GPT_5_CODEX_INSTRUCTIONS
9. Concise prompting: codex-rs/core/gpt_5_codex_prompt.md is our preferred prompt (68 lines). New models should use it, NOT the verbose gpt_5_1_prompt.md (331 lines)
10. Logo/branding: .github/codex-infinity-200h.webp

STEPS (execute in order):
1. Run: git diff HEAD..upstream/main --stat (understand what changed)
2. Run: git merge upstream/main --no-edit
3. If conflicts: resolve them preserving our customizations. For our custom files (package.json, README, bin/codex-infinity.js, daily-upstream-sync.sh, verify_vendor.js) ALWAYS keep ours. For Cargo.lock accept theirs. For Rust source conflicts, merge carefully keeping our custom flags/features.
4. Verify customizations survived:
   - grep -q "codex-infinity" codex-cli/package.json
   - grep -q "auto_next_steps" codex-rs/tui/src/cli.rs
   - grep -q "yolo2" codex-rs/tui/src/cli.rs
   - grep -q "auto_next_idea" codex-rs/tui/src/cli.rs
   If any check fails, investigate and fix before proceeding.
5. Check if upstream added any new model series (e.g. gpt-5.5, gpt-6). If so, add entries in model_family.rs using GPT_5_CODEX_INSTRUCTIONS (the concise prompt). Keep prompting minimal.
6. Build: cd /home/lee/code/codex/codex-rs && cargo build --release -p codex-cli
7. Copy binary: cp /home/lee/code/codex/codex-rs/target/release/codex /home/lee/code/codex/codex-cli/vendor/x86_64-unknown-linux-gnu/codex/codex
8. Bump patch version in codex-cli/package.json (increment the patch number)
9. git add the changed files (README.md, codex-cli/package.json, codex-rs/core/src/model_family.rs, codex-cli/vendor/x86_64-unknown-linux-gnu/codex/codex, and any other changed files)
10. git commit with message: "Codex Infinity vX.Y.Z - merge upstream + maintain custom features"
11. git push origin main
12. cd /home/lee/code/codex/codex-cli && npm publish --access public

SAFETY:
- If cargo build fails, try to fix the issue (max 2 attempts). If still failing, abort: git merge --abort or git reset --hard HEAD~1
- If npm publish fails, log the error but still exit 0 (push already succeeded).
- NEVER force push. NEVER rewrite history.
- If >20 conflicting files, abort and log a summary.
PROMPT

EXIT_CODE=$?
if [ $EXIT_CODE -ne 0 ]; then
    echo "WARNING: Claude exited with code $EXIT_CODE"
fi

echo "=== Sync completed at $(date) (exit: $EXIT_CODE) ==="

# keep only last 30 logs
ls -t "$LOG_DIR"/upstream-sync-*.log 2>/dev/null | tail -n +31 | xargs -r rm
