# PLAN — mod/wallpaper (Wave 1)

Branch: mod/wallpaper @ 2d492447. Budget ~1h.

## Goal

Wallpaper strategy v0: when to change / sync with stage-shell. Do NOT redo rc.5 GPU framebuffer.

## Must

1. New crate live2d-ai-mod-wallpaper from template
2. members + desktop path; NO FACTORIES
3. settings_spec: mode (off|follow_stage|interval), interval_secs
4. Pure strategy state machine + unit tests; real image swap via existing DisplayPrefs/stage-bg (placeholder OK if documented)
5. docs/architecture/wallpaper-mod-v0.md
6. REGISTER-wallpaper.md

## Forbidden

Edit l2d-wasm-demo / stage_bg / framebuffer; carousel; FACTORIES.

## Gate

crate tests green.
