# BATCH-0425 · ⭐⭐⭐⭐ **两个降级选了不同方向，而只有一半写了理由**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— **`_devMode` 的翻转传播**（B0424 留）

## 跑的命令（全部只读）
```
sed -n '16,32p' lib/app/shell_admin.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**两条理由的层级不同**（0 条新发现）
```dart
Future<void> _loadAppStatus() async {
  try {
    final status = await _api.fetchStatus();
    final bool dev = status['dev_mode'] == true;
    if (mounted && **dev != _devMode**) { _devMode = dev; _refresh(); }    // :20-23  ⭐ 只在真翻转时刷新
  } catch (_) {
    // **忽略：保持 false。**                                                    // :25
  }
}
```
四个可核点：
1. ⭐⭐⭐⭐ **`dev != _devMode` 是去抖**：**不是每次轮询都刷新**
   ⇒⇒⇒⭐⭐ **⇒ 「读状态」与「推送变化」被分开** ⇒⇒⇒ **⇒ B0342 核的「条件成立才给回调」的读侧**
2. ⭐⭐⭐⭐⭐ **而 `catch` 写的是「忽略：保持 false」** ⇒⇒ 与 B0415 核的「`/env` 失败**沿用旧值**」形成对照：
   | | 失败时 | 注释写的是 | 层级 |
   |---|---|---|---|
   | `/env`（B0415） | **沿用旧值** | 「**不该让整个面板报错**」 | ⭐ **后果** |
   | `fetchStatus`（本批） | **保持 `false`** | 「**忽略：保持 false**」 | ⚠⭐⭐ **当前值** |
   ⇒⇒⇒⭐⭐⭐ **⇒ 两句理由的**层级不同**：「不该让整个面板报错」说的是**后果**；
   「保持 false」**只是复述了结果** ⇒⇒⇒⭐⭐⭐ **⇒ ⇒ 后者没有回答「为什么这样选是对的」**
3. ⭐⭐⭐⭐ **而「status」在这个仓里至少**三个意思**、分处三处、没有统一命名**：
   | 出现处 | 指什么 | 端点 |
   |---|---|---|
   | `_api.fetchStatus()` | **应用状态**（`dev_mode`） | `/api/v1/app/status` |
   | `_diagApi.status()` | **渲染面/模型状态**（`active_model_id`） | `/api/v1/status` |
   | `runtime_status`（B0333 核） | **运行时状态**（WS 事件） | WS 帧 |
   ⇒⇒⇒⭐ **⇒ 三个 `status` 同形不同源** ⇒⇒ **⇒ 而它们分属三层架构 ⇒ 混淆风险低**
   ⇒⇒⇒ **⇒ 按判据不记 FINDINGS**，记为一条观察
4. ⭐⭐ **而 `_loadAppStatus` 与 `_loadAdmin` 是两个方法** ⇒⇒⇒⭐ **⇒ 应用状态与管理面数据分开取**
   ⇒⇒ **⇒ 同 B0415 核的「降级①在两次刷新之间、不额外刷新」的手法**

⇒ ⇒⭐⭐⭐⭐ **而本批最值钱的是第 2 点**
> **同一个仓里两个降级选了不同方向，而只有一半写了理由** ⇒⇒
> **⇒ 「保持 false」这句话**陈述了做法、没陈述理由**** ⇒⇒⇒⭐⭐
> **⇒ ⇒ 而这一条正是 B0348 那条「把『我们没修』与『为什么』写下来」的最小对照面** ——
> **⇒⇒⇒ **「为什么」的最低形态不是解释，是「这样选是安全的」那一句** ⇒⇒⇒⭐⭐⭐

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `_api.fetchStatus()` 的**端点与重试策略**（**未读**）· `AppShell._currentBackground` 本体 ·
   `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
