# BATCH-0301 · ⭐⭐⭐ **「配额满不是假设」** —— 一句注释把不可验证的上限**升级成算过的事实**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_session` 的裁剪体 + `browser_io` 的存档写入（B0300 留）

## 跑的命令（全部只读）
```
sed -n '362,372p' lib/chat/chat_session.dart
sed -n '78,90p' lib/app/browser_io.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **B0300 的两个未核项全部关闭**
### ① 裁剪体与文档一致
```dart
static void _trimMessages(ChatSession session) {          // :365
  while (session.messages.length > kMaxMessagesPerSession) { session.messages.removeAt(0); }
}
```
⇒ ⇒ **丢的是最旧的**（`removeAt(0)`）⇒ ⇒ **与「超了丢最旧的」逐字一致** ⇒ **B0300 关闭**

### ② ⭐⭐⭐ 而这一段是**「把假设变成可核事实」的范本**（`browser_io.dart:79-83`）
> 「写入聊天会话存档（**失败静默**：无痕模式 / 配额满**都会抛**）。
> **配额满不是假设**：localStorage 常见上限 **5 MB**，而 `ChatSessionStore`
> 用 `maxSessions` × `kMaxMessagesPerSession` **双重夹持**把体积压在配额内。
> 即便如此，**写失败也只丢「这次之后的新记录」，不该影响正在进行的对话**。」

四个可核点：
1. ⭐ **失败原因被穷举**：「**无痕模式 / 配额满**都会抛」⇒ ⇒ 不是笼统的「可能失败」
2. ⭐⭐⭐ **「配额满不是假设」** ⇒ ⇒ **一句注释把一个不可验证的上限升级成被算过的事实**
   ⇒ ⇒ 且**它给了那个数**（**常见上限 5 MB**）⇒ ⇒ **有出处、有数字**
   ⇒ ⇒ ⭐ 与 **B0164 家族的反面应用**：B0164 是「保守启发式 + **声明误判代价**」；
   **这里是「把不可验证的配额当成已知数，并用结构把它压下去**」
3. ⭐⭐ **失败的后果被限定到一个明确范围**：「写失败也只丢『**这次之后的新记录』**，**不该影响正在进行的对话**」
   ⇒ ⇒ 与 B0288「**失败归因到正确的那半**」同族，**且更具体**（丢的是「新记录」，活的是「进行中」）
4. ⭐ `try { … } catch (_) {` ⇒ ⇒ **真的 catch 了**（不是空 `catch` 体）

⇒ ⇒ **0 findings**；而 ② + ③ 合起来是本审计见过最强的一组「**未知 → 有数 → 结构兜住 → 残余失败被限定**」

## 未核实项
1. `catch (_) {` 之后**具体做什么**未读（是否通知用户 / 只是记日志）—— B0293 核过「失败不能静默」，
   而这里**明确写了「失败静默」** ⇒ **两者是否矛盾，未核** ⇒ **下一批第一件事**
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
12. **两侧措辞同步无机制**（B0260 敞口）
13. Mod crates 42 未读；`mod-system` 余 8 文件未读
14. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
15. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
16. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
