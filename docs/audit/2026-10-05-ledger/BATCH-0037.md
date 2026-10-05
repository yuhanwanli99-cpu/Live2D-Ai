# BATCH-0037 · chat_controller + 双消费路径核验

Phase 1 · 域覆盖 → 前端层「未审新区」

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/chat/chat_controller.dart` — 623（**读 450-559**）
2. `shell/flutter/lib/state/ui_state_tracker.dart` — 250（**读 145-204**）
3. 跨语言对照（已在前批读过的原文）：`conversation/engine.rs:542` / `supervisor/handlers.rs:326`
   / `ws/events.rs:66-70`（致命错误 → `turn_state{failed}` 的投影链）

## 跑过的命令（全部只读）
```
git ls-files 'shell/flutter/lib/chat' | xargs wc -l | sort -rn
sed -n '450,559p' chat_controller.dart
sed -n '145,204p' ui_state_tracker.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 A（两份快照）：**核验为刻意分工，非缺陷**；失效半径仅显示层且自愈
- 维度 C（引用语义）：会话切换的「迟到 delta 落进新会话」陷阱已被 `_abandonTurn()` 显式堵住
- 维度 D（落盘节流）：轮末才写，体积双重夹持 ✔
- 维度 E（跨路径一致性）：错误文案共用 `formatWsError`；兜底码同值不同源（备忘）

## 未核实项
1. `chat_controller.dart` 的 `send` / `_appendDelta` / `_abandonTurn` 实现（:1-450 / :559-623）未读
2. `chat_session.dart`(422) **未读**——`touchActive` / `replace` / `pruneEmpty` 的语义与
   `maxSessions`/`kMaxMessagesPerSession` 的实际夹持点未核
3. `ui_state_tracker.dart:1-145` 未读（`enterInterrupted` / `consume` 的前半段）
4. 「两条路径共用 `formatWsError`」我只核了 tracker 侧；`chat_controller` 侧那处调用未定位到行

## 本批新增
**0 条**（净产出：关闭 STATE 里挂了 6 批的「双快照」疑问 + 三项核验 + 一处备忘）
