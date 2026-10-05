# BATCH-0381 · ⭐⭐⭐ **三层自证在一个门禁里** —— 而扫描器自身被排除**且理由写明**

Phase 4 · **证伪** —— 同样的问法第三次，用在 B0000 ⑥（UI 文案露出 Markdown `**`）

## 跑的命令（全部只读）
```
grep -rln "EmphasizedText|markdown|\*\*" shell/flutter/test/ | head -4
grep -rn "EmphasizedText" shell/flutter/test/ | head -4
grep -rn "test(" shell/flutter/test/visual_language_test.dart | head -6
sed -n '336,348p' test/visual_language_test.dart ; sed -n '362,372p' test/visual_language_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0000 ⑥ 的扫描规则在，且它是本仓最强的门禁**（0 条新发现）
`test/visual_language_test.dart`：
1. ⭐⭐⭐ `test('**扫描器能抓到违规（合成样例自证）**')`（`:341`）⇒ 内嵌一个**合成坏样例**
   （`'背景图**盖在纯色底上**：有图时看得到图。'`）⇒ ⇒ **先证明它会红**
2. ⭐⭐ **反向对照**（`:362`）`expect(findBareBold(commented), isEmpty);`
   ⇒ ⇒ **被注释掉的代码里带 `**` 不该被扫出来** ⇒⇒ **它不是简单 grep**
3. ⭐⭐ `test('**lib/ 里没有违规**')`（`:366`）⇒ 递归走 `lib/**/*.dart`，
   且带注释「**渲染器自己当然有 `**`**」⇒⇒ **扫描器自身被排除、且排除理由写明**

⇒ ⇒⭐⭐⭐ **三层自证**
| 层 | 证明什么 |
|---|---|
| ① 合成坏样例 | **扫描器不是空转的**（不会「结构上无法失败」） |
| ② 反向对照 | **它不是在 grep**（有上下文判断） |
| ③ 排除规则 + 理由 | **它知道自己会撞到自己** |

⇒ ⇒⭐ **这正是我 P1 判据里那条「test that structurally cannot fail」被认真对待的样子**
⇒ ⇒ **B0344 那道字体门禁**以另一种形式有同一性质（哈希自校 · 「防的是过期合规」）
⇒ ⇒⇒ ⭐ **而据此可以说一句关于这个仓库的实话**：
> **这个仓有少数几个门禁到了「先证明自己会红」这一层，多数没到。**
⇒ ⇒⇒ **这不构成缺陷**（**没到那层的门禁仍然在拦东西**）⇒ ⇒ **只作为一条观察记下**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
