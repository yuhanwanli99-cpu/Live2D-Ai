# BATCH-0299 · ⭐⭐ **「必须等」不是纪律，是 `Future<bool>` 的签名** —— 而 `true` 的判据要**对上号**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `live2d_bridge.dart` 的换模回执机制（B0298 定位到）

## 跑的命令（全部只读）
```
sed -n '116,170p' lib/live2d/live2d_bridge.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **B0298 留的「等」那半边，核到了，且是本审计最强的一处「纪律即类型」**
```dart
/// 换模的**唯一权威回执**：…要么回 `loaded`（换成了），要么回 `error`（没换成）。
/// **收条之前不得说「已切换」** —— 这是 `live2d_stage.dart` 里那条 **R2 规矩**的同一个道理。  // :120-122
/// 协议 v1 `sync` 换模型，并**等到渲染面回执**才返回（**R2 规矩**）。
/// 返回 `true` = 渲染面回了 `loaded`，**且报的正是我们发出去的那个 url**（**真换了**）；
/// `false` = 超时、渲染面报错，或**回执报的是别的 url**（**没换成**）。
/// 为什么必须等：`POST /models/{id}/activate` **只改后端 registry**，**真正换皮发生在这个 iframe 里**。
/// **只发不等，界面就会在舞台还没换的时候说「已切换」** —— 正是 rc.2 要消灭的「**激活了但没换皮**」。  // :160-167
Future<bool> swapModel(String url, {Duration timeout = const Duration(seconds: 15), …});
```
五个可核点：
1. ⭐⭐⭐ **`Future<bool> swapModel` 是 `await` 到回执才返回的** ⇒ **调用方拿不到「已发出但未确认」的结果**
   ⇒ ⇒ ⭐ **「只发不等」在类型上就做不到** —— **纪律写进了签名**
   ⇒ ⇒ 与「恰一次」纪律（B0132/B0156 靠**断言**守住）同族，但**这是更强的一档：靠类型守住**
2. ⭐⭐ **`true` 的判据是「回了 `loaded` **且报的正是我们发出去的那个 url**」**
   ⇒ ⇒ **回执要「对上号」** ⇒ ⇒ 一个**迟到的、属于上一次切换的** `loaded` **不会被误当成这次成功**
3. ⭐ **`false` 的三条原因被并列**：超时 / 渲染面报错 / **回执 url 对不上** ⇒ ⇒ **全归到一个 `false`**
4. ⭐⭐ **理由写到了机制层**：「activate **只改后端 registry**，**真正换皮发生在这个 iframe 里**」
   ⇒ ⇒ 而「只发不等」的后果**逐字点名**，且**指名它要消灭的那个缺陷名**（「激活了但没换皮」）
5. ⭐ **它说这条与 `live2d_stage.dart` 的 R2「是同一个道理」** ⇒ ⇒ **第六次「指路」**

### ⑥ 顺带：`_lastSentStageBg` 的注释（`:134-138`）**又是一个「排过队 ≠ 已送达」的记录**
> 「ready 前的发送是**入队**，而队列会被 `maxQueue` **截断**（`_queue.removeAt(0)`）——
> 把『**排过队**』当成『**已送达**』就会出现『**用户看着有壁纸，重挂 iframe 之后永远是黑底**』」
> 「见 `sendStageBg` 与 `data/background_decode_cache.dart` **同族的 F-0002-1**」
⇒ ⇒ **点名了具体失败症状**，且 **把自己的历史发现按族引用了** ⇒ ⇒ ⭐ **又一条「记过的事实至今可被查到」**

## 未核实项
1. `live2d_bridge.dart` 余面未读（队列实现 · `_readyTimer`/`_mouthTimer` · `_modelLoads` 的注入点）
2. `main.dart:1240+` 余面未读
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
