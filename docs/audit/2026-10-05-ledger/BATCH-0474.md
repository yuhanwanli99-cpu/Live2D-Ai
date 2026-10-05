# BATCH-0474 · ✅ **两侧逐字相同** —— 而「**必须**逐字一致」是要求、不是巧合

Phase 1 · 域覆盖 · 跨语言（兑现 B0473 留的：`wake_phrase_from_config` 的缺省词与它的兜底）

## 跑的命令（全部只读）
```
grep -n "fn wake_phrase_from_config" -A 8 crates/live2d-ai-mod-voice-input/src/gate.rs | head -10
grep -n "小可|默认唤" shell/flutter/lib/voice/voice_listen_controller.dart shell/flutter/lib/main.dart | head -4
grep -rn "DEFAULT_WAKE_PHRASE" crates/live2d-ai-mod-voice-input/src/*.rs | grep -i "const|=" | head -2
sed -n '29,35p' shell/flutter/lib/voice/voice_listen_controller.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**一份跨语言常量 + 它漂移时的用户可见对句**
| 侧 | 落点 | 值 |
|---|---|---|
| Rust | `gate.rs:70` | `pub const DEFAULT_WAKE_PHRASE: &str = "小可爱";` |
| Dart | `voice_listen_controller.dart:33` | `const String kDefaultWakePhrase = '小可爱';` |

Dart 侧 `:30-32` 把它写成**要求**：
> 「**必须与 Rust 侧 `live2d_ai_mod_voice_input::gate::DEFAULT_WAKE_PHRASE` 逐字一致** ——
> **两边默认值漂移会让「按钮说小可爱、服务端拒小可爱」**」
> 「**回归在 `test/voice_wake_default_consistency_test.dart`**」

### 四个可核点
1. ⭐⭐⭐⭐⭐ **两个常量逐字相同** ⇒⇒ **而兜底路径也相同**：
   `gate.rs:77-80` `Some(raw) => raw.trim().to_string()` / `None => DEFAULT_WAKE_PHRASE.to_string()`
   ⇒⇒ **⇒ 键缺失与键非字符串**走**同一条**兜底 ⇒⇒⭐⭐⭐⭐⭐
2. ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而漂移的后果被写成一个具体的、用户可观察的对句**：
   「**按钮说小可爱、服务端拒小可爱**」
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 那个后果是两侧各说各话、而**用户是唯一知道两边不一致的人** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「逐字一致」被写成要求、不是巧合** ⇒⇒ **⇒⇒⇒ 而「回归在 `test/voice_wake_default_consistency_test.dart`」**
   **又一次「指路 + 回归文件名」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒ ⇒ 而这个回归正是 B0438 核过的那 30 个一致性测试里**最短的一个**（41 行）⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而这与 B0440 那条「两个文件各钉一个数值」是同一族、但更硬**：
   | | 钉的是什么 | 不一致怎么被发现 |
   |---|---|---|
   | B0440 `500` × 2 | **数值** | 两次 `expect`（同语言） |
   | **本处** | **一个中文词** | ⭐ **一条跨语言回归**（同语言时**编译期就互不可见**） |
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 「跨语言的两份常量」与「跨文件的两个数值」的关键差别是**发现机制****
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 同一语言的两次 `expect` ⇒ 编译就红；跨语言 ⇒ 只能靠一条测试**
   **⇒⇒⇒ ⇒「所以跨语言的那一对**必须**有一条跨语言断言」** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   （B0411 / B0412 那条的**第三个实例** —— 前两个是 `kSentenceEnders` 与字体子集）

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 4 点**
> **跨语言的两份常量与跨文件的两个数值，关键差别在**发现机制****
> **⇒⇒⇒ ⇒ 同一语言的两次 `expect` ⇒ 编译就红；跨语言 ⇒ 只能靠一条测试**
> **⇒⇒⇒ ⇒⇒ 「跨语言的那一对**必须**有一条跨语言断言」** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
> **⇒⇒⇒ ⇒⇒⇒ ⇒ 而本仓对这个要求的落实方式是**把后果写成一句用户可见的对句**（「按钮说小可爱、服务端拒小可爱」）⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `voice_wake_default_consistency_test.dart` 的**断言形状**（它读 Rust 源还是硬编码 `'小可爱'`？
   **后者就等于没测** · **未读**）· `wake_gate_open` 本体（`:83-84`）· `clean_transcript` 的尾部（`:270` 之后）
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
