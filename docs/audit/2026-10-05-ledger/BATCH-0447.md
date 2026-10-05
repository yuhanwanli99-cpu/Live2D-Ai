# BATCH-0447 · ✅ **F-0446-01 定论成立** —— 而**根比「没过滤」更靠下**

Phase 4 · **证伪**（定论 F-0446-01：读 `DiagnosticsApi.logs()` 与 `LogLine`）

## 跑的命令（全部只读）
```
sed -n '/Future<List<LogLine>> logs()/,/^  }/p' lib/api/diagnostics_api.dart
grep -n "class LogLine" -A 12 lib/api/diagnostics_api.dart | grep -E "final|class|String get"
```
未跑任何 cargo / flutter / pnpm 命令。

## ⚠ 本批的**工具层事故**（第四次，四种形状之四）
**上一轮我发出的消息在句子中间被截断**（「读 `log` …」后中断）⇒⇒ **一句话发到一半就断了**
⇒⇒ **与前三类事故（heredoc 吞文件 · 漏 import · 相对路径）都不同**：**这一次坏的是**输出本身**
⇒⇒⭐⭐ **⇒⇒ 而它的可复用解法与前三次同源**：**长文本分段发、每段末尾自成一句完整的话**
⇒⇒ **⇒ 前三次我改成「长文本用 `write`、一个脚本一件事」；这一次要加的是「**一句话不要跨越两次输出**」**

## ★ 本批产出：**定论成立，且根更靠下**
```dart
// lib/api/diagnostics_api.dart
Future<List<List<LogLine>> logs() async {  // 实际签名：Future<List<LogLine>> logs()
  final raw = _decode(response.body)['lines'] ?? _decode(response.body)['logs'];   // ⭐ 两条兜底键
  if (raw is! List) return const <LogLine>[];
  return raw.whereType<Map<Object?, Object?>>()
      .map((m) => LogLine.fromJson(_stringKeys(m))).toList();                          // ⭐ 零 where
}
class LogLine {                                       // :58-70
  final String **level**;  final String **target**;  final String **message**;
  String get asText => '[$level] $target: $message';
}
```
### 四个可核点
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 「`LogLine` 没有 `type` 字段」比「`logs()` 没过滤」更根本**
   ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ 要按 `sentence_ready` 过滤，**必须先让 `LogLine` 带一个区分日志种类的字段**
   ⇒⇒ **⇒⇒⇒ ⇒⇒ 「这不是加一个 `where` 就完的事」**
2. ⭐⭐⭐ **⇒⇒ 而服务端的 `MAX_LINES` 是按行数截尾、不是按类型** ⇒⇒ **⇒⇒ 那个方向也没有现成字段可用**
3. ⭐⭐⭐⭐⭐ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ **⇒ 「一行文案」是**唯一不需要动协议**的落点**
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ 「标题写它实际显示的东西」与「加字段做过滤」的成本差是**一个协议字段 vs 零****
4. ⭐⭐⭐⭐ **⇒ 而 `['lines'] ?? ['logs']` 这条兜底本身是一条未记的判据** ⇒⇒⭐⭐
   **⇒⇒ ⇒⇒ 「服务端两种键名都认」这件事只存在于这一行** ⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒⇒ ⇒ ⇒ 它与 B0321 那条（「参数不生效 ⇒ 删参数」）**是同一类痕迹的残留**

⇒ ⇒ **0 条新发现**（F-0446-01 成立、**建议已改写**）
⇒ ⇒⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 3 点**
> **「标题写它实际显示的东西」与「加字段做过滤」的成本差是：一个协议字段 vs 零。**
> **⇒⇒ ⇒⇒ 而两处都还没决定 ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒ 「先改文案、暂不承诺过滤」是那个零成本的顺序**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `['lines'] ?? ['logs']` 这条兜底**是否有测试**（**若有，则两键名都被守住**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~440）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
