# LICENSE-SCOPE — what each part is licensed under

This document describes the **current Rust baseline** (`crates/`, `0.1.0-rc.1`).  
Project-owned code defaults to **AGPL-3.0-only** ([LICENSE](LICENSE)). Third-party code and assets keep their own terms. Commercial use is allowed when AGPL-3.0 obligations are met; for proprietary use without those obligations, see [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md).

Legacy Python / Android stack notes (historical only): [docs/legal/legacy/README.md](docs/legal/legacy/README.md).

## 1. Project code (Rust baseline)

| Scope | License | Notes |
|---|---|---|
| Workspace crates under `crates/` (except where a crate overrides) | **AGPL-3.0-only** | See root `LICENSE` and `Cargo.toml` `workspace.package.license` |
| `crates/l2d` | **MIT OR Apache-2.0** | Render wrapper; see that crate’s `LICENSE-*` |
| `xtask`, root tooling committed in-tree | **AGPL-3.0-only** unless marked otherwise | |

Contributions of project-owned code are licensed AGPL-3.0-only by default.

## 2. Live2D rendering (Rust baseline)

| Component | License | Distributed in this repo? |
|---|---|---|
| [Ayagami](https://github.com/AyagamiDev/ayagami) (+ wgpu) | MIT OR Apache-2.0 | As a dependency (pin / crates.io / git), not Cubism Core |
| Live2D Cubism proprietary Core / Web Core | Live2D Proprietary Software License | **Not** part of the Rust baseline; **not** shipped here |

If you download Cubism binaries yourself for experiments, they remain under Live2D’s EULA and must not be committed to this repository.

## 3. Models and media

- Live2D model binaries (`.moc3`, textures, etc.) are **not** covered by this project’s AGPL.
- `assets/models/*` is gitignored except placeholders / README — **no public redistribution** of raw model files via this repo.
- Obtain models from their authors and import locally. See [assets/models/README.md](assets/models/README.md).

## 4. Secrets and local config

- No API keys or provider credentials are bundled.
- `.env`, local `live2d-ai.toml` with secrets, chat history, caches, and uploads stay on the machine — not in the publish tree.

## 5. Public tree expectations

Public source should include project code + docs needed to build the Rust baseline, and must **exclude** proprietary Cubism Core binaries, model originals, secrets, and local absolute-path machine state.
