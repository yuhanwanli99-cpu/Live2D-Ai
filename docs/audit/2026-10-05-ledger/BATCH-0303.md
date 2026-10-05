# BATCH-0303 · ✅ **「本次会话」的粒度是「每次保存」** —— 而那半句注释**是精确的，不是松的**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_controller` 的保存时机（B0302 明确「不断言」的那一项）

## 跑的命令（全部只读）
```
grep -rn "saveChatSessions|persistSessions" lib/ --include=*.dart
sed -n '604,620p' lib/chat/chat_controller.dart
grep -n "_persist" lib/chat/chat_controller.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；✅ **B0302 里我拒绝肯定的那一句，现在有证据了**
```dart
void _persist() {                                   // :610
  final void Function(ChatSessionStore store)? write = persistSessions;
  if (write != null) write(sessions);               // :612  每次**整体**写
}
```
⇒ ⇒ `_persist()` 出现在 **8 个以上站点**（`:98 :161 :197 :500 :518 :528 …`）⇒ ⇒ **保存是**按状态变化**发生的，不是「每会话一次」**
⇒ ⇒ **而每次 `write(sessions)` 写的是整个 blob** ⇒ ⇒ **失败一次 = 到下一次成功为止的记录都没落盘**
⇒ ⇒ ⇒ **这正是那半句注释说的**：「写失败也只丢『**这次之后的新记录**』」⇒ ⇒ **注释精确**
⇒ ⇒ 且 `browser_io` 那半句「**存储不可用不影响本次会话**」里的「本次会话」
⇒ ⇒ **指「你此刻正在进行的这场对话（内存里的那份）」** ⇒ ⇒ **两半句互相印证**
⇒ ⇒ ⇒ **0 findings**；**边界判据 = 每次保存**，而**文档两半都对**

### ② 顺带一个**闭环**：`_persist()` 出现在 `:608` —— **紧接在「本轮失败」之后**
```dart
_error = message;  _errorCode = code;  _persist();     // :606-608
```
⇒ ⇒ **失败的轮次也会落盘** ⇒ ⇒ 与 **B0247 的「未收尾」落盘链**一致
⇒ ⇒ ⭐ **失败本身也被记录** ⇒ ⇒ 与 B0288「失败归因到正确的那半」**同向**：
**失败不是被吞掉的东西，它被写下来了**

## 未核实项
1. `_persist()` 的**全部 8+ 站点**未逐一核（只核了 `:608`）⇒ 「按状态变化」的覆盖面**大体**成立、
   **穷举**未做
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
