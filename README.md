# Live2D-Ai

**v0.2.4-rc.1** — a minimal, general-purpose **Live2D avatar ↔ AI dialogue** platform.

Live2D-Ai wires a single core loop and keeps everything else behind a Mod boundary:

**text → LLM (chat only) → TTS → lip-sync → Live2D render + Web UI**

It ships one **white (bai) model** so a fresh clone renders out of the box; the terms in [`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md) apply and are **not** an author authorization to redistribute. Import any other Live2D assets yourself, lawfully. Complex extras (external input, persona, voice input, session memory, …) are optional Mods that fail independently of the core loop.

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
- Mods compiled into the binary (**6 registered**): `external-input`, `persona`,
  `voice-input`, `memory`, `director`, `local-tts-melo`. **`external-input` and
  `local-tts-melo` are enabled by default** (the latter launches the bundled MeloTTS on
  boot); the other four are opt-in (`memory` writes `persona.system_prompt`, so it must
  be turned on deliberately).
- `local-tts` (CosyVoice3) is **sealed — not registered**; the crate stays in the tree
  but must **not** be re-registered until it is adapted.
- `wallpaper` and `pet-desktop` are **ARCHIVED**: **not registered and not compiled
  into the binary**; the crates were **physically deleted on 2026-10-01** and now only
  exist in tag `checkpoint/pre-d1-dormant`. They must **not** be re-registered. See
  [`docs/architecture/ARCHIVED-mods.md`](docs/architecture/ARCHIVED-mods.md). The
  user-facing stage/shell background (`DisplayPrefs`) is **kept** — it is not the
  wallpaper Mod.
- `local-llm` is **no longer started** (removed from the registry in `0.2.0-rc.1`; the
  crate itself was deleted on 2026-10-01 and now only exists in tag
  `checkpoint/pre-d1-dormant`). The action-driving `director` crate was removed in
  `0.1.0-rc.2`; its archive branch `archive/action-layer-p6` **no longer exists**
  (re-verified 2026-10-06: not local, not on the remote, not in the historical
  bundles) — recover the deleted sources from `main`'s own history:
  `git show 98469df^:crates/live2d-ai-mod-director/src/lib.rs`.
- The `director` registered today (2026-09-14, same name, different job) **does drive
  the stage**: its rule layer maps **user input** to an action preset and broadcasts the
  per-sentence WS `action_cue` (the only stage driver; a neutral turn emits the
  `preset_id=="none"` revoke sentinel). `latest.preset_id` is panel-read-only (the
  front-end pull-to-drive channel was retired, D12); the async second LLM (priority 40)
  is **off by default** and its real HTTP client is wired (`staging_http.rs`).
  Contract: `docs/architecture/director-rfc.md`; current status: `AGENTS.md`
  §「动作与表演的现行状态（2026-09 实测）」.
- Voice input: a local ASR **sidecar** (any process you run) POSTs transcripts to
  `POST /api/v1/voice/transcript`; no ASR runtime is linked into the binary.
- Mod failure disables that Mod only; the core loop keeps running.
- Contract, "add a Mod" checklist and the stable-release (Rust/C) rule:
  [`docs/architecture/mod-product-chain.md`](docs/architecture/mod-product-chain.md).

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
(cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build
```

`flutter` is the Flutter SDK in `~/flutter` (`scripts/ignite.sh` puts `$HOME/flutter/bin` on `PATH`).
`./scripts/ignite.sh --build` builds the **debug backend** and starts it; it does **not** rebuild Flutter
when `shell/flutter/build/web/index.html` already exists, so the first line must come first. The server
`exec`s and does not exit — run `./scripts/ignite.sh --check` from another terminal, then open
<http://127.0.0.1:18080/app/>.

The white (**bai**) model ships with the repo, so a fresh clone renders out of the box — see
[`assets/models/README.md`](assets/models/README.md), terms in
[`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md) (the author's terms, **not** a
redistribution authorization).

### Useful CLI modes

The main entry (what `scripts/ignite.sh` serves, and what acceptance targets):

```bash
--web [--http-port P]   # Web UI — the main entry
```

The native second shell is **gone** (removed from the build on 2026-10-01, D1/W2-B):
`--chat` / `--window-smoke` / `--model-smoke` / `--pet-mode` / `--benchmark*` no longer
exist. What remains besides `--web` is the audio smoke:

```bash
--audio-smoke [--audio-smoke-secs S] [--audio-smoke-silence]   # Audio smoke (tool)
```

Recovery (files + the 111 exiting tests, by name) and the full ledger live in
[docs/architecture/ARCHIVED-native-shell.md](./docs/architecture/ARCHIVED-native-shell.md);
the dormancy ledger is in [AGENTS.md](./AGENTS.md).

### Ignition (WSL2 → Windows browser)

Development and the **server process** live in WSL2; Windows only opens a browser
(no binaries, no Flutter builds on the Windows side).

Standard build + start (from the repo root; order matters):

```bash
(cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build
```

`flutter` is the SDK in `~/flutter`. `./scripts/ignite.sh --build` builds the **debug backend** and
starts it; it does **not** rebuild Flutter when `shell/flutter/build/web/index.html` already exists —
hence line 1 first. The last line `exec`s into the server and does not exit; probe it from another
terminal:

```bash
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
- **Live2D models / textures:** not covered by AGPL; obtain and import lawfully. The **white model's few required files are tracked** (`bai.moc3`, `texture_00_4096.png`); the terms in [`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md) apply and are **not** an author authorization to redistribute. Scope notes: [LICENSE-SCOPE.md](LICENSE-SCOPE.md)

Render stack uses open-source Ayagami — not Live2D Inc.’s proprietary Cubism Core SDK. If you add third-party Cubism binaries yourself, those stay under Live2D’s own EULA and are never part of this project’s license.
