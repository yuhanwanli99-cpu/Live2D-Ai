# BATCH-0446 · ✅ **+1 条 P2：标题在替一个不存在的过滤背书**

Phase 4 · **证伪**（B0445 留的：`logs()` 内部有没有过滤）

## 跑的命令（全部只读）
```
sed -n '159,178p' lib/settings/sections/director_observer_section.dart
grep -n "loadLogLines" lib/settings/sections/director_observer_section.dart
sed -n '210,226p' lib/settings/sections/director_observer_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0446-01 (P2)**
```dart
// :21   /// | D 送 TTS 文本 | 左 = 前端已收 text_delta；**右 = 日志端点 sentence_ready** |
// :634  Text('**原始段（日志端点 sentence_ready）**', style: muted)
// :210  Future<List<String>> _defaultLoadLogLines() async {
// :212    return **lines.map((LogLine l) => l.asText).toList()**;      // ⭐ 全部、零过滤
```
### 四个可核点
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 「只含 `sentence_ready`」只存在于注释（`:21`）与那行小标题（`:634`）；
   代码是全量映射、零过滤** ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒ 标题在替一个不存在的过滤背书**
   ⇒⇒ 用户与后来的读代码者都会以为右列只有 `sentence_ready`，而它显示**全部日志行**
   ⇒⇒ **而这一列是 dev_mode 下唯一的「D 送 TTS 文本」对照面**（B0433 已核这块整块只在 devMode 下渲染）
2. ⭐⭐⭐⭐ **⇒ 而过滤只能做在客户端** ⇒⇒ **B0321 已核 `?limit=` 服务端不读**（`handle_logs()` 只按内置
   `MAX_LINES` 截尾，参数被删掉而不是留着）⇒⇒⇒⭐⭐ **⇒⇒ 「按 `sentence_ready` 过滤」这条路是存在的、只是没写**
   ⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ 缺口是**实现**缺口、不是能力缺口**
3. ⭐ **⇒⇒⇒ 而 `loadLogLines` 是可注入的**（`:160`，B0432 核过 `DirectorObserverFeed` 那个注入思路）⇒⇒
   ⇒⇒ **⇒⇒ ⇒⇒ ⇒ 而它的缺省实现就是全量 ⇒⇒ ⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ 测试可以注入假数据口、所以「这一列显示什么」是**可单测**的**
   ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ 「它现在是一个**只有注释主张**的事实」**（写进了建议）
4. ⚠ **⇒⇒⇒ 软栗（如实记）**：**我未读 `DiagnosticsApi.logs()` 与 `LogLine` 的定义** ⇒⇒
   **若 `logs()` 本身只含 `sentence_ready`（过滤在服务端或 API 内部）⇒⇒ 则本条不成立**
   ⇒⇒⇒ **⇒⇒⇒ 而这是**可一读定论**的** ⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒ ⇒ 下一批第一件事**
   ⇒⇒⇒⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒ ⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒ **按 B0271：强度不足时先记待核、而反证条款已写进 FINDINGS**

⇒ ⇒ **修法在同一处二选一**：**按标题兑现**（客户端 `.where`）**或按代码改标题**（「服务端日志（全部行）」）
⇒ ⇒ **且建议把「这条列是不是全量」写进 `DiagnosticsApi.logs()` 的文档** —— **那个方法刚被 B0321 改过、而它的文档没记这件事**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. ⭐⭐ **`DiagnosticsApi.logs()` 与 `LogLine` 的定义**（**F-0446-01 唯一的软栗**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~455）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
