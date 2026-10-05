# BATCH-0009 · 响应投影 DTO（两份）

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`

## 读过的文件（来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/dto.rs` — 443（**读 1-240**；:241-443 是
   `llm_status_from`/`tts_status_from` 函数体、`capabilities_info` 与测试——其中
   `llm_status_from` 的函数体已在 BATCH-0002 读过（dto.rs:240-258））
2. `crates/live2d-ai-desktop/src/web_api/models_routes/dto.rs` — 275（**读 1-150**；
   :151-275 是 `apply` 与 `validate` 实现及测试）

合计 2 文件 / 718 行（读了 390 行）。`settings_routes/*` 顺延 BATCH-0010。

## 为验证跨端契约而读的清单外文件（grep + 片段）
- `shell/flutter/lib/settings/sections/dev_tools_section.dart:1108-1109, 1186`（`status['audio']` 的真实消费者）
- `shell/flutter/lib/api/ws_frame.dart`（BATCH-0003 已读，本批复用结论）

## 跑过的命令（全部只读）
```
grep -rn "\.audio\b|audio\.available|available" shell/flutter/lib/api/ws_status.dart
grep -rn "status.audio|\.audio\.available" shell/flutter/lib/
grep -rln "app/status|AppStatus|appStatus" shell/flutter/lib/
grep -rn "AudioStatus|'audio'|\"audio\"" shell/flutter/lib/
```

## 维度覆盖（本批）
- 维度 E（契约一致性）：✔ 错误体三段式 `code`( &'static str )/`message`/`details` 统一
  （dto.rs:146-189），`code` 用 `&'static str` 从类型上锁死「稳定契约字符串」；
  `ApplyStatus` 的四个 snake_case 字面量（dto.rs:204-220）就是前端硬编码锚点
- 维度 E（跨端实证）：✘ `app.status.audio` 是**写死**的，且 DTO 头注描述的是条件语义 → F-0009-01
- 维度 E（反证 F-0002-04）：`dto.rs:102` 的字段注释写的是
  「最近一次回退原因码（`performance_ok` = 没回退过）」，而 `app_routes.rs:305-308`
  把同一个值用在第三种含义上（「根本没有 supervisor，从没跑过」）⇒ **F-0002-04 得到反证支持**，
  不是我的推断，是 DTO 自己的契约被生产者违反。已在 F-0002-04 补记。
- 维度 A：`deny_unknown_fields` 用在**请求体**上是正确的（输入校验），
  与 F-0007-01（用在**持久化**结构上导致向前不兼容）形成对照 ⇒ 佐证 F-0007-01 的建议方向
- 维度 E：✘ 同文件内 `ImportRequest` 是唯一没有 `deny_unknown_fields` 的请求体 → F-0009-02

## 未核实项
1. `dto.rs:241-443` / `models_routes/dto.rs:151-275` 未读。
2. `SettingsView`（`live2d_ai_runtime::settings::view`）与 Dart `settings_models.dart` 的
   逐字段对齐**不在本 crate**，属 runtime + flutter 两个批次——维度 E 的主战场尚未开打。
3. `AppInfo.capabilities` 的 5 个字段删除（dto.rs:20-25）是否与前端一致，未核 Dart 侧。

## 本批新增
P0 0 · P1 0 · P2 1 · P3 1 ｜ 另：为 F-0002-04 补到一条 DTO 侧的反证
