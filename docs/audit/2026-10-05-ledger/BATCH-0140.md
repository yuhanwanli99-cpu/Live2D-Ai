# BATCH-0140 · ⭐ 解掉 B0139 的跨层优先级疑问：**两个标志从不同时为真 ⇒ 不分歧**

Phase 1 · 域覆盖 · `chat/chat_controller.dart`（收口调用面）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/chat/chat_controller.dart` — 623（定点 380-470：`_onAudio` 与
   **`_finishTurn({bool failed = false, bool stopped = false})`**（:429）+ 全部 4 个调用点）

## 跑的命令（全部只读）
```
awk 'NR>=380 && NR<=470' chat_controller.dart | grep -nE "void |bool |failed|stopped|_finishTurn|required"
grep -n "_finishTurn(" -A 2 chat_controller.dart | grep -E "failed|stopped|_finishTurn"
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**B0139 的未核实疑问关闭 —— 且结论是「结构上不可能分歧」**
### ① `_finishTurn` 的两个标志是**带默认值的参数**，而**每个调用点只设其中一个**
```dart
void _finishTurn({bool failed = false, bool stopped = false}) { … }   // :429
…
_finishTurn(stopped: true);                          // :215 用户按停止（failed 默认 false）
_finishTurn(failed: completed == false);             // :245 text_delta{completed}
_finishTurn(failed: status == 'failed');             // :260 turn_state（**failed 直接取自后端 status**）
_finishTurn(failed: true);                           // :327 本地错误路径（error 帧 / 兜底码）
```
⇒ **`failed` 与 `stopped` 从不同时为真。**

### ② ⇒ 「优先级相反」是**不可观察的**
- Rust（`engine.rs:540-546`）**先判 `cancelled`**；Dart（`settleTurn` :80-81）**先判 `failed`**；
- 但 `:260` 的 `failed` **直接取自后端的 `status`** ⇒ 后端报 `cancelled` 时
  `failed == ("cancelled" == "failed") == false` ⇒ `settleTurn` 的 `failed` 分支**根本不会进**；
- 而 `:215`（停止）**不带** `failed` ⇒ 也不会进；
⇒ **两个分支各自只由一种信号触发**，「同时失败又同时停止」这个 tie **在结构上不存在**
⇒ **`settleTurn` 里 `failed` 先于 `stopped` 的顺序，从未被真正行使过**（不是「碰巧对」）。

### ⭐ 归入正面模式：**把 tie 设计成不可能，而不是靠测试保证**
本仓处理这类「同一状态两层各判一次」的地方，**有的会分歧**（F-0034-01 的 `==` 绕过 `BackgroundImage.==`、
F-0062-01 的三段职责无人认领），**有的则让 tie 无法出现**（此处）。
⇒ 分界线很清楚：**先看两侧是否可能被同一批输入同时触发**；不可能 ⇒ 是设计赢，可能 ⇒ 就是接缝风险。

## 未核实项
1. `_finishTurn` **是否可能被同一轮调用两次**（若有，B0036/B0037 核过的 `_streaming` 闸是第一道，
   但本批未核 `_finishTurn` 自身是否有幂等守卫）⇒ **已记 STATE**
2. `:327` 的本地 `failed: true` 与 `:260` 的后端 `failed` 在**同一轮**能否叠加（若能，
   则 tie 仍可能出现）⇒ **未核实**
3. `engine.rs` 阶段 2/3 余段（TTS 排空等待 / `TurnReport` 汇总 / 表演层 snapshot 注入点）未读
4. `supervisor/turn.rs` 余 530 行未读；`conversation/mod.rs` 余 ~490 行未读
5. `worker.rs` 余 200 行 / `llm.rs` 余 360 行 / `client.rs` 余段 / `plan.rs` 余 700 行未读
6. `secrets.rs` 10 条测试断言体未读；B0120「是否别处 chmod 过 toml」未核
