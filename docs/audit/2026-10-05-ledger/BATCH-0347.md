# BATCH-0347 · ⭐ **作用域在函数名里**；而「停用还原」与「清掉本会话的卡」被**命令名**分开了

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `persona_panel` 的**导入/清除**派发（B0231 核过四种组合的存在）

## 跑的命令（全部只读）
```
sed -n '305,330p' lib/settings/mods/persona_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **「两个撤销」被区分**
```dart
await _runForSession('import_card', {'card_json': text});   // 绑**当前会话**     :311
await _run('import_card',          {'card_json': text});   // **全局**人设        :324
_runForSession('clear_import', const {});                  // 清**当前会话**绑定  :329
```
四个可核点：
1. ⭐⭐ **`_runForSession` 与 `_run` 承载了作用域** ⇒ ⇒ **作用域在函数名里，不在参数里**
   ⇒ ⇒ **每个存在的动作只有一个作用域版本**（`clear` 只有 session 版）
   ⇒ ⇒⇒ **「为什么没有 clear 的全局版」不需要额外解释 —— 它不存在**，
   **而每个存在的动作都只有一个作用域**
2. ⭐⭐ **本地判空 + 具体提示**（「先粘贴角色卡 JSON 再点导入」）
   ⇒ ⇒ 与 **B0330（编辑）/ B0346（导入）同形** ⇒ ⇒ **同族面板的同族动作写法一致**
3. ⭐ **`_clearSession` 是一行**，且「只清当前会话」**写在函数名里**
   ⇒ ⇒ 而 B0231 核过头注说的是「**会话绑定的卡不进主链**」⇒⇒ **UI 侧三个动作与头注三条一致**
4. ⭐⭐ **而 `clear_import` 与「停用」是两条不同的路**：
   B0220 核过 Mod 侧的**停用 = 还原到 `persona-mod-base.txt` 基线**（B0331 的 UI 文案：「关闭开关会**还原成
   主链 `system_prompt`**」）⇒ 而这里是 **只清掉当前会话的卡** ⇒⇒ **命令名把它们分开**
   ⇒⇒ ⭐ **同一个人心里的两个「撤销」被区分了**：**全局那个是「还原」· 会话那个是「清掉」**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的收获是「一个歧义被命令名消掉了」**：
**如果没有 `_run` / `_runForSession` 这个区分、也没有 `import_card` / `clear_import` 这个区分，
「关掉角色卡」会被理解成一件��而它其实是两件。**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~450 / `chat_panel` ~440）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
