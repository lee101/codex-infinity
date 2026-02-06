# Authentication

For information about Codex CLI authentication, see [this documentation](https://developers.openai.com/codex/auth).

## Anthropic (Claude)

Codex includes a built-in `anthropic` provider that uses Anthropic's OpenAI-compatible endpoint.
Set `ANTHROPIC_API_KEY` (standard API key) or `ANTHROPIC_OAUTH_TOKEN` (Claude Code OAuth token),
then configure `model_provider = "anthropic"` in `config.toml` (for example, with
`model = "claude-opus-4-6"`).
