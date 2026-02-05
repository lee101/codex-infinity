# Getting started with Codex CLI

For an overview of Codex CLI features, see [this documentation](https://developers.openai.com/codex/cli/features#running-in-interactive-mode).

## Codex Infinity add-on backups

Use `codex infinity` to manage add-on backups via the Codex Infinity control plane:

```bash
codex infinity addons backups owner/repo --type postgres
codex infinity addons backups owner/repo --type postgres --json
codex infinity addons events owner/repo --type postgres --limit 50
codex infinity addons events owner/repo --type postgres --event-type restore --json
codex infinity addons restore owner/repo --type postgres --object-key addon-backups/addon-id/2026/02/05/backup.dump --yes
codex infinity addons restore owner/repo --type postgres --object-key addon-backups/addon-id/2026/02/05/backup.dump --yes --json
```

Set `CODEX_INFINITY_API_KEY` (and optionally `CODEX_INFINITY_BASE_URL`) to authenticate.
