# BATCH-0415 · ⭐⭐⭐⭐ **两次嵌套降级** —— 而这补上了 B0392 那条原则的**另一半**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `shell_admin._loadAdmin` 的**失败分支**

## 跑的命令（全部只读）
```
sed -n '/Future<void> _loadAdmin/,/^  }/p' lib/app/shell_admin.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**四个调用、两种失败语义**（0 条新发现）
```dart
_adminLoading = true; _adminError = null; _refresh();
try {
  models = await _modelsApi.list();  mods = await _modsApi.list();
  status = await _diagApi.status();  caps = await _diagApi.capabilities();
  // 密钥真源状态（键名 + 是否已设置）。**永不包含值**。
  // **失败不连坐**：`/env` 挂了不该让整个管理面板报错，所以单独兜底。
  EnvStatus env = _envStatus;
  try { env = await _envApi.list(); } on ApiException { env = _envStatus; }      // ① 降级
  …
  _adminLoading = false; _refresh();
  // 日志只有 dev_mode 才可读——**分开取**，403 不该让整个面板失败。
  await _loadLogs();                                                               // ② 降级
} on ApiException catch (e) { _adminError = e.toString(); _adminLoading = false; _refresh(); }
```
四个可核点：
1. ⭐⭐⭐⭐ **两次嵌套降级，各有各的理由**：
   | 降级 | 失败什么 | 处置 | 理由（逐字） |
   |---|---|---|---|
   | ① | `/env` | **沿用 `_envStatus` 旧值** | 「**失败不连坐**：`/env` 挂了不该让**整个管理面板**报错」 |
   | ② | `/api/v1/logs` | **它自己的 403 分支自己处理** | 「日志只有 dev_mode 才可读——**分开取**，403 不该让整个面板失败」（B0352 已核其 403 分支） |
   ⇒⇒⇒⭐ **⇒ 而「整块失败」只留给前三个调用** ⇒⇒ **⇒ 四个调用、**两种失败语义****
2. ⭐⭐⭐⭐⭐ **而「不连坐」这个词选得很准** ⇒⇒ **它说的是「一个子系统的失败不该让无关的子系统也报红」**
   ⇒⇒⇒⭐⭐ **⇒ 而这补上了 B0392 那条原则的**另一半**：
   - **B0392**：「**降级不能压过错误**」（**错误不能被「降级成正常」盖住**）
   - **本批**：「**降级不能把无关的也一起盖住**」
   ⇒⇒⇒⇒⭐⭐⭐ **⇒ 两半合起来才是完整的一句**：
   > **「降级要只降该降的那一处，且降级的痕迹要看得见。」**
3. ⭐⭐ **`_adminError = e.toString()`** ⇒⇒ **错误原文上屏**（同 B0352 的「原文照显」）
4. ⭐⭐ **两次 `_refresh()`（开始 / 结束）而降级①在两次之间、**不额外刷新****
   ⇒⇒ **⇒ 与 B0346 核的「成功才清 `_importController`」同一手法**：**中间态不触发界面动作**

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐ **而本批的收获是「补上另一半」**
⇒ ⇒ **⇒ 我在 B0392 记下「降级不能压过错误」时，以为自己已经掌握那条原则**
⇒ ⇒⇒⭐ **⇒ 而那只说了「降级不许盖住错误」，没���「降级不许盖住别的」**
⇒ ⇒⇒⭐⭐ **⇒ 两次都做对、而我只看见一次 ⇒ 这与 B0369「凭片段」是同一个形状**：
**⇒ 我把一条原则的**一半**当成了全部**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
