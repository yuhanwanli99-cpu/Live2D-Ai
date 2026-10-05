# BATCH-0298 · 🔻 **我上一批的「承诺成立」认错了对象** —— 这是**两条不同的通道**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `main.dart:1217` 的 `onAck` 闭包本体（B0297 留）

## 跑的命令（全部只读）
```
sed -n '1212,1240p' lib/main.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；🔻 **更正我 B0297 的结论**
```dart
// 渲染面回执 → **缩放百分比**。**首屏也要有值**：过去只在上一次
// 放大/缩小时才读，于是初始状态一直显示「—」。                        // :1213-1215
onRenderEvent: _directorLog.add,
onAck: (StageAckEvent ack) {                                                 // :1217
  if (mounted && ack.scale != _stageScaleFromAck) { setState(() => _stageScaleFromAck = ack.scale); }
},
```
### ① ⇒ **这个 `onAck` 的用途是「缩放读数」，不是「换模型」**
⇒ ⇒ 我 B0297 写「AGENTS rc.2 ③ 的承诺**成立**」⇒ **那句话认错了对象**
⇒ ⇒ 而这个消费点本身**正是在修一个已记录的缺陷**（「初始状态一直显示『—』」）
  ⇒ **同 AGENTS rc.5 ⑧ 与 B0238 核的那个** ⇒ **三处（声明 / 消费 / 变更历史）现在都核到了**

### ② ⇒ ⭐ **而 AGENTS rc.2 ③ 说的那个「等回执」是另一条通道**
| 通道 | 语义 | 出处 |
|---|---|---|
| **`stage-ack` 流**（`onAck`） | 渲染面**对已应用参数**的回执 ⇒ **UI 的缩放读数** | `stage.dart:159` / `main.dart:1217` |
| **桥的 flush 回执** | `loaded`（**换成了**）/` error`（**没换成**）⇒ **换模型是否生效** | `bridge.dart:122` / `:166` |
⇒ ⇒ **两者都成立，但它们是两回事** ⇒ ⇒ AGENTS rc.2 ③ 指的是**后者**

### ③ ⇒ 所以**「等回执才说已切换」那半边仍未核**
B0297 核到的是**前者**；**后者的「等」**（调用方怎么在 `loaded` 之前**不说**「已切换」）
⇒ ⇒ **在 `live2d_bridge` 的余面里** ⇒ **下一批第一件事**

## 未核实项
1. ⭐ **「等回执才说已切换」的「等」** —— 未核（`live2d_bridge` 余面）
2. `live2d_bridge.dart` 余面未读 · `main.dart:1240+` 余面未读
3. `message_bubble.dart` 余约 460 · `live2d_stage` 余约 600 · `shell_admin` 余面 · `env_key_field` 余约 110 未读
4. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
5. `main.rs` 余约 930 行未读
6. 真实动作计划的 token 长度（F-0020-01 永久敞口）
7. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
8. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
9. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
10. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
11. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
12. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
13. Mod crates 42 未读；`mod-system` 余 8 文件未读
14. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
15. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
16. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
