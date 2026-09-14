# Mod parallel protocol (mod/<name> branches)

> Baseline: v0.2.0-rc.1 = 2d492447. No dynamic loading.
> Many workers on short-lived branches; one integrate PR registers FACTORIES.

## 0. Rule

Own your crate only: `crates/live2d-ai-mod-<id>/` + plans/docs.
Do NOT edit AVAILABLE_MOD_FACTORIES / mod_count_* / default_mods_manifest.

## 1. Tech debt

Mod contract + event isolation are ready. No blocking debt.
Known product debt: listen_port display-only, pet skeleton, persona polish, Action dormant.
Verdict: GO — finish mods step by step / in parallel waves.

## 2. What "parallel" means

| Kind | OK? | How |
|---|---|---|
| A. Dev parallel (branches/workers) | yes | this protocol |
| B. Runtime multi-Mod enable | yes | already (Failed isolated) |
| C. Separate process / sub-repo | sidecar only | e.g. bilibili py; official Mods stay static-linked |

No dynamic .so marketplace.

## 3. Conflict red lines

Forbidden for workers: FACTORIES, mod_count, default manifest, bumping global version.
Allowed: own crate; append one members line; desktop path dep; REGISTER-<id>.md for integrator.

## 4. Waves

Wave 1 (open now, ~1h each):
- mod/voice-input — new crate voice->text->say
- mod/wallpaper — strategy v0 (do NOT redo framebuffer)
- mod/persona-polish — polish existing persona only

Wave 2:
- mod/memory-v0
- mod/director-rfc (RFC + skeleton; no Action revive)
- mod/pet-desktop (solo merge preferred)

Closed: local-llm abolished; external-input + bilibili sidecar (rc.1).

## 5. Integrate

PRs -> apply REGISTER-*.md -> gate green -> 0.2.0-rc.2 (or Wave1=rc.2, Wave2=rc.3).

Local branches already created at 2d492447: mod/voice-input, mod/wallpaper, mod/persona-polish.
