# BATCH-0317 · ✅ **两条结清**；而它们分别是那条「失败是答案」链的**第五层**与**判据来自类型**的又一例

Phase 1 · **未核实项清账（第 5 批）**

## 跑的命令（全部只读）
```
grep -rn "class ActionCue" lib/                                   # ⇒ lib/api/ws_frame.dart:131
grep -n "setActiveSession" -B 4 lib/api/api_client.dart
sed -n '124,150p' lib/api/ws_frame.dart ; sed -n '134,148p' lib/api/api_client.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；✅ **两条结清**
| 未核实项 | 结果 |
|---|---|
| `ActionCue` 定义 | ✅ `ws_frame.dart:131`，**13 个字段**（5 required + 8 可选） |
| `ApiClient.setActiveSession` 的说明 | ✅ 见 ① |

### ① ⭐⭐ `setActiveSession` = 那条链的**第五层**，而且**写明了失败在哪兑现**
> 「注入才会落到**用户正在看的**那个会话，而不是上一次发消息的那个」
> 「**失败不抛**：会话切换是**纯辅助信息**，不该因为后端不可达就挡住用户」
> `} catch (_) { // 刻意吞掉：见上。**真正的失败由下一次发送的 4xx/5xx 暴露。** }`
⇒ ⇒⭐⭐ **「失败不抛」+「失败在哪露出来」** 两句都在
⇒ ⇒ 与 B0302（存档失败 vs 本次会话）· B0288（写成功但读状态失败）·
B0316（连不上 = 一次成功自检）**同族** ⇒ **第五层**
⇒ ⇒ 而这一次**更进一步**：失败**被推迟到一个真正会疼的地方**（**下一次发送的 4xx/5xx**），
**而不是消失** ⇒ ⇒ ⭐ **「吞掉」与「丢弃」是两件事，前者可接受、后者不可**
⇒ ⇒ 判据也写明了：「**纯辅助信息**」⇒ ⇒ **又一处「按什么算重要」的显式判据**
⇒ ⇒ 且「`sessionId` 传 null / 空 = **清掉活动会话**（服务端按**全局桶**处理）」
⇒ ⇒ **与 B0220/B0231 核过的「按会话分桶 + 全局桶回落」口径一致**

### ② ⭐⭐ 而 `ActionCue` 的注释**理由来自类型系统**
> 「**为什么必须在这里解析**：`ActionCue` 是**封闭**类，`fromJson` **不认识的键会被直接丢掉**
> —— 不接住它们，`body` / `head` cue 就到不了舞台（**整条链断**）。」
⇒ ⇒ **「为什么必须」的答案不是品味，是「封闭类 + `fromJson` 丢未知键」这条类型后果**
⇒ ⇒ ⇒ **判据来自类型**（同 B0213 的 `.filter()` 那一族）⇒ ⇒ **与 B0228 那条链的六种形态同源**
⇒ ⇒ 而它与**协议 §11 / O13** 的关系也写明：「`id` 与 [presetId]：**`expression` 的表情面板 id
由这条通道承载；旧路径仍用 [presetId]**」⇒ ⇒ **新通道承载新语义、旧路径不动**（B0152「有理由的重复」的又一种）

## 未核实项（本批后仍开着 7 条）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · 两个 Timer · `_modelLoads` 注入点）· `main.dart:1240+`
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行
5. `requestAnimationFrame` 的**暂停语义**
6. 面板余面（`dev_tools_section` ~1845 / `live2d_stage` ~600 / `memory_panel` ~545 /
   `persona_panel` ~520 / `message_bubble` ~460 / `chat_panel` ~465）
7. **永久不可核**：`reqwest` `.timeout()` 是否覆盖 body 读取（禁止运行）·
   动作计划的 token 长度（仓库里拿不到）· `_finishTurn` 是否幂等 · `secrets.rs` 断言体 ·
   B0120「chmod toml」是否有别处 · GitHub 侧 secret scanning（**仓库设置**）
