# Live2D-Ai

**v0.1.0-rc.2** — a minimal, general-purpose **Live2D avatar ↔ AI dialogue** platform.

Live2D-Ai wires a single core loop and keeps everything else behind a Mod boundary:

**text → LLM (chat only) → TTS → lip-sync → Live2D render + Web UI**

It does **not** ship or bind any character, skin, or model. You import your own lawfully obtained Live2D assets. Complex extras (desktop pet, external input, local LLM, …) are optional Mods that fail independently of the core loop.

> Chinese: [README.zh-CN.md](./README.zh-CN.md)

---

## Features

### Core loop
- **LLM chat only** — OpenAI-compatible `/chat/completions` (SSE). No tools / function calling in the request body (regression-tested).
- **TTS as core** — OpenAI-compatible `/audio/speech`; endpoint comes from `[tts]` in `live2d-ai.toml`.
- **Sentence units** — split on real sentence boundaries; lip-sync driven by per-sentence audio + volume envelope.
- **Pure reducer core** (`live2d-ai-core`) — epoch gates + latches, no I/O, no async.
- **Supervisor** — dedicated thread + event loop for turns.

### Live2D + UI
- **Renderer** — [Ayagami](https://github.com/AyagamiDev/ayagami) (MIT OR Apache-2.0) + wgpu; **no** Live2D Cubism proprietary SDK in the Rust baseline.
- **Web UI** — primary entry (`--web`); stage + chat layout, settings panels, offline-friendly Flutter Web build gates.
- **WASM render path** — `/render` + `/models/*` via `l2d-wasm-demo`.

### Mods
- Trait registry (`live2d-ai-mod-system`): enable / disable / restart at runtime.
- Bundled Mods (compiled in): external input, pet-desktop (skeleton), local LLM.
- Mod failure disables that Mod only; the core loop keeps running.

### Platform (this baseline)
- Primary: **Linux / WSL2**.
- Android and native Windows desktop are out of scope for the `0.1.0` RC line.

---

## Quick start

| Tool | Notes |
|---|---|
| Rust | MSRV **1.92** |
| `wasm32-unknown-unknown` | `rustup target add wasm32-unknown-unknown` |
| trunk | WASM builds (`cargo install trunk`) |

```bash
cp live2d-ai.toml.example live2d-ai.toml   # OpenAI-compatible LLM/TTS
cargo build --release
./target/release/live2d-ai-desktop --web --http-port 18080
# open http://localhost:18080/
```

Put Live2D model files under `assets/models/` locally (see `assets/models/README.md`). Nothing under that path is shipped in git.

### Useful CLI modes
```bash
--web [--http-port P]   # Web UI (main entry)
--chat                  # Terminal dialogue loop
--model-smoke [P]       # Render smoke
--audio-smoke           # Audio smoke
--benchmark [P]         # Render benchmark
```

### Ignition (WSL2 → Windows browser)

Development and the **server process** live in WSL2; Windows only opens a browser
(no binaries, no Flutter builds on the Windows side).

```bash
./scripts/ignite.sh            # preflight + serve on port 18080
./scripts/ignite.sh --build    # also rebuild Rust + Flutter Web first
./scripts/ignite.sh --check    # health-probe an already running server
```

Then open <http://127.0.0.1:18080/app/> (`/` 302-redirects to `/app/`).
`shell/flutter/build/web` is the single source of truth for the front-end artifacts;
keep `LIVE2D_AI_FLUTTER_WEB_DIR` a **WSL path** (the script sets it for you).

`--check` asserts `GET /` = 302 → `/app/`, `GET /app/` = 200, and that the served
`index.html` / `main.dart.js` contain **no** `gstatic.com/flutter-canvaskit` reference
(offline red line — a CDN build means a white screen without network).

### Tests
```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Architecture (crates)

```
crates/
  l2d                      Live2D render (Ayagami + wgpu)
  live2d-ai-core           Pure reducer state machine
  live2d-ai-runtime        LLM / TTS / conversation / settings
  live2d-ai-desktop        Supervisor + Web API + desktop shell
  l2d-wasm-demo            Web WASM render
  live2d-ai-mod-system     Mod traits / registry
  live2d-ai-mod-*          Optional Mods
```

Docs index: [docs/README.md](./docs/README.md)

---

## License & assets

- **Project code:** [AGPL-3.0-only](LICENSE) © 2026 Sakura Motion Project  
  Optional proprietary terms: [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md)
- **Third-party:** see [NOTICE](NOTICE) and [CREDITS.md](CREDITS.md)
- **Live2D models / textures:** not covered by AGPL; obtain and import lawfully. This repo does **not** redistribute `.moc3` or texture binaries. Scope notes: [LICENSE-SCOPE.md](LICENSE-SCOPE.md)

Render stack uses open-source Ayagami — not Live2D Inc.’s proprietary Cubism Core SDK. If you add third-party Cubism binaries yourself, those stay under Live2D’s own EULA and are never part of this project’s license.
