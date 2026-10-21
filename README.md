# Live2D-Ai

**v0.3.1** — a minimal, general-purpose **Live2D avatar ↔ AI dialogue** platform.

Live2D-Ai wires a single core loop and keeps everything else behind a Mod boundary:

**text → LLM (chat only) → TTS → lip-sync → Live2D render + Web UI**

It ships one **white (bai) model** so a fresh clone renders out of the box; the terms in [`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md) apply and are **not** an author authorization to redistribute. Import any other Live2D assets yourself, lawfully; models you import stay in your local model library and are **not** written into the repo. Complex extras (external input, persona, voice input, session memory, …) are optional Mods: each lives in its own local repo and is **not** compiled into the default build — the core loop runs without any Mod.

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
- Trait registry (`live2d-ai-mod-system`) stays in the core: enable / disable / restart at runtime.
- **The default build compiles 0 Mod factories** (`AVAILABLE_MOD_FACTORIES` is empty;
  `default_mods_manifest` has an empty `mods` object), so no Mod — not even
  `external-input` or `local-tts-melo` — is enabled out of the box.
- Seven factories exist, each in its **own public repo** (not cloned with the core,
  not referenced by the core's Cargo manifests). Addresses are in
  [`docs/architecture/CURRENT.md`](docs/architecture/CURRENT.md) §8:

  | Repo (this machine) | crate | id |
  | --- | --- | --- |
  | `/home/skystar/live2d-ai-mod-external-input` | `live2d-ai-mod-external-input` | `external-input` |
  | `/home/skystar/live2d-ai-mod-persona` | `live2d-ai-mod-persona` | `persona` |
  | `/home/skystar/live2d-ai-mod-voice-input` | `live2d-ai-mod-voice-input` | `voice-input` |
  | `/home/skystar/live2d-ai-mod-memory` | `live2d-ai-mod-memory` | `memory` |
  | `/home/skystar/live2d-ai-mod-local-tts` | `live2d-ai-mod-local-tts` | `local-tts-melo` (`MELO_FACTORY`) |
  | `/home/skystar/live2d-ai-mod-local-engine` | `live2d-ai-mod-local-engine` | `local-engine` |
  | `/home/skystar/live2d-ai-mod-a2a` | `live2d-ai-mod-a2a` | `a2a` |

- **Mounting is manual**: add the crate's `path` dependency back to the root
  `[workspace.dependencies]` and `live2d-ai-desktop`, re-add the factory to
  `AVAILABLE_MOD_FACTORIES` (plus count / id assertions) and restore that Mod's host
  routes / integration. There is **no dynamic loader**; none of this is in the default
  commit. See [`docs/architecture/two-repo-mods.md`](docs/architecture/two-repo-mods.md).
- While a Mod is unmounted its routes answer **404** with
  `error.code = "mod_disabled"`, without calling the LLM or `say`:
  `POST /api/v1/external/chat`, `POST /api/v1/voice/transcript`,
  `GET /.well-known/agent-card.json`, `POST /api/v1/a2a`.
- `local-engine` (0.3.4) is **spawn-only**: it starts a process and never writes
  `[tts]` / `[llm]`, probes, or swaps endpoints.
- `local-tts-melo` only launches the external Melo process (its `melo/` lives in the
  local-tts repo); it never writes `[tts]`. The sealed CosyVoice3 engine (`engine/`)
  also lives there and must **not** be re-registered until it is adapted.
- **`director` is not a Mod any more**: the rule table lives in the core, the director
  crate exited `AVAILABLE_MOD_FACTORIES`, no longer delivers cues, and must **not** be
  re-registered. The per-sentence second director path is a core resident: always on, no
  enable switch. Current contract: `docs/architecture/CURRENT.md` §4.
- Voice input: a local ASR **sidecar** (any process you run) POSTs transcripts to the
  (mounted) `POST /api/v1/voice/transcript`; no ASR runtime is linked into the binary.
- Mod failure disables that Mod only; the core loop keeps running.
- `wallpaper` / `pet-desktop` / `local-llm` are **ARCHIVED / deleted** and must **not**
  be re-registered; see [`docs/architecture/ARCHIVED-mods.md`](docs/architecture/ARCHIVED-mods.md).
  The user-facing stage/shell background (`DisplayPrefs`) is **kept** — it is not the
  wallpaper Mod.
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

**Core alone:** cloning the core and filling `[llm]` / `[tts]` is enough — no Mod repo is
needed to build or run it (with cloud TTS, not even Melo).

`flutter` is the Flutter SDK in `~/flutter` (`scripts/ignite.sh` puts `$HOME/flutter/bin` on `PATH`).
`./scripts/ignite.sh --build` builds the **debug backend** and starts it; it does **not** rebuild Flutter
when `shell/flutter/build/web/index.html` already exists, so the first line must come first. The script
then waits until `GET /` returns 200 (the launcher page; a pre-launcher build that still answers 302
falls back to `GET /app/` = 200) and only then prints `http://127.0.0.1:<port>/` and opens it in the
default browser. It no longer waits for a local TTS (`GET {tts}/models`) — that 600-second wait was
removed in 0.3.11. The script does not exit; run `./scripts/ignite.sh --check` from another terminal.

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
hence line 1 first. The script then waits until `GET /` returns 200 (the launcher page) and only then prints the URL and opens it
in the default browser. It no longer waits for a local TTS — the 600-second `GET {tts}/models` wait
was removed in 0.3.11; probe it from another terminal:

```bash
./scripts/ignite.sh --check    # health-probe an already running server
```

Then open <http://127.0.0.1:18080/> — the launcher page. Tick which already-compiled Mods to
confirm, flip 预启动 (pre-start) if you want the process-spawning ones launched, click 进入, and it
hands over to `/app/` (opening `/app/` directly still works and does not bounce back).
0.3.12 adds a read-only folder list to the same page: `mods/<id>/mod.toml` under the process
working directory is validated and shown as discovered / unavailable (with a reason). Preflight
only — nothing is executed, no process is spawned, and a folder is never added to the compiled
factory table; ticking one on 进入 only records the check in `mods.json`.
`shell/flutter/build/web` is the single source of truth for the front-end artifacts;
keep `LIVE2D_AI_FLUTTER_WEB_DIR` a **WSL path** (the script sets it for you).

`--check` asserts `GET /` = 200 launcher page (body contains `live2d-ai-launcher`), `GET /app/` = 200,
and that neither the launcher page nor the served `index.html` / `main.dart.js` / `flutter_bootstrap.js`
reference `gstatic.com/flutter-canvaskit` (offline red line — a CDN build means a white screen without
network).

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
  live2d-ai-mod-*          director (retired) / template; Mod implementations live in their own repos
```

Docs index: [docs/README.md](./docs/README.md)

---

## License & assets

- **Project code:** [AGPL-3.0-only](LICENSE) © 2026 Sakura Motion Project — the **only**
  public license for project-owned code. Commercial use is allowed as long as the AGPL
  terms are met; no extra license is needed.
- **Avoiding the AGPL source obligations?** That is **not** a second public license: it
  requires commercial payment plus a **separate written agreement**. Contact QQ
  `338438383` — see [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md).
- **Third-party:** see [NOTICE](NOTICE) and [CREDITS.md](CREDITS.md)
- **Live2D models / textures:** not covered by AGPL; obtain and import lawfully. The **white model's few required files are tracked** (`bai.moc3`, `texture_00_4096.png`); the terms in [`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md) apply and are **not** an author authorization to redistribute. Imported models stay in your local model library and are not written into the repo. Scope notes: [LICENSE-SCOPE.md](LICENSE-SCOPE.md)

Render stack uses open-source Ayagami — not Live2D Inc.’s proprietary Cubism Core SDK. If you add third-party Cubism binaries yourself, those stay under Live2D’s own EULA and are never part of this project’s license.
