#!/usr/bin/env bash
# E2E smoke test: verify the Cerebras-hosted models are reachable through OpenPaths.
#
# Codex's `cerebras/*` models prefer a direct CEREBRAS_API_KEY and otherwise fall
# back to OpenPaths (openpaths.io), which also serves the Cerebras-hosted
# open-weight models. This script confirms the key works and the models respond.
#
# Usage:
#   ./scripts/e2e_cerebras_openpaths.sh
# Reads OPENPATHS_API_KEY (and optional OPENPATHS_BASE_URL) from the environment
# or from a gitignored .env at the repo root.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -f "${repo_root}/.env" ]]; then
  # shellcheck disable=SC1091
  set -a; source "${repo_root}/.env"; set +a
fi

: "${OPENPATHS_API_KEY:?Set OPENPATHS_API_KEY (e.g. in ${repo_root}/.env)}"
base_url="${OPENPATHS_BASE_URL:-https://openpaths.io}"
base_url="${base_url%/}"

models=("gpt-oss-120b" "zai-glm-4.7")
failed=0

for model in "${models[@]}"; do
  echo "== ${model} via ${base_url} =="
  resp="$(curl -sS -m 60 "${base_url}/v1/chat/completions" \
    -H "Authorization: Bearer ${OPENPATHS_API_KEY}" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"${model}\",\"messages\":[{\"role\":\"user\",\"content\":\"Reply with the single word: pong\"}],\"stream\":false}")"
  content="$(printf '%s' "${resp}" | python3 -c 'import sys,json; print(json.load(sys.stdin)["choices"][0]["message"]["content"])' 2>/dev/null || true)"
  if [[ -n "${content}" ]]; then
    echo "  OK -> ${content}"
  else
    echo "  FAIL -> ${resp}"
    failed=1
  fi
done

if [[ "${failed}" -ne 0 ]]; then
  echo "e2e: at least one Cerebras model failed via OpenPaths" >&2
  exit 1
fi
echo "e2e: all Cerebras models reachable via OpenPaths"
