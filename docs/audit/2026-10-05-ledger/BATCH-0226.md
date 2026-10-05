# BATCH-0226 · ⭐ 转前端：`chat_panel.dart` 的头注**在 5 行内**告诉我「它不该有帧处理」

Phase 1 · 域覆盖 · **转前端 `.dart`**（183 未读，最大面）—— `lib/ui/chat_panel.dart`(569)

## 跑的命令（全部只读）
```
wc -l lib/ui/chat_panel.dart
grep -nE "text_fallback|reasoning|turn_state|case " lib/ui/chat_panel.dart      # ⇒ 零命中
grep -oE "ChatRole\.[a-z]+|k[A-Z][A-Za-z]+Notice|hasReasoning" lib/ui/chat_panel.dart  # ⇒ 只命中 bubble.dart
sed -n '1,22p' lib/ui/chat_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **「两次零命中」是对的，而头注在 5 行内说明了原因**
```dart
/// 聊天面板（L3）：消息列表 + 音频条 + 输入区 + 设置入口。
///
/// **只依赖纯模型**（`chat/chat_message.dart`、`state/ui_phase.dart`），
/// **不** import `package:web` 那条链 —— 于是**整个面板可 VM/widget 测试**。   // :1-5
```
⇒ ⇒ **它刻意只依赖纯模型** ⇒ ⇒ **它读不到原始帧** ⇒ ⇒ **我那两次「零命中」是正确结果、不是漏搜**
⇒ ⇒ **帧的处理在 controller 层**（`ws_client` → `ChatController` 决策 → 面板渲染），
而**决策层我已在 B0189–B0192 审过**（`_finishTurn`/`settleTurn` · 错误分支 · 通道闭包）
⇒ ⇒ **这个文件的职责就是渲染**，不是解帧

### ⭐ 而这已是**本仓第 4 处「把可测性设计进去并写下来」**
| # | 位置 | 做法 | 核验 |
|---|---|---|---|
| 1 | `main.dart` | 经 `app/browser_io.dart` **隔离 `package:web`** ⇒ VM 加载不了 ⇒ 因此那条测试**只能扫源码**（B0180） | B0180 |
| 2 | `LiveRegionThrottle` | **时钟可注入** `now: () => now` ⇒ 测试**控制时间而非 sleep** | B0179 |
| 3 | `interval_secs_from_config` | **在转换前**挡掉 `NaN`/无穷 ⇒ 下游拿不到非法值 | B0213 |
| 4 | **本批** `chat_panel.dart` | **不 import `package:web` 那条链** ⇒ **整个面板可 VM/widget 测试** | B0226 |

⇒ ⇒ 而本批的收获是**读法**而非结论：
> 我在新根上的第一反应是「面板应该处理那些帧」⇒ **错的**；而**头注 5 行就说了它的职责**
> ⇒ ⇒ **B0213 立的读法（先头注）在换根的第一批就立刻回本。**

## 未核实项
1. `chat_panel.dart` 余 ~540 行未读（消息列表渲染、音频条、输入区、设置入口）
2. `error_banner.dart`（B0136 核过它「按码分流」）未读本体
3. `message_bubble.dart`(577) · `memory_panel.dart`(694) · `persona_panel.dart`(608) · `director_observer_section.dart`(668) · `live2d_stage.dart`(666) 未读
4. 前端 `.dart` 仍 183 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
