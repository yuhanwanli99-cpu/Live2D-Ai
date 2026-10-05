# BATCH-0038 · 会话体积夹持 + epoch_gate（停止即静音红线）

Phase 1 · 域覆盖 → 前端层「未审新区」

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/chat/chat_session.dart` — 422（定点 340-399）
2. `shell/flutter/lib/audio/epoch_gate.dart` — （**全读**，52 行）
3. 跨语言对照：`desktop/src/supervisor/turn.rs:149 / 674-677`、`supervisor/tests_stall.rs:232-239`、
   `web_api/ws/events.rs:169-175`（后者 B0017 已核）

## 跑过的命令（全部只读）
```
grep -n "kMaxMessagesPerSession|maxSessions|prune|while (.*len()" chat_session.dart
sed -n '340,399p' chat_session.dart
ls shell/flutter/lib/audio/ ; cat shell/flutter/lib/audio/epoch_gate.dart
grep -rn "NewEpoch" crates/ --include=*.rs | grep -v test
grep -rn "new_epoch|NewEpoch" supervisor*.rs supervisor/
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 B（生命周期）：会话裁剪不删「用户正在看的」——明确避开 `firstWhere(orElse:)` 反模式
- 维度 C（取消）：**「停止即静音」红线端到端验证**，跨 Dart/Rust 两侧各有一个钉子
- 维度 A（真源）：Dart 存储与 LLM 历史**完全解耦**（`ChatBody` 不带历史）
- 维度 J（可测性）：`relativeTime` 显式注入 `now` 的纪律在三个文件一致执行

## 未核实项
1. `chat_controller.dart` 的 `send` / `_appendDelta` / `_abandonTurn` 实现未读
2. `audio/{audio_player,gain,sentence_assembler,stage_clock,wav}.dart` 未读
   （`sentence_assembler.dart` 与 Rust 侧 `dialogue/sentence.rs` 是同名不同实现，**值得对一次**）
3. `chat_session.dart:1-340`（消息追加 / `touchActive` / `replace` / `pruneEmpty`）未读
4. `epoch_gate` 的 Dart 侧**测试**是否存在未核实（其头注声称「抽成纯函数后可单测」，
   但我没读那条测试）

## 本批新增
**0 条**（净产出：一条红线跨三语言 + 两侧测试的端到端验证；顺带闭合 F-0036-01 的疑问）
