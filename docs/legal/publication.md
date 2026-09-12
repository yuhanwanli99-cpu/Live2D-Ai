# Publication policy (Rust baseline)

Applies to the current public tree around **v0.1.0-rc.1** (Rust workspace under `crates/`).

## Must exclude from public release archives / git publish tree

- Live2D model originals: `.moc3`, textures, and other raw model binaries under `assets/models/` (except README / placeholders).
- Secrets: `.env`, provider keys, tokens, chat history, caches, crash dumps, machine-local absolute paths.
- Live2D Cubism proprietary Core / Web Core binaries (not part of this baseline; never commit).

Generic loaders, adapters, tests that **skip** when assets are missing, and attribution docs may remain. Users import models locally.

## Required gates (intent)

1. `cargo test --workspace --all-targets` (and project CI workflows) pass on the published revision.
2. Secret scan clean on history used for publication.
3. Archive / tree contains none of the excluded paths above.
4. README states platform scope (Linux / WSL2 primary) and that models are not bundled.

## Legacy stacks

Python PC Preview and Android publication rules are historical. See [legacy/README.md](legacy/README.md).
