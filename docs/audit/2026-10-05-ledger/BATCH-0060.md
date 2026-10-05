# BATCH-0060 · ⭐ 换根：Mod 边界（8 个 Mod crate 首个根）

Phase 1 · 域覆盖 → **换根**（CONSOLIDATION-07 停止规则生效）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-system/src/services.rs` — 279（读 150-239：`ModSettingsApplier` /
   `ModSettingsReader` / `ModServices` 全字段）
2. `crates/live2d-ai-mod-system/src/` 全目录规模 — 10 文件 / 1452 行
3. `crates/live2d-ai-desktop/src/mod_registry.rs` — （定点 60-70 / 360-405 / 1311-1340）
4. `crates/live2d-ai-desktop/src/web_api/cli_entry.rs` — （定点 180-230：`apply_settings` 生产接线）

## 跑过的命令（全部只读）
```
git ls-files 'crates/live2d-ai-mod-*/src/*.rs' | xargs wc -l | sort -rn
grep -n "pub fn |pub struct ModServices" mod-system/src/*.rs
sed -n '150,239p' mod-system/src/services.rs
grep -rn "deny_unknown_fields" runtime/src/settings/patch.rs desktop/src/web_api/settings_routes/mod.rs
grep -rn "apply_settings" desktop/src/ --include=*.rs
sed -n '180,224p' desktop/src/web_api/cli_entry.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0060-01（P2）** —— 换根后的第一个发现
Mod→settings 信任边界**失败开放**，且**契约声明与实现相反**：
`services.rs:158-160` 写「写不存在的键由 host 的 `SettingsPatch` 反序列化**拒绝**」，
而生产实现（`cli_entry.rs:193`）只对**类型错**返回 `false`；`SettingsPatch` 系**无
`deny_unknown_fields`** ⇒ 未知键被 serde **静默丢弃** ⇒ Mod 拿到 `true` 却什么也没改成。
详见 FINDINGS 的完整四段（摘录/调用链/影响/反证）。

**这一条同时回答了 CONSOLIDATION-07 留下的待检验问题**：
「本审计尚未出现『某全新区域第一次被审就出 P1/P2』的样本」——**现在有了**（P2，非 P1）。

## 维度覆盖（本批）
- 红线 R（Mod 边界与秘密）：**命中** —— 边界声明失败开放 + applier 把整个 patch 打进日志
- 维度 E：Mod 的 `apply_settings` JSON 契约 vs 宿主 `SettingsPatch` 的字段集
- 维度 G：secret 若出现在 patch 里会进日志（文件层可兜、stdout 不可）
- 维度 J：该边界**零测试覆盖**（唯一相关回归只断言「合法 patch 被接受」）

## 未核实项
1. `ModSettingsReader` 的**生产实现**（`mod_registry.rs:380-401` 的 `reader` 闭包）未逐行核 ——
   契约说它「不含任何密钥明文，也不含 `api_key_env` 变量名」，**这条断言尚未验证**
2. `session.rs`(476) / `topics.rs`(164) / `settings.rs`(155) / `factory.rs`(142) 未读
3. 8 个 Mod crate 的**各自实现**（persona 1004 / director 902 / memory 895 / external-input 847 …）未读
4. `ModSessionPrompts` 的「按会话分桶」是否真的隔离（防 A 会话泄漏到 B）未核
5. `template` crate（新 Mod 起点）未读 —— 它的 `settings_spec` 是否会诱导作者写出 `secret` 字段

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0
