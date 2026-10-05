# BATCH-0063 · F-0062-01 的同形态扩散核

Phase 1 · 域覆盖 · Mod 根（第 4 批，同一根）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-voice-input/src/lib.rs` — （定点 185-200：`settings_spec` 的 `token` 字段；:234-240：`token_from_config` 及其空值语义注释）
2. 5 个在册 Mod 的 `lib.rs`（`grep -n 'secret.*true'` 逐一核）

## 跑过的命令（全部只读）
```
for m in external-input persona voice-input memory director; do
  grep -n 'secret.*true|"secret"' crates/live2d-ai-mod-$m/src/lib.rs; done
sed -n '185,200p' crates/live2d-ai-mod-voice-input/src/lib.rs
grep -rn "token_from_config" -A 12 crates/live2d-ai-mod-voice-input/src/lib.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**，但把 **F-0062-01 的范围确定下来**
扩散核：5 个在册 Mod 中 **`external-input` 与 `voice-input` 带 `secret:true` 字段**（都是 `token`），
`persona` / `memory` / `director` 无 ⇒ 前者两个受同一接缝影响，后三个不受。
⇒ 宿主侧从未实现 secret 保留语义 ⇒ **每个带 secret 字段的 Mod 都会丢**，当前 2/5。
⇒ 加重一点：`voice-input` 的**字段标签**（「访问令牌（空 = 不鉴权）」）与
**函数注释**（「空值语义是『不鉴权』而不是『用空 token 鉴权』」）**都自己写明了后果**，
而那个「空」正是本产品 UI 的保存流程会造出来的。
⇒ 修法不变（B0062 建议①），但**验收要求**新增：回归要**覆盖全部在册 Mod**
（逐个 Mod 各自测会漏掉「新增 Mod 忘了遵守」）。

## 未核实项
1. `ModSessionPrompts` 生产实现（`supervisor.session_scopes().as_mod_prompts()`）**仍未读** ——
   「按会话分桶不串味」的实现面，B0061 起挂了 3 批
2. `mod-system/src/{topics,settings,factory,error,status,descriptor,registry}.rs` 未读
3. `persona`(1004) / `director`(902) / `memory` 余段 实现本体未读
4. `template` crate 未读
5. `external-input` / `voice-input` 的 HTTP 端点门禁回归是否覆盖「token 被抹」这一态 —— 未查

## 本批新增
**0 条**（净产出：F-0062-01 的影响范围从 1 个 Mod 定到 2/5，并追加跨 Mod 验收要求）
