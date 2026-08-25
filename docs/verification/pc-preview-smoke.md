# PC v0.2.0-preview.1 final smoke checklist

## Automated

- [x] Build PC-only source archive with `scripts/build_pc_preview.py`.
- [x] Archive excludes model binaries, local models, runtime caches, `.env`, chat history, local backgrounds, virtual environments and `node_modules`.
- [x] Run current-tree secret patterns with `scripts/check_public_secrets.py`.
- [x] Python test suite: 440 passed in the generated public tree.
- [x] Renderer clean `npm ci`, tests, type-check and build pass without model assets.
- [x] Renderer dependency audit reports zero known vulnerabilities.
- [ ] GitHub Gitleaks full-history job passes.
- [ ] GitHub Python job completes `uv sync --locked`.

## Manual application smoke

- [x] Start from a newly generated asset-free archive on Linux using the locked Python 3.12 environment.
- [x] Without an API key, server starts on 127.0.0.1; /health, /, /api/config and /api/models return 200, and logs give an actionable key prompt without exposing a key.
- [ ] Import a lawfully obtained Live2D model and verify it renders in a browser.
- [ ] Send text and verify LLM reply, subtitle, TTS and lip sync.
- [ ] Trigger nod, shake_no and look_around; confirm visible movement and gaze returns to center.
- [ ] Interrupt playback and confirm motion/audio release without a stuck pose.
- [ ] Save settings, restart, and confirm persistence without exposing keys in API responses or logs.

Preview Go requires the two GitHub checks plus model import/render, real provider conversation/TTS/lip-sync, motion visual inspection, interrupt release, and persistence smoke.
