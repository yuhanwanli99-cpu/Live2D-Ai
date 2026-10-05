# BATCH-0247 · ⭐ 「未收尾」标记的**理由是一个条件**，且**只在为真时落盘**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_message.dart` / `chat_controller.dart` 的 `unfinished`

## 跑的命令（全部只读）
```
grep -rnE "unfinished|未收尾" lib/
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **闭合 rc.3 N0 链的第 4 跳，并核到「条件式落盘」**
| 层 | 位置 | 内容 |
|---|---|---|
| 字段的理由 | `chat_message.dart:84-85` | 「**刷新后那段文字还在，就必须还带着「未收尾」这个限定**」 |
| 序列化 | `:101` | `if (unfinished) 'unfinished': true,` |
| 反序列化 | `:125` | `unfinished: raw['unfinished'] == true,` |
| 施加 | `chat_controller.dart:369/387` | 「**整段设置**（不是追加）并打上 `unfinished`」→ `bubble.unfinished = true;` |

三个可核点：
1. ⭐⭐ **字段的理由是一个**条件**，不是「我们要持久化」：
   「**刷新后那段文字还在，就必须还带着『未收尾』这个限定**」
   ⇒ ⇒ 换句话说：**落盘是让那句话继续为真的唯一办法**
2. ⭐ **只在为真时写键**（`:101`）⇒ ⇒ **正常消息的存档里没有这个键** ⇒ **不增加常态存档体积**
   ⇒ 且 `fromJson` 用 `== true` ⇒ ⇒ **缺键 / 非 true 都归 false** ⇒ **向前兼容**（老存档能读）
   ⇒ ⇒ 这是 **P25 思路的一个小实例**：**字段按需写、读取端给默认** ⇒ **新字段不污染旧数据**
3. ⇒ **rc.3 N0 链四跳全通**（B0133 记的那条）：
   WS `text_fallback` 帧 → `ConversationUiEvent::TextFallback`（B0135）→ **`_applyTextFallback`
   整段设置 + 标记**（本批）→ **落盘**（`:101`）→ 刷新后**仍带限定**（`:125`）
   ⇒ 而「刷新后还在」这个条件**被写成了字段的理由** ⇒ **不是「顺手做了持久化」，是「不做就不成立」**

## 未核实项
1. `dev_tools_section.dart` 余 ~1850 · `live2d_stage.dart` 余 ~620 · `memory_panel.dart` 余 ~560 ·
   `persona_panel.dart` 余 ~520 · `message_bubble.dart` 余 ~480 · `chat_panel.dart` 余 ~480 ·
   `error_banner.dart` 余 ~15 未读
2. `chat_controller.dart` 的 `_applyTextFallback` **本体**未读（本批只读到 `:369/:387` 两行）
3. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
4. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
5. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
6. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
