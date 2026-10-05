# BATCH-0263 · ⭐ **F-0263-01（P3）**：重建后旧快照**会一直留着**；且模式 Q **第四次抓到我**

Phase 1 · 域覆盖 · `live2d_stage.dart` 的 `presetStatus` 写者（B0251 留的未决点）

## 跑的命令（全部只读）
```
grep -nE "PresetStatusAction.clear|clear\(\)|_presetStatus|onReady:" live2d_stage.dart
sed -n '60,70p' live2d_stage.dart ; sed -n '365,382p' live2d_stage.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0263-01（P3）**；**模式 Q 第四次拦截了我自己的违规**
### ① 「唯一写者」核到了
```dart
final PresetStatusUpdate update = presetStatusUpdateFor(event, DateTime.now());   // :366-369
switch (update.action) {
  case set:    presetStatus.value = update.status;
  case clear:  presetStatus.value = null;
  case ignore: break;
}
void _handleHostError(String message) { … setState(() => _hostError = message); }  // :380-382 **不碰 presetStatus**
```
⇒ ⇒ **重建后，新渲染面没有正在播的动画** ⇒ ⇒ **它不会发 `expired/dropped`**
⇒ ⇒ **旧快照不是「留一会儿」，而是可能一直留着**（B0251 的未决点**结清**）
⇒ ⇒ **不升 P2**：B0232 已核它是「**显示快照**」⇒ **无行为依赖** ⇒ 危害限于「诊断区显示已不生效的状态」
⇒ ⇒ **修法一行**（重建/重试路径显式清空）—— 而它便宜**正因为 B0251 已核 `clear` 已是一等动作**

### ② ⭐ 而**更一般的教训**才是本条的价值
> **「清理由 ack 驱动」的设计，在「重建后**没有东西可清**」的场景下会失灵。**
⇒ 纯函数 + 单一写者（P27 同族）是好的，**但它有边界**；**机制有边界时，边界必须被写下来**
⇒ 否则下一个人会以为「ack 驱动 = 永不失真」⇒ 建议在该字段文档补一句「**仅由 ack 更新；重建后需手动清**」

### ③ ⭐ 而 `:65` 那个同名 `..clear()` 我**核了它无关**
`:65` 的 `_bySeq ..clear()` 是 **`ActionCue` 的 map**（`:62` `replace()` 的实现），**与 `presetStatus` 无关**
⇒ **两处都核过，不混为一谈** ⇒ 写进反证，避免后人误以为有第二个 `clear` 写者

### ④ ⚠ 模式 Q **第四次抓到我的同一形态违规**
`F-0255-01` 被 `INDEX.md` 引用**却没有 `### F-` 规范头** ⇒ 收尾检查当场抓出
⇒ **四次复发都在同一形态**（B0184 / B0189 / B0255 / 本批）：**新发现急着写正文、忘了先写头**
⇒ ⇒ 执行提示已立：**先头后正文**；而**这道检查第四次发挥了作用**

## 未核实项
1. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~560 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
2. `onReady` 的实现（是否间接清过）—— **未核，但我的结论不依赖它**（即便会清，也只是「有时会清」）
3. `ErrorResponse` 定义未读；`MutatingCheckError::as_message()` 未读
4. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
5. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
