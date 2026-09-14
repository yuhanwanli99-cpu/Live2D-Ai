# PLAN — mod/voice-input (Wave 1)

Branch: mod/voice-input @ 2d492447. Budget ~1h.
Protocol: PARALLEL-PROTOCOL-2026-09-14.md.

## Goal

New Mod: voice input -> text -> ModServices.say_tx. Half-finished OK. Do NOT touch FACTORIES.

## Must

1. Copy live2d-ai-mod-template -> crates/live2d-ai-mod-voice-input/
2. Root Cargo.toml members + desktop path dep (NO FACTORIES registration)
3. settings_spec: backend (mock|sidecar), locale, optional token; no second enabled
4. Pure helpers: mock ASR text clean -> say; tests
5. Optional docs/examples/voice-sidecar/ README stub
6. docs/plans/parallel-mods/REGISTER-voice-input.md for integrator

## Forbidden

FACTORIES / mod_count / default manifest / global version; revive Action; full ASR inside Rust core.

## Gate

cargo test -p live2d-ai-mod-voice-input green.
