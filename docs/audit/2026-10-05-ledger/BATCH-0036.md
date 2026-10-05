# BATCH-0036 · 主链路 Dart 侧：turn_liveness 跨语言契约核验

Phase 1 · 域覆盖 → 前端层「未审新区」（按 CONSOLIDATION-04 校准后切入）

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/chat/turn_liveness.dart` — 200（**全读**）
2. `crates/live2d-ai-desktop/src/supervisor/handlers.rs` — （定点 320-340，跨语言对照）

## 跑过的命令（全部只读）
```
git ls-files 'shell/flutter/lib/chat' | xargs wc -l | sort -rn
grep -rn "TurnAborted" crates/ --include=*.rs
grep -rn "TurnAborted" -A 6 supervisor/handlers.rs app_event.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 C（取消/收尾）：Dart 侧全部收口判据 × Rust 侧投影面 **逐条对齐**，见 FINDINGS 表格
- 维度 E（契约）：**跨语言断言逐字核实为真**，并发现它承载着一个未被记录的契约 → F-0036-01

## 未核实项
1. `chat_controller.dart`(623) / `chat_session.dart`(422) / `chat_markdown.dart`(199) /
   `chat_message.dart`(128) **未读**——落盘节流、双消费路径（`UiStateTracker` vs
   `ChatController`）的一致性都还没看
2. **是否有 Dart 测试钉住「停止 ⇒ 收口」**（决定 F-0036-01 能否降为备忘）——未核实
3. `audio/epoch_gate.dart`（Dart 侧，被 `kTurnPreemptedCode` 注释引用）未读

## 本批新增
P0 0 · P1 0 · P2 0 · **P3 1**
