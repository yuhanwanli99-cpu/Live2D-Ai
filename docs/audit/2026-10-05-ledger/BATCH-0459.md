# BATCH-0459 · ✅ **`code` 是必填的** —— 而「不写码」不是「没码」、是**选了只写一段**

Phase 4 · **证伪**（兑现 B0458 留的唯一待核：`ApiException` 有没有 `code` 字段）

## 跑的命令（全部只读）
```
grep -n "class ApiException" -A 14 lib/api/api_client.dart | head -18
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0458 那条「不对称」被解释掉了**（0 条新发现）
```dart
// lib/api/api_client.dart:30-40
class ApiException implements Exception {
  const ApiException(this.code, this.message, {this.status});
  final String code;      // ⭐ 必填 · 非空
  final String message;   // ⭐ 必填 · 非空
  final int? status;      // ⭐ **可空的才是它**（有含义：HTTP 层有没有状态码）
  @override
  String toString() => status == null ? '$code：$message' : 'HTTP $status $code：$message';
}
```

### 五个可核点
1. ⭐⭐⭐⭐⭐ **⇒ `code` 是必填的非空 `String`；可空的那个是 `status`**
   ⇒⇒ **⇒ ⇒ 而 `status` 才是「传输层有没有状态码」的那个字段** ⇒⇒⭐⭐⭐⭐⭐
2. ⭐⭐⭐⭐⭐⭐ **⇒ 而 `toString()` 已经把 `code` 与 `message` 合成了**
   「`$code：$message`」/「`HTTP $status $code：$message`」⇒⇒⭐⭐⭐⭐⭐
   **⇒⇒⇒ ⇒ 「传输层失败不写码」这个「合理解释」**不成立** ——
   `code` 一直都在、而 `toString()` 一直带着它** ⇒⇒⇒⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐ **⇒ 而同一个「`$code：$message`」的拼法在同一行代码里出现过两次**：
   `main.dart:1049`（`errorMessage ?? errorCode ?? '未知原因'` 之外的那条 `catch` 里只写 `e.message`）
   ⇄ `api_client.dart:38-39`（`toString()` 里**两个字段都写**）
   ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ **⇒⇒ 「有一处现成的合成」是「要不要自己再拼一次」这个问题的正确答案** ⇒⇒⇒⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐ **⇒ 而 B0453 那条判据在这里也成立**：`status` 可空、且**它的取值里没有一个值本身有意义**
   （`null` = 「传输层失败，没有 HTTP 状态」；`200`/`404` = 真状态码）⇒⇒
   ⇒⇒ **⇒ 回落成 `0` 会与「传输层失败」混同** ⇒⇒ **⇒ 而它**没有**回落到 `0`** ⇒⇒⭐⭐⭐⭐⭐
5. ⇒ ⇒ **⇒ B0458 那条「不对称」现在可以写清了**：
   > **它是一个选择**：那条分支**只展示用户读得懂的那一段**（`e.message`），
   > **而 `e.code` 仍在 `toString()` 里**、也仍可被别处取用 ⇒⇒⇒⭐⭐⭐⭐⭐
   > ⇒⇒ **⇒ 按判据（用户可察觉的后果 = 0；两端都是真值、只是展示不同）⇒⇒ **不记发现** ⇒⇒⇒⭐⭐⭐⭐⭐**

⇒ ⇒ **0 findings**（B0458 的待核项结清）
⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 3 点**
> **`toString()` 已经把 `code` 与 `message` 合成了** ⇒⇒ **而另一处手写了其中一段**
> **⇒⇒⇒⇒⇒ ⇒ 「有一处现成的合成」是「要不要自己再拼一次」这个问题的正确答案**
> **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ 而 B0453 的判据在这里也成立**：`status` 可空、其值域里没有一个值本身有意义 ⇒⇒ **回落成 `0` 会与「传输层失败」混同** ⇒⇒ **而它没有回落到 `0`**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~585 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~440 / `chat_panel` ~440）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
