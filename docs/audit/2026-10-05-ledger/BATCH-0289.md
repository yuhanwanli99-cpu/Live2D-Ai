# BATCH-0289 · ⭐ 桥接队列：**「队列随旧桥销毁」这个危险被点名了**，而自愈靠「重放当前状态」

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `live2d_stage.dart` 的桥接生命周期与消息队列

## 跑的命令（全部只读）
```
grep -nE "队列|queue|入队|flush|重建|rebuild|retry|_pending" lib/live2d/live2d_stage.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **一个被点名的危险**
```dart
/// 渲染面就绪回调（**每次**进入 ready 都触发，含错误后 [Live2DStageState.retry] 重建 iframe）。  // :168-169
/// … **loading，过早 `sync` 只能入队；重建 iframe 后队列已随旧桥销毁。**                          // :172
```
四个可核点：
1. ⭐⭐ **「重建 iframe 后队列已随旧桥销毁」** ⇒ ⇒ **队列的生命周期绑在「桥」上，不是绑在 widget 上**
   ⇒ ⇒ ⇒ **一个比「桥」活得久的队列，会静默丢掉全部内容** ⇒ ⇒ **他们把这条危险写在了回调的文档上**
2. ⭐ **自愈机制因此被写明**：`:131-132` 偏好快照是「**iframe 重建 / 重挂后的自愈快照**」，
   舞台在 **`_attach`（首帧 / retry 重挂）** 与 `didUpdateWidget`（值变化）时应用它
   ⇒ ⇒ ⇒ **设计形态是「状态住在 widget、队列是每桥的传输、重挂时重放当前状态」**
3. ⭐ **队列语义被限定**：`:214`「桥会把它们**按序 flush**；渲染面**只认最后一个**」
   ⇒ ⇒ **所以「flush 全部」是安全的**（过期的中间态会被最后一个覆盖）
   ⇒ ⇒ ⭐ **而这正是「重放一切」得以成立的前提** —— 两句话必须成对
4. ⭐ **两个阶段被命名**：`:346`「**ready 前会入队，ready 后按序 flush**」

⇒ ⇒ **0 findings**；而这是「**不要推断、去读真源**」家族的又一实例：
**状态在 widget、传输在桥、真相靠重放** —— 与 B0238（`presetStatus` 只由 ack 驱动）·
B0247（`unfinished` 落盘以免「刷新后还在」变假）**同族**

## 未核实项
1. ⭐ **`_attach` 的重放是否真的重放全部**（`:132` 是**声明**；**执行体未读**）⇒ **声明与行为是否一致，未核**
2. `live2d_stage.dart` 余约 600 行未读
3. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
4. `env_key_field.dart` 余约 110 行 · `shell_admin.dart` 余面未读
5. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
6. `main.rs` 余约 930 行未读
7. 真实动作计划的 token 长度（F-0020-01 永久敞口）
8. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
9. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
10. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 ·
    `message_bubble` ~470 · `chat_panel` ~465
11. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
12. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
13. Mod crates 42 未读；`mod-system` 余 8 文件未读
14. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
15. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
16. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
