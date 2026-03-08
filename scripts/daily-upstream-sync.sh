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

# source env without -u (bashrc may reference unset vars)
set +e
[ -f "$HOME/.profile" ] && source "$HOME/.profile" 2>/dev/null
[ -f "$HOME/.bashrc" ] && source "$HOME/.bashrc" 2>/dev/null
set -e

# source API keys if available
[ -f "$HOME/.cron-env" ] && source "$HOME/.cron-env"

for cmd in cargo npm git; do
    command -v "$cmd" >/dev/null || { echo "FATAL: $cmd not found in PATH"; exit 1; }
done

cd "$REPO_DIR"

if ! git remote get-url upstream &>/dev/null; then
    git remote add upstream https://github.com/openai/codex.git
fi
git fetch upstream

UPSTREAM_HEAD=$(git rev-parse upstream/main)
LOCAL_HEAD=$(git rev-parse HEAD)

if [ "$UPSTREAM_HEAD" = "$LOCAL_HEAD" ]; then
    echo "Already up to date with upstream."
    exit 0
fi

echo "Upstream has new commits (local: ${LOCAL_HEAD:0:8}, upstream: ${UPSTREAM_HEAD:0:8})"

# our files to always keep ours on conflict
OUR_FILES=(
    "README.md"
    "codex-cli/package.json"
    "codex-cli/bin/codex-infinity.js"
    "codex-cli/scripts/verify_vendor.js"
    "scripts/daily-upstream-sync.sh"
)

# attempt merge
if git merge upstream/main --no-edit 2>&1; then
    echo "Clean merge succeeded"
else
    CONFLICTS=$(git diff --name-only --diff-filter=U 2>/dev/null || true)
    CONFLICT_COUNT=$(echo "$CONFLICTS" | grep -c . || true)
    echo "Merge has $CONFLICT_COUNT conflicts"

    if [ "$CONFLICT_COUNT" -gt 20 ]; then
        echo "ABORT: Too many conflicts ($CONFLICT_COUNT). Needs manual merge."
        git merge --abort
        exit 1
    fi

    # auto-resolve: keep ours for known custom files
    for f in "${OUR_FILES[@]}"; do
        if echo "$CONFLICTS" | grep -q "^${f}$"; then
            echo "Keeping ours: $f"
            git checkout --ours "$f" && git add "$f"
        fi
    done

    # for Cargo.lock, accept theirs (will regenerate)
    if echo "$CONFLICTS" | grep -q "Cargo.lock"; then
        echo "Accepting theirs for Cargo.lock (will regenerate)"
        git checkout --theirs codex-rs/Cargo.lock && git add codex-rs/Cargo.lock
    fi

    # check remaining conflicts
    REMAINING=$(git diff --name-only --diff-filter=U 2>/dev/null || true)
    if [ -n "$REMAINING" ]; then
        echo "REMAINING CONFLICTS need manual resolution:"
        echo "$REMAINING"
        echo "Attempting claude merge..."

        # try claude if available, with timeout
        if command -v claude &>/dev/null; then
            timeout 1800 claude -p --dangerously-skip-permissions --model sonnet --verbose \
                "Resolve remaining merge conflicts in this repo. Keep our custom features (yolo flags, auto-next, codex-infinity branding). Accept upstream structural changes. Files: $REMAINING" \
                2>&1 || {
                echo "Claude merge failed or timed out. Manual intervention needed."
                git merge --abort 2>/dev/null || true
                exit 1
            }
        else
            echo "claude not available. Manual merge needed."
            git merge --abort
            exit 1
        fi
    fi
fi

# verify our customizations survived
echo "Verifying customizations..."
grep -q "codex-infinity" codex-cli/package.json || echo "WARN: codex-infinity branding missing from package.json"
grep -q "auto_next_steps" codex-rs/tui/src/cli.rs || echo "WARN: auto_next_steps flag missing"
grep -q "yolo2" codex-rs/tui/src/cli.rs || echo "WARN: yolo2 flag missing"

# build
echo "Building codex-tui..."
cd "$REPO_DIR/codex-rs"
if cargo build --release -p codex-tui 2>&1; then
    echo "Build succeeded"
else
    echo "Build failed. Aborting merge."
    cd "$REPO_DIR"
    git merge --abort 2>/dev/null || git reset --hard HEAD~1
    exit 1
fi

# copy binary
VENDOR_DIR="$REPO_DIR/codex-cli/vendor/x86_64-unknown-linux-gnu/codex"
mkdir -p "$VENDOR_DIR"
cp "$REPO_DIR/codex-rs/target/release/codex" "$VENDOR_DIR/codex"
echo "Binary copied to vendor"

# bump patch version
cd "$REPO_DIR"
CURRENT_VERSION=$(node -p "require('./codex-cli/package.json').version")
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
NEW_VERSION="$MAJOR.$MINOR.$((PATCH + 1))"
cd codex-cli
npm version "$NEW_VERSION" --no-git-tag-version
cd "$REPO_DIR"
echo "Version bumped: $CURRENT_VERSION -> $NEW_VERSION"

# commit
git add -A
git commit -m "Codex Infinity v${NEW_VERSION} - merge upstream + maintain custom features"
echo "Committed merge"

# push
git push origin main 2>&1 || echo "WARN: git push failed"

# npm publish
cd "$REPO_DIR/codex-cli"
npm publish --access public 2>&1 || echo "WARN: npm publish failed (may need auth)"

echo "=== Sync completed at $(date) ==="

# keep only last 30 logs
ls -t "$LOG_DIR"/upstream-sync-*.log 2>/dev/null | tail -n +31 | xargs -r rm
