# BATCH-0371 · ✅ **路径不成立**（我提的问题被自己答掉）—— 而答案落在**两个已核结论的交集**上

Phase 4 · **证伪** —— 结清 B0370 留下的那条「去重会不会吞帧」

## 跑的命令（全部只读）
```
grep -nE "_fail\(|_lastSentStageBg|_modelLoaded" shell/flutter/lib/live2d/live2d_bridge.dart
sed -n '519,530p' live2d_bridge.dart ; sed -n '375,392p' live2d_bridge.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **我提的问题不成立，而理由是一条可提炼的**
| 路径 | 状态 |
|---|---|
| `ready` 时 `sendStageBg(X)` ⇒ `_lastSentStageBg = X`（B0370 核过**乐观记账**） | ✅ |
| 下行失败 ⇒ `_fail`（`:519-526`）**只重置**：`_errorMessage` · `_phase = error` · **两个 Timer** · `notifyListeners()` | ✅ |
| `destroy`（`:375-392`）**只重置**：**两个 Timer** · `_phase = destroyed` · `notifyListeners()` ⇒ 然后 `dispose()` | ✅ |
| ⇒ **两者都不重置 `_lastSentStageBg`** | ✅（这是我担心的地方） |
| **但用户点重试 ⇒ `retry()` 拆掉旧桥、`new Live2DBridge(transport)`**（B0323 已核） | ⭐ |
| ⇒ ⇒ **新桥的 `_lastSentStageBg` 是 `null`** ⇒ ⇒⇒ **所以去重不会吞** | ✅ |

⇒ ⇒⇒⭐ **我担心的失效路径不成立**，而它不成立**不是因为有人记得清这个字段**，
**而是因为重试换了一个新对象** —— **桥侧的状态不跨桥存活，所以也不会跨桥变陈旧**。

### ⭐ 而这提炼出一条比 B0323 的第一个理由更普适的
B0323 核过 `retry()` 的理由是「**宿主不需要知道舞台内部是怎么重挂的**」（窄接口）。
⇒ ⇒ **这里浮出它的第二个理由**：
> **「重建整个对象」是一种状态重置手段**，它比「在旧对象上逐个清字段」**更难漏** ——
> 因为**漏一个字段就会带一个陈旧值过桥，而换对象则结构上不可能。**
⇒ ⇒ 同族：`_attach` 每次重放全部（B0290）· **重试换对象而非清字段**（本批）

### ⚠ 而我据此**修正 B0357 的「清空清单」**
| 我 B0357 的说法 | 实际 |
|---|---|
| 「重试路径上三样东西各自被清：旧桥 / 旧相位 / 旧错误」 | ✅ **对，但那是**舞台侧**（`live2d_stage`）**的清单** |
| （我隐含以为）**桥侧**也是逐个清 | ❌ **桥侧是**换对象**（`Live2DStageState.retry()` → 新 `Live2DBridge`）** |
⇒ ⇒⇒ ⭐ **两处机制不同，但都到位** ⇒ ⇒ **我上一批把它们当成同一份清单来比**
⇒ ⇒⇒ **这是「凭片段」形态的**第二天复发**（B0369 立、今天又犯）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1790 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
