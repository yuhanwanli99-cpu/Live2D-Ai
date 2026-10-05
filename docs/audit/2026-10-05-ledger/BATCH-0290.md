# BATCH-0290 · ✅ **声明成立**；而执行体里有**一件注释没写**的事

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `_attach` 的执行体（B0289 留的「声明 vs 行为」核验）

## 跑的命令（全部只读）
```
grep -nE "_attach|_applyPrefs|重放|replay" lib/live2d/live2d_stage.dart
sed -n '330,352p' lib/live2d/live2d_stage.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **「声明 vs 行为」这一项结清，且多找到一个没写进注释的处置**
```dart
void _attach(Live2DTransport transport) {
  if (!mounted) { unawaited(**transport.dispose()**); return; }          // :331-334  ⭐⭐
  final previous = _bridge;
  if (previous != null) { **previous.removeListener(_onBridgeChanged);**  // :337  ⭐ 先摘
                          previous.dispose(); }                        // :338  ⭐ 再释放
  unawaited(_renderSubscription?.cancel());                             // :340  ⭐
  final bridge = Live2DBridge(transport)..addListener(_onBridgeChanged);
  _renderSubscription = bridge.renderEvents.listen(_onRenderEvent);
  _bridge = bridge;  setState(() {});  bridge.start();
  bridge.sendSync(model:…, scale:…, dark:…, stageColor:…, actionScales:…);  // :347-353  ⭐ 重放
```
五个可核点：
1. ⭐⭐⭐ **`if (!mounted)` 时的处置是「把刚到手的 transport 还回去」**（`:331-334`）
   ⇒ ⇒ **「widget 已经没了」的回调里最容易漏的就是这个刚到手的资源** ⇒ **它被显式 `dispose` 了**
   ⇒ ⇒ **而这不是「清空」或「忽略」，是「还回去」** ⇒ 同 B0288 的「**失败归因到正确的那半**」
   ⇒ ⇒ ⭐ **这件事没有任何注释说明，只有代码做了** ⇒ **代码说出了注释没说的话**
2. ⭐ **旧桥的处置有顺序**：`removeListener` → **再** `dispose` ⇒ **先摘监听再释放**（反了会在 dispose 后收到回调）
3. ⭐ **旧的 render 订阅被显式 `cancel()`**（`:340`）⇒ ⇒ **`:200` 的「随旧桥一起取消」在执行体里兑现了**
4. ⭐ **重放确实发生**：`:347-353` **五类值**（model / scale / dark / stageColor / actionScales）在每次挂桥时全量重发
   ⇒ ⇒ **`:132` 的声明成立** ⇒ **B0289 的未核项关闭**
5. ⭐ `:123`「现在 `_attach` **每次挂桥都重发一次**，与 `stageColor` **同一条纪律**」
   ⇒ ⇒ 「同一条纪律」指的是 **B0120 / B0237 那条「配置快照必须与实际一致、外部改动要能自愈」的家族** ⇒ **理由链完整**

### ⇒ ⭐ 而第 1 点与 B0287 **方向相反、同源**
B0287：**有理由才清**（写成功 ⇒ 明文没有存在的理由 ⇒ 清）；本批：**没主体就还回去**（widget 已卸载 ⇒ 这个 transport 无人持有 ⇒ 还）
⇒ ⇒ **两条都是「资源归属于谁」的正确处理** ⇒ ⇒ ⭐ **可提炼**：
> **处理资源的两条基本问句是：「它现在还需要存在吗」与「谁还持有它」。**
> **大多数泄漏与误清都源于只问了其中一条。**

## 未核实项
1. `live2d_stage.dart` 余约 600 行未读
2. `env_key_field.dart` 余约 110 行 · `shell_admin.dart` 余面未读
3. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
4. `main.rs` 余约 930 行未读
5. 真实动作计划的 token 长度（F-0020-01 永久敞口）
6. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
7. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
8. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 ·
   `message_bubble` ~470 · `chat_panel` ~465
9. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
10. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
11. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
12. Mod crates 42 未读；`mod-system` 余 8 文件未读
13. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
14. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
15. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
