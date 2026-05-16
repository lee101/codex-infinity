#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/deploy_new_binary.sh [--dry-run] [--bump] [--no-bump] [--tag <tag>]

Builds the Rust CLI, installs the new Linux x64 binary into vendor/, verifies
the npm package payload, and publishes @codex-infinity/codex-infinity.

By default the script bumps the package patch version before publishing because
npm package versions are immutable. Dry-runs restore package.json after the
publish preview. Use --no-bump only when package.json already contains an
unpublished version.
EOF
}

dry_run=0
bump=""
npm_tag="latest"

while (($# > 0)); do
  case "$1" in
    --dry-run)
      dry_run=1
      shift
      ;;
    --bump)
      bump=1
      shift
      ;;
    --no-bump)
      bump=0
      shift
      ;;
    --tag)
      if (($# < 2)); then
        echo "error: --tag requires a value" >&2
        exit 2
      fi
      npm_tag="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$bump" ]]; then
  bump=1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
codex_cli_root="$(cd "$script_dir/.." && pwd)"
repo_root="$(cd "$codex_cli_root/.." && pwd)"
codex_rs_root="$repo_root/codex-rs"
binary_src="$codex_rs_root/target/release/codex"
binary_dest="$codex_cli_root/vendor/x86_64-unknown-linux-gnu/codex/codex"

if ((!dry_run)); then
  if ! (cd "$codex_cli_root" && npm whoami >/dev/null); then
    echo "error: npm is not authenticated; run npm login before deploying." >&2
    exit 1
  fi
fi

cd "$codex_rs_root"
cargo build --release -p codex-cli

mkdir -p "$(dirname "$binary_dest")"
install -m 755 "$binary_src" "$binary_dest"

cd "$codex_cli_root"

package_json_backup=""
restore_package_json() {
  if [[ -n "$package_json_backup" && -f "$package_json_backup" ]]; then
    cp "$package_json_backup" package.json
    rm -f "$package_json_backup"
  fi
}

if ((dry_run && bump)); then
  package_json_backup="$(mktemp)"
  cp package.json "$package_json_backup"
  trap restore_package_json EXIT
fi

if ((bump)); then
  npm version patch --no-git-tag-version
fi

CODEX_INFINITY_REQUIRED_GROUPS=linux-x64 node scripts/verify_vendor.js
node bin/codex-infinity.js --version

if ((dry_run)); then
  npm publish --access public --tag "$npm_tag" --dry-run
else
  npm publish --access public --tag "$npm_tag"
fi
