# BATCH-0300 · ⭐ **我怀疑的「不对称」不成立** —— 三个「长期累积」各用**对**的手段闭合，且理由**是定量的**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 聊天会话的存储层边界

## 跑的命令（全部只读）
```
grep -rnE "maxMessages|MAX_MESSAGES|trim|removeRange|\.removeAt|上限" lib/chat/chat_session.dart lib/chat/chat_controller.dart
sed -n '150,158p' lib/chat/chat_session.dart
grep -rn "kMaxMessagesPerSession" lib/
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **「三个累积物」全部闭合，且第四个（总体积）也闭合**
### ① 存储层**确实有界**，且**与视图同名同值**
```dart
/// 每个会话保留的消息条数上限，超了丢**最旧**的。                     // :150
/// 与视图层的 `kChatHistoryLimit`（500）**取同一个值**：两处不一致会出现
/// 「**界面上能看到但重开就没了**」这种**最难查的偏差**。                 // :151-153
static const int kMaxMessagesPerSession = 500;                        // :154
…
while (session.messages.length > kMaxMessagesPerSession) { … }        // :366  ⭐ **裁剪真的实现了**
```
⇒ ⇒ **视图裁 + 数据裁，两层都做** ⇒ ⇒ **B0249 判定的「重复」在此被**成对**使用**：两处声明 + 两条裁剪

### ② 而**总体积**是**双重夹持**，且**对着配额算过**
- `browser_io.dart:82`：「用 `maxSessions` × `kMaxMessagesPerSession` **双重夹持**把体积**…**」
- `chat_session.dart:147`：「**即使每个都到 `kMaxMessagesPerSession` 也远在配额内**」
⇒ ⇒ ⭐ **最坏情况被算过**，不是「希望不会太大」

### ③ ⇒ **「三个长期累积」各用对的手段**
| 对象 | 手段（管的是哪一类成本） | 核验 |
|---|---|---|
| **日志列表** | **虚拟化**（view 成本） | B0227 |
| **观察者缓冲区** | **上界 + 上界可见**（内存成本） | B0240 |
| **聊天历史** | **视图 500 + 数据层 500**（view + 存储 **都**管） | B0243 + **本批** |
| **总体积** | **`maxSessions` × `kMaxMessagesPerSession` 双重夹持 + 最坏情况算过** | **本批** |
⇒ ⇒ **0 findings**；而这一批的价值是**把我自己的怀疑证伪了**

### ④ ⭐ 而它与 B0249 一起，给出一条新的区分
| 什么可以重复 | 什么不能少 |
|---|---|
| **两处 `const` 声明**（B0249：收敛会**层间倒挂** ⇒ 重复更便宜） | **两条裁剪**（本批：数据层与视图层**各管一类成本**） |
⇒ ⇒ **声明可以重复、执行不能重复** —— 这是「重复可接受」这条判据的**另一半**
⇒ ⇒ 且 `:151-153` **点名了不同步的症状**（「界面上能看到但重开就没了」）⇒⇒ **B0270 那一课的**反向**应用**：
B0270 是「文档叫你手写 ⇒ 重建不许丢」；这里��「文件是程序自己的 ⇒ 裁剪要护住不变量」

## 未核实项
1. `chat_session.dart:360-370` 的裁剪体未读（`:366` 的 `while` 里具体删什么）· `browser_io.dart:82` 的配额数字未读
2. `message_bubble.dart` 余约 460 · `live2d_stage` 余约 600 · `shell_admin` 余面 · `env_key_field` 余约 110 未读
3. `live2d_bridge.dart` 余面未读（队列 · 两个 Timer · `_modelLoads` 注入点）· `main.dart:1240+` 余面未读
4. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
5. `main.rs` 余约 930 行未读
6. 真实动作计划的 token 长度（F-0020-01 永久敞口）
7. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
8. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
9. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
10. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
11. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
12. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` **裁剪体**未读
13. Mod crates 42 未读；`mod-system` 余 8 文件未读
14. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
15. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
16. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
