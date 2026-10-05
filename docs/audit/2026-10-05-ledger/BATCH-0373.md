# BATCH-0373 · ✅ **「校验」在服务端**；UI 是**三层文案**的薄通道

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 模型登记的**「校验」**（B0372 留）

## 跑的命令（全部只读）
```
grep -rn "onImport:" lib/ --include=*.dart ; grep -rn "importModel|Future<void> _importModel|registerModel" lib/
sed -n '185,212p' lib/app/shell_admin.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **「校验」的落点确认在服务端，UI 是三层文案的薄通道**
```dart
Future<void> _importModel(String id) async {
  _busyId = id;  _adminMessage = '**正在导入 $id…**';  _refresh();            // :186-187  ① 在途可见
  try {
    await **_modelsApi.import(id)**;                                            // :189  ② 只发请求
    if (!mounted) return;
    _busyId = null;
    _adminMessage = '**已导入 $id，点「激活」即可换皮**';  _refresh();          // :192  ③ 成功带**下一步**
    await _loadAdmin();
  } on ApiException catch (e) {
    if (!mounted) return;  _busyId = null;
    _adminMessage = '**导入失败：${e.message}**';  _refresh();                  // :198  ④ 失败带**码**
  }
}
```
四个可核点：
1. ⭐ **「校验」在服务端** —— `_modelsApi.import(id)` **只是发请求** ⇒ ⇒
   真正的校验在 `model_root.rs` 那一侧（**B0001 核过 `find_model3_json` / `model_root.rs`**）
   ⇒ ⇒ ⇒ **Dart 侧没有校验** ⇒ 而 B0372 读的那句注释（「这里登记 + 校验」）**主语是整条流程**，
   **不是这个文件** ⇒ ⇒ **注释没有说谎，但会让人以为校验在 Dart 侧**
   ⇒ ⇒ ⭐ **又一条「该换个文件问」的注记**（B0316 规则第三次因「职责在别处」而适用）
2. ⭐⭐ **三层文案各司其职**：**在途**「正在导入…」· **成功带下一步**「点『激活』即可换皮」·
   **失败带码**「导入失败：${e.message}」（`ApiException.message` 含码 · B0159）
   ⇒ ⇒ **成功给的是「下一步」** ⇒⇒ **同 B0330/B0346/B0355 那条「反馈含后果」的家族** —— **这里是「下一步」而非「后果」**
3. ⭐ **`_busyId` 在两条分支上都复位**（`:191` 与 `:197`）⇒ **闩不会卡住**（同 B0355 核的 Mod 侧）
4. ⭐ 成功后 `await _loadAdmin()` ⇒ ⇒ **列表在动作后被重读** ⇒ 同 B0355 核的「立刻重取运行态」

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批把 B0372 那条注释的性质定了下来**：
**它没有说谎，但它省略了「校验在服务端」这一层** ⇒ ⇒ 而按我的判据（**P3 要「读者会因此做错事」**）——
**读者会少一层理解、不会做出错误操作** ⇒ ⇒ **仍不记发现** ⇒ ⇒ **只入未核实区的一条注记**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. ⭐ **服务端 `import` 的校验本体**（**B0373 定位到它不在 Dart 侧** ⇒ 下一批该问 `models_routes`）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
