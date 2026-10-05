# BATCH-0637 · crates 真实清单（14 个 crate）+ 一条 P1

Phase 1 · 域覆盖 · Mod crates（首次进这个域）

## 跑的命令（全部只读）
```
git ls-files "crates/live2d-ai-mod-*/" | wc -l      # => 0（路径写法错）
git ls-files crates/ | sed 's|crates/\([^/]*\)/.*|crates/\1|' | sort -u
git ls-files crates/live2d-ai-mod-director/
grep -n "mod-director" Cargo.toml crates/live2d-ai-desktop/Cargo.toml
```

## 我自己的错（先记）
账上写「Mod crates 42 个文件」**错**；真实是 **14 个 crate**，而我那条路径写法 **零命中**
⇒ B0440 第 1 种形态（路径写法不对）**又一次在我自己身上生效**。

## 14 个 crate
l2d / l2d-wasm-demo / live2d-ai-core / live2d-ai-desktop / live2d-ai-runtime
/ live2d-ai-mod-{director, external-input, local-llm, memory, persona,
pet-desktop, system, template, voice-input, wallpaper}

前五个已审（web_api 49/49 结项）；voice-input 核过 gate.rs；其余**未审**。

## 本批产出
**F-0637-01 (P1)** —— `live2d-ai-mod-director` 在 workspace `members` 里、
被 `live2d-ai-desktop` 直接依赖、5 个源文件在 git 里，而 AGENTS 写它「**已删除**」。
（详见 FINDINGS.md）

## 未核
其余 9 个 mod crate 的本体 · director 的 lib.rs 头注 · `AVAILABLE_MOD_FACTORIES` 的当前内容 ·
local-llm 的 DEPRECATED 标注（AGENTS 记「crate 暂留并标 DEPRECATED」）· F-0469-01 那条 `Cargo.toml` description 的同款手法
