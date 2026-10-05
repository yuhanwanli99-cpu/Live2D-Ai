# BATCH-0324 · ⭐ persona 面板承诺的「坏卡失败处置」**兑现**，而第三条带一条**作用域声明**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `persona_panel` 的三条提示

## 跑的命令（全部只读）
```
grep -nE "失败|坏|error|errorCode|处置|回滚|原来" lib/settings/mods/persona_panel.dart
sed -n '40,48p' lib/settings/mods/persona_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **头注承诺兑现，且「三条提示都走文字」是同一条纪律的第三次应用**
```dart
/// # **三条提示都走文字**
/// 1. 状态 failed / error → 「**角色卡没被接受，修好再打开开关**」+ 日志 `mod: persona`   // :42
/// 2. 导入失败 → 带**错误码**（`command_failed` / `unsupported_command` / `command_unavailable`…）// :43
/// 3. 与 memory 共存 → last-writer-wins（后写覆盖，不做仲裁）；**会话绑定的卡不进主链，
///    因此不受这条影响**。                                                          // :44-45
```
三个可核点：
1. ⭐ **失败提示给的是「下一步」而不是「出错了」**：「**修好再打开开关**」⇒ ⇒ P1 家族
2. ⭐ **三个码被逐个点名**（`command_failed` / `unsupported_command` / `command_unavailable`）
   ⇒ ⇒ **B0213 已核 `command_unavailable` 真的会在停用时出现** ⇒ ⇒ **码是服务端真会回的那些**
3. ⭐⭐ **第三条带一条作用域声明**：「**会话绑定的卡不进主链，因此不受这条影响**」
   ⇒ ⇒ 而 **B0221 已核 persona Mod 侧**的 `render_injected_text` / `render_from_config`
   **只写 `[persona] system_prompt`** ⇒ ⇒⇒ **「不进主链」就是那个事实的 UI 侧说法**
   ⇒ ⇒⇒ **两侧对同一件事各说一句、且一致** ⇒ ⇒ **B0246 核的「同一句话复用」在此升级为
   「同一件事在两侧各说一句」**
4. ⭐ **「三条提示都走文字」** ⇒ ⇒ **没有一条靠图标或颜色单独表达**
   ⇒ ⇒ 与 B0245 的「**状态用文字、不靠颜色**」**同一条纪律的第三次应用**
   （连接状态 · Mod 启停 · **persona 提示**）⇒ ⇒ **而 B0237 已核 `personaStatusIsFailed`
   认两个值（`failed`/`error`），此处 `:42` 用的正是它**

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~595 / `memory_panel` ~545 /
   `persona_panel` ~515 / `message_bubble` ~460 / `chat_panel` ~465）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
