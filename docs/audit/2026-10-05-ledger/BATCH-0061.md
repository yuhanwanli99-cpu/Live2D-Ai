# BATCH-0061 · Mod 读通道 + 会话隔离（保持在同一根）

Phase 1 · 域覆盖 · Mod 根（第 2 批）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-system/src/session.rs` — 476（定点 96-120：`sanitize_session_id` / `owner_merge_rank`）
2. `crates/live2d-ai-mod-system/src/services.rs` — （定点 180-202：`ModSettingsReader` 契约）
3. `crates/live2d-ai-desktop/src/mod_registry.rs` — （定点 358-408：reader/applier 闭包装配）
4. `crates/live2d-ai-desktop/src/web_api/cli_entry.rs` — （定点 273-287：`read_settings` 生产实现）
5. `crates/live2d-ai-mod-memory/src/lib.rs` — 895（定点 700-729：`on_scoped_event` 二次归一）
6. `crates/live2d-ai-mod-memory/src/strategy.rs` — （定点 570-574：`resolve_session_store_path`）
7. `crates/live2d-ai-mod-memory/src/strategy_tests.rs` — （定点 290-302：路径回归）

## 跑过的命令（全部只读）
```
grep -n "fn sanitize_session_id" -A 22 mod-system/src/session.rs
grep -n "sanitize_session_id(" mod-system/src/session.rs
sed -n '379,408p' mod_registry.rs ; sed -n '358,379p' mod_registry.rs
grep -n "read_settings" -A 14 cli_entry.rs
grep -n "jsonl|PathBuf|join(|sanitize|session" mod-memory/src/lib.rs
grep -rn "fn resolve_session_store_path" -A 22 mod-memory/src/*.rs
sed -n '700,729p' mod-memory/src/lib.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**（三项核验通过 + 两条假设被自己推翻）
1. `ModSettingsReader` 读通道**干净且契约为真**（返回 `SettingsView`，无 `api_key_env` 字段、
   `has_api_key` 是布尔）⇒ 密钥与变量名都不出门
2. ⭐ **对照结论**：同一条 Mod↔host 边界，**读侧严格、写侧失败开放**（F-0060-01）——
   两半严格程度不一致 ⇒ 不是「整体松」，是写侧那一次实现漏了考虑
3. 会话 id → 文件名**三重归一**（宿主 / `on_scoped_event` / `resolve_session_store_path`），
   非法 id 退回全局桶 + warn；**路径穿越不成立**（id 永远是文件名的一部分，`/` `\` 被白名单排除）
4. 附带：`AssistantReplied` 直接用本轮 id 选桶，**不依赖 `current_session`** ⇒ 避开切会话瞬间的错位竞态

## 未核实项
1. `mod-system/src/{topics,settings,factory,error,status,lib,descriptor,registry}.rs` 未读
2. 4 个在册 Mod 的 `settings_spec` 未逐个核（`external-input` 的 `token` 为 secret，B0045 已核过一处）
3. `ModSessionPrompts` 的**生产实现**（`supervisor.session_scopes().as_mod_prompts()`）未读
   —— 「按会话分桶不串味」的实现面
4. `template` crate（新 Mod 起点）未读
5. memory Mod 的**读**路径（会话桶检索）是否同样只认 `current_session`，未核

## 本批新增
**0 条**（净产出：读/写两半姿态的对照结论 + 两条被自己推翻的路径穿越假设）
