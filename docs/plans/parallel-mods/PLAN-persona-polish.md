# PLAN — mod/persona-polish (Wave 1)

Branch: mod/persona-polish @ 2d492447. Budget ~1h.
Only edit crates/live2d-ai-mod-persona/.

## Goal

Polish card path: config -> enable -> system_prompt change -> disable restore; bad input errors clearly.

## Must

1. Tests for card_path/card_json -> enable -> apply_settings system_prompt -> disable restore baseline
2. Bad JSON / missing file / oversized: explicit error, no panic
3. Clarify enable semantics (manifest enabled is sole source of truth)
4. REGISTER-persona-polish.md (note: no new FACTORIES row)

## Forbidden

Persona fields back into core [persona]; FACTORIES; other Mods.

## Gate

cargo test -p live2d-ai-mod-persona green.
