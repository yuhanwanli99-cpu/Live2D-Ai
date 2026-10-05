# BATCH-0297 · ⭐ 承诺**成立**；而我**差点把它记成「无消费点」** —— B0274 规则第四次拦我

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 模型切换「等回执才说已切换」的 UI 侧

## 跑的命令（全部只读）
```
grep -rnE "已切换|loaded|ack|回执|sendSync\(model" lib/ --include=*.dart
grep -rn "acks\b|StageAckEvent|onAck" lib/ | grep -vE "live2d_bridge.dart"
grep -rn "onAck:" lib/ test/ --include=*.dart
grep -rn "切换" lib/ --include=*.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **AGENTS rc.2 ③ 的承诺**在代码里**成立**
### ① 三个环节都在
| 环节 | 位置 |
|---|---|
| **声明** | `live2d_stage.dart:159` `final ValueChanged<StageAckEvent>? onAck;`（B0238 核过：文档写了「缺了它会怎样」） |
| **调用** | `live2d_stage.dart:317` `widget.onAck?.call(ack);` |
| ⭐ **消费** | **`main.dart:1217` `onAck: (StageAckEvent ack) { … }`** |
⇒ ⇒ ⇒ **承诺成立**

### ② 而**桥那一层也写明了「必须等」的后果**（`live2d_bridge.dart`）
- `:122`「`loaded`（**换成了**），要么回 `error`（**没换成**）」⇒ ⇒ **两条分支都有名字**
- `:166`「**只发不等，界面就会在舞台还…**」⇒ ⇒ **不等的具体后果被点名**
⇒ ⇒ 与 B0288「失败归因到正确的那半」同族：**成功与失败各有各的表示**

### ③ ⭐⭐ 而本批真正的收获：**我差点记成一条错发现**
我第一次的 grep（`onAck`，**排除**了 `live2d_bridge.dart`）**只看到声明与调用、没看到消费**
⇒ ⇒ 我据此准备写「**`onAck` 无消费点 ⇒ AGENTS 那句承诺是旧的**」
⇒ ⇒ **第二次 grep（`onAck:`，含构造处）把它找出来了** ⇒ ⇒ **那是个错误，且会是一条**方向相反**的错发现**
⇒ ⇒ ⭐ **B0274 规则第四次拦我，且第二次专门拦「无消费点 / 不存在」这一类**
⇒ ⇒ **具体动作**：**从「声明/调用」转向「消费」时，要搜的是「构造处传参」那个模式**（`onAck:`），
**而不是继续搜同一个词** ⇒ ⇒ **同词搜索与「谁传给它」是两个模式**

## 未核实项
1. `main.dart:1217` 那个 `onAck` 闭包的**本体未读**（它怎么用 ack、怎么决定「已切换」文案）
2. `live2d_bridge.dart` 余面未读（`:122`/`:166` 附近的队列与回执解析）
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
