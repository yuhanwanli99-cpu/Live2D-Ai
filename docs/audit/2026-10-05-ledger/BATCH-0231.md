# BATCH-0231 · `persona_panel.dart`：**2 入口 × 2 作用域 = 4 个组合，逐个有文档行**；跨 Mod 关系「与 memory 文档同口径」

Phase 1 · 域覆盖 · 前端 `.dart`（41/218）—— `lib/settings/mods/persona_panel.dart`(608)

## 跑的命令（全部只读）
```
grep -nE "last-writer|记忆|坏卡|import_card|规范化" lib/settings/mods/persona_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**第四处「指路而非复述」**
| 入口 | 作用域 | 命令 | 行 |
|---|---|---|---|
| 粘贴框 | **绑定当前会话**（主路径） | `import_card(card_json)` → `_runForSession` | :301 / :311 |
| 粘贴框 | **不给 `session_id`** = **旧的全局行为** | `import_card(card_json)` → `_run` | :314 / :324 |
| 选 PNG 卡 | 同样绑定当前会话（**前缀由 Mod 剥**） | `import_card(data_base64)` | :331 / :373 |

⇒ ⇒ ⭐ **会话绑定在 UI 上是一个可见的选择，不是隐式行为**（两条路径各有自己的文档行）
⇒ ⇒ ⭐ **`prefix` 的归属写清了**：「**前缀由 Mod 剥**」⇒ **责任在 Mod 侧、UI 不做**
   ⇒ 与我在 B0113 核的 persona 侧（`render_injected_text` / `render_from_config` 是 **Mod 侧纯函数**）**一致**
⇒ ⇒ ⭐ **2 入口 × 2 作用域 = 4 个组合，逐个有文档行** ⇒ **组合面被逐案记录**（这是让 4 路矩阵保持诚实的办法）

### 跨 Mod 关系：**用用户能懂的话 + 给出可行动建议**
```dart
/// 与「记忆」Mod 的关系：**last-writer-wins，没有仲裁**（**与 memory 文档同口径**）。   // :585
'后写覆盖、不做仲裁——**想稳定用全局人设就别同时开「记忆」的注入**'                        // :595
```
⇒ ⇒ ⭐ 不只说规则（后写覆盖），**还说用户该怎么避免**（别同时开注入）⇒ ⇒ **P1 的 UI 版**
⇒ ⇒ ⭐ 「**与 memory 文档同口径**」⇒ **第四处「指路而非复述」**（B0217 模板 / B0220 归属 / B0228 改动记录 / 本批）

## 未核实项
1. `persona_panel.dart` 余约 560 行未读（会话绑定卡片的呈现、停用还原的 UI、坏卡失败的处置文案本体）
2. `message_bubble.dart` 余 ~500 行 · `memory_panel.dart` 余 ~600 行 · `chat_panel.dart` 余 540 行 · `error_banner.dart` 余 45 行未读
3. `director_observer_section.dart`(668) · `live2d_stage.dart`(666) 未读
4. 前端 `.dart` 仍 177 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
