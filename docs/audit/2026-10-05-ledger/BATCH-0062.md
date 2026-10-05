# BATCH-0062 · Mod 根第 3 批：4 个在册 Mod 实现（external-input 侧）

Phase 1 · 域覆盖 · Mod 根

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-external-input/src/lib.rs` — 847（定点 112-161：`render_injected_text` /
   `render_from_config` / `token_from_config` / `env_token_is_set_with`）
2. `crates/live2d-ai-mod-external-input/src/lib.rs` — （定点 216 / 341 / 382：token 状态两态渲染）
3. `crates/live2d-ai-desktop/src/mod_registry.rs` — （定点 455-470：`enable_with_config` / `reload_config`）
4. `crates/live2d-ai-desktop/src/web_api/mods_routes.rs` — （定点 157-166 / 250-266：config 动作）
5. `shell/flutter/lib/settings/sections/dev_tools_section.dart` — （定点 700-712：secret 守卫）
6. `shell/flutter/lib/api/mods_api.dart` — （定点 14-30：`secret` 契约声明）

## 跑过的命令（全部只读）
```
grep -n "fn token_from_config|fn render_from_config|fn render_injected_text|sanitize|trim()" external-input/src/lib.rs
sed -n '112,161p' external-input/src/lib.rs
grep -rn "setConfig" shell/flutter/lib/
grep -n "read_settings" -A 14 cli_entry.rs
sed -n '700,712p' dev_tools_section.dart ; sed -n '14,30p' mods_api.dart
sed -n '455,470p' mod_registry.rs ; grep -n "reload_config" -B 2 -A 18 mods_routes.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0062-01（P2）= 已登记 S3 的「升级」**
机制已定位：**前端省略 secret 键 + 宿主整份替换 ⇒ 键被删除**。
两侧各自都「看起来对」，是**接缝缺陷**（模式 C），不是哪一侧写错。
并追加了登记表没说的后果：**token 只存于 config 时，抹掉它 ⇒ 注入端点鉴权直接失效**
（链到 B0005 已核的 `external_input_gate` / `check_token`）。界面回**成功**文案，无任何信号。
详见 FINDINGS 四段（摘录/调用链/影响/反证）。

## 维度覆盖（本批）
- 红线 R（Mod 边界与秘密）：**命中** —— secret 的**保存**路径
- 维度 E：spec 驱动的表单 + `secret` 字段语义
- 维度 G：token 值**从不进 Mod 进程**（`env_token_is_set_with` 只问存在性，`lookup` 可注入，
  「绝不读取/回显明文」），这半边是对的 —— 错的是**保存**时把它删了
- 模式 C（接缝缺陷）：**第 2 例**（第 1 例是 F-0006-01 的 `redacted_config` 与 schema 的脱敏契约）

## 未核实项
1. `voice-input` / `persona` / `memory` 的 `settings_spec` 与 config 保存路径未逐个核 ——
   **同一形态的洞可能在其余三个 Mod 上同样成立**（凡有 `secret:true` 字段的），
   **本批只核了 `external-input` 一个**。`persona` 的 `api_key_env` 类字段、
   `voice-input` 的 `token` 都需核
2. `mod_registry.rs:1252/1289` 的测试为何用 `apply_settings: |_| true` 桩（是否因此漏掉合并回归）未追
3. `ModSessionPrompts` 生产实现（`supervisor.session_scopes().as_mod_prompts()`）仍未读
4. `mod-system/src/{topics,settings,factory}.rs` 未读

## 本批新增
P0 0 · P1 0 · **P2 1**（已登记 S3 的升级）· P3 0
