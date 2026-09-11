# Contributing to Live2D-Ai

The current public preview scope is the PC client under `Live2D-Ai-pc/open-llm-vtuber`.
Android changes may remain in the development monorepo but are not part of the PC Preview support promise.

## Development setup

```bash
cd Live2D-Ai-pc/open-llm-vtuber
uv sync --locked
uv run pytest tests/
cd renderer
npm ci
npm test
npm exec tsc -- --noEmit
npm run build
```

Do not commit API keys, chat history, uploaded backgrounds, generated audio, model files without redistribution permission, or local environment files.

## Pull requests

- Keep HTTP endpoints thin and put business behavior in services.
- Keep `routes.py` below the architecture guard threshold; add domain routers instead.
- Add tests for behavior changes.
- Update both configuration templates for configuration schema changes.
- Preserve third-party copyright and license notices.
- Explain user-visible behavior and migration impact in the PR description.

Contributions to project-owned code are accepted under AGPL-3.0-only. By submitting a contribution, you confirm that you have the right to provide it under that license.
