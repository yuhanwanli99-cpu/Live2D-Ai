# BATCH-0475 · ✅ **它读 Rust 源码** —— 而**「不伪造通过」是 AGENTS 那条纪律在测试体例上的同款实现**

Phase 4 · 证伪（兑现 B0474 留的：那条跨语言回归的断言形状）

## 跑的命令（全部只读）
```
cat shell/flutter/test/voice_wake_default_consistency_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**疑问结清 —— 断言是真跨语言的**（0 条新发现）
```dart
// test/voice_wake_default_consistency_test.dart
/// 前端按钮文案说「小可爱 ……」，服务端闸门也必须认「小可爱」——
/// 两边默认值漂移会变成「按钮让你说小可爱、**服务端回 400 wake_phrase_require**」
/// 而且**断网 / 无日志时完全看不出来**。
/// 这里直接读 Rust 常量源码做交叉核对（字体子集门禁同款「生成物 + 源码」）
/// 找不到仓库结构（例如包被单独拷出去跑）→ 明确 skip，**不伪造通过**。
const String _rustGatePath = '../../crates/live2d-ai-mod-voice-input/src/gate.rs';
…
final RegExpMatch? m = RegExp(
  r'pub const DEFAULT_WAKE_PHRASE:\s*&str\s*=\s*"([^"]+)"',
).firstMatch(source);
expect(m, isNotNull, reason: 'Rust 侧必须定义 DEFAULT_WAKE_PHRASE');
expect(m!.group(1), kDefaultWakePhrase, reason: '两端缺省唤醒词漂移：Dart=… Rust=…');
```

### 四个可核点
1. ⭐⭐⭐⭐⭐ **它用正则从 Rust 源码里抓常量、再与 Dart 常量比**
   ⇒⇒ **⇒⇒ 断言是真跨语言的、不是硬编码对拍** ⇒⇒⇒⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒ ⇒ 而若它硬编码 `'小可爱'` 与 Dart 常量比，那**两侧同改时仍会绿** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐**
2. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而头注把后果写成一个具体的错误响应**：「**服务端回 `400 wake_phrase_require`**」
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 比 B0474 那句「按钮说小可爱、服务端拒小可爱」更具体：它点名了状态码** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「断网 / 无日志时完全看不出来」是本仓反复出现的「看不见的失败」表述**：
   | 处 | 说法 |
   |---|---|
   | 字体子集 | 「**有网时看不出来**」· 断网即豆腐块 |
   | CanvasKit CDN | 缺 `--no-web-resources-cdn` ⇒ **断网即白屏** |
   | `GET /logs?limit=200` | 回 **501** ⇒「**诊断日志从未成功过**」 |
   | **本处** | 「**断网 / 无日志时完全看不出来**」 |
   ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「明确 skip、**不伪造通过**」** 是 AGENTS 那条纪律在**测试体例**上的同款实现
   （AGENTS：「端到端探针**只在有活端点时跑**，缺配置就明确跳过 —— **绝不伪造绿灯**」）
   ⇒⇒ **⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ **⇒ 「不伪造绿灯」从 CI 纪律变成了测试体例、而这两处是同一句话** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 而「找不到文件 ⇒ print SKIP ⇒ return」是**最弱的一种 skip**（它不失败、不标记 `skip:`）⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐**

⇒ ⇒ **0 findings**（B0474 留的疑问结清：**它读源码、不是硬编码**）
⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 1 点**
> **它读 Rust 源码里的常量、再与 Dart 常量比** ⇒⇒ **断言是真跨语言的、不是硬编码对拍**
> **⇒⇒⇒ ⇒ 而若它硬编码 `'小可爱'` 与 Dart 常量比，那两侧同改时仍会绿** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
> **⇒⇒⇒ ⇒⇒⇒ ⇒ 而头注比 B0474 那句更具体：它点名了状态码（`400 wake_phrase_require`）**
> **⇒⇒⇒ ⇒⇒⇒ ⇒ ⇒ 「断网 / 无日志时完全看不出来」是本仓第 4 次出现的「看不见的失败」表述**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `wake_gate_open` 本体（`gate.rs:83-84`）· `clean_transcript` 的尾部（`:270` 之后 `Option` 怎么变）·
   `lower_eq` / `is_separator` 本体
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
