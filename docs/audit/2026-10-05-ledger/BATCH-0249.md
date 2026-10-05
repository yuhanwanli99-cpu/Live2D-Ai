# BATCH-0249 · ⭐ `kChatHistoryLimit` 是**两份声明**（非引用）—— 而**收敛的代价更大**，故重复是对的

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `kChatHistoryLimit` 的**施加点**与**第二个站点**

## 跑的命令（全部只读）
```
grep -rn "kChatHistoryLimit" lib/
sed -n '148,160p' lib/chat/chat_session.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **一条对 P25 / 模式 F 判据的精化（本轮最有价值的产出）**
### ① 施加点与文档**一致**
```dart
// 只渲染最后 [kChatHistoryLimit] 条（**不**在数据层裁剪：…）          // chat_panel:142
final List<ChatMessage> visible = messages.length > kChatHistoryLimit
    ? messages.sublist(messages.length - kChatHistoryLimit) : …        // :144-145
```
⇒ **视图层裁剪、保留最新** ⇒ 与 B0243 核的文档（「超出后**丢最旧的**」「刻意在视图层裁剪而不是数据层」）**完全一致** ⇒ **文档说的做了**

### ② 而第二处是**另一份声明**（不是引用），且**它写明了不同步的后果**
```dart
/// 每个会话保留的消息条数上限，超了丢**最旧**的。
/// 与视图层的 `kChatHistoryLimit`（500）**取同一个值**：两处不一致会出现
/// 「**界面上能看到但重开就没了**」这种**最难查的偏差**。            // chat_session:150-154
static const int kMaxMessagesPerSession = 500;                        // :155
```
⇒ ⇒ `chat_panel:32` 与 `chat_session:155` 是**两份 `const` 声明** ⇒ **不是引用**
⇒ ⇒ 按 B0152 的判据：这是**候选重复**

### ③ ⭐⭐ 但核完发现：**它不构成缺陷，而且「收敛」的代价更大**
若让 `chat_session.dart` **import** `ui/chat_panel.dart` 的常量以保证一致
⇒ ⇒ **`data/` 层就会依赖 `ui/` 组件文件** ⇒ ⇒ **层间反向依赖**
⇒ ⇒ **那比「两个 500」更糟**
⇒ ⇒ ⇒ **重复不是「被容忍的缺陷」，而可能是两个不完美选项里更便宜的那个**
⇒ ⇒ 而且**文档没有假装耦合是免费的** —— 它**点名了不同步的症状**（「界面上能看到但重开就没了」）
并**自评其难度**（「**最难查的偏差**」）

⇒ ⭐ **判据精化（对 P25 / 模式 F）**：
> **「重复是否可接受」不只取决于「有没有理由」，还取决于「收敛的代价」。**
> **若收敛会引入层间反向依赖，则重复是更便宜的那个选项。**
⇒ ⇒ ⇒ **P25（「让各处都是引用」）不是无条件的** —— 它**在能引用、且引用不造成层间倒挂时**才成立。
⇒ ⇒ 而本例的文档**恰好停在正确的诚实度上**：**点名症状，而不是假装零成本。**

## 未核实项
1. `chat_session.dart` 的 `maxSessions = 50` 的**截断施加点**未读（`get active` 之后）
2. `dev_tools_section.dart` 余 ~1850 · `live2d_stage.dart` 余 ~620 · `memory_panel.dart` 余 ~560 ·
   `persona_panel.dart` 余 ~520 · `message_bubble.dart` 余 ~480 · `chat_panel.dart` 余 ~480 ·
   `error_banner.dart` 余 ~15 未读
3. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
4. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
5. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
6. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
