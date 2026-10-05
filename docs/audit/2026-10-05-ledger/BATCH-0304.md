# BATCH-0304 · ⭐ 流式中切会话：**先 `_abandonTurn()`，而「切会话」同时还改变了注入的归属**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_controller.selectSession`

## 跑的命令（全部只读）
```
grep -nE "switchSession|selectSession|activeId|setActive" lib/chat/chat_controller.dart
sed -n '520,536p' lib/chat/chat_controller.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **一个没查过的状态问题，答案是「先放弃」**
```dart
void selectSession(String id) {
  if (sessions.active?.id == id) return;   // :525  ① 切到当前 = 什么都不做
  _abandonTurn();                          // :526  ② ⭐ **放弃当前这一轮**
  if (!sessions.select(id)) return;        // :527  ③ 切不过去就停
  _persist();  _syncActiveSession();  notifyListeners();   // :528-530
}
```
五个可核点：
1. ⭐⭐ **`_abandonTurn()` 在切换之前** ⇒ **流式中的那一轮被放弃** ⇒ **那一轮的文字不会落到新会话里**
   ⇒ ⇒ ⭐ 而这**接上 B0248 那个链**：`text_fallback` 是「**覆盖式**整段正文」+ `unfinished` 标记
   ⇒ ⇒ ⇒ **「放弃」在 B0248 的语境下有确切含义** —— 那一轮**带着「未收尾」的标记留在旧会话里**
   ⇒ ⇒ **不是被静默搬走、也不是被静默删除** ⇒ ⇒ **与 B0288「失败不是被吞掉的东西，它被写下来了」同向**
2. ⭐ **切到当前会话直接 `return`** ⇒ **重复点击无害**
3. ⭐ **切换失败（`sessions.select` 返回 `false`）就停** ⇒ **不 `_persist`、不同步服务端** ⇒ **失败不半途生效**
4. ⭐⭐ 而 `_syncActiveSession()`（`:533-537`）说明了**为什么**：
   「只影响**不带会话 id 的注入路径**（external-input 弹幕 / voice-input 转写）：
   **它们接在当前这段对话上**，所以必须知道用户正在看哪个会话」
   ⇒ ⇒ ⭐⭐ **所以「切会话」不只是 UI 行为** —— **它改变了注入的归属**
   ⇒ ⇒ 而这是 B0221 / B0231 核过的**「按会话分桶不串味」**链的**前端一环**
5. ⭐ `:539`「**失败不抛**（见 `ApiClient.setActiveSession` 的说明**）」⇒ ⇒ **第七次「指路」**

⇒ ⇒ **0 findings**；⭐ 而 ① + ④ 合起来是一句结论：
> **「切会话」在语义上不是一个纯 UI 动作**（它同时「放弃当前轮」+「改变注入归属」），
> **而代码没有假装它是纯 UI 动作** —— 两个副作用**写在同一个连续块里、没有藏**。

## 未核实项
1. `_abandonTurn()` 的**本体未读**（它到底把那一轮标成什么、有没有发 `_finishTurn`）
   ⇒ ⚠ **这是 B0247/B0248 链上**我一直**没读**的那个函数 ⇒ **下一批第一件事**
2. `ApiClient.setActiveSession` 的**说明本体**未读（`:539` 指过去的那处）
3. `message_bubble.dart` 余约 460 · `live2d_stage` 余约 600 · `shell_admin` 余面 · `env_key_field` 余约 110 未读
4. `live2d_bridge.dart` 余面未读 · `main.dart:1240+` 余面未读
5. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
6. `main.rs` 余约 930 行未读
7. 真实动作计划的 token 长度（F-0020-01 永久敞口）
8. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
9. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
10. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
11. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
12. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
13. **两侧措辞同步无机制**（B0260 敞口）
14. Mod crates 42 未读；`mod-system` 余 8 文件未读
15. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
16. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
17. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
