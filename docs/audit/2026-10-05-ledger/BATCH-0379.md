# BATCH-0379 · ✅ **回归在** —— 而 group 名**就是那条规矩**，第二条钉住**拒绝**

Phase 4 · **证伪** —— 结清 B0378 留的「`stage-ack` 读数的回归测试是否存在」

## 跑的命令（全部只读）
```
grep -rln "stage-ack|stageAck|lastAck" shell/flutter/test/ | head -5
grep -rn "lastAck|stage-ack" shell/flutter/test/ | head -6
sed -n '119,152p' test/live2d_bridge_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0000 ⑧ 的修法有回归**（0 条新发现）
```dart
group('**stage-ack 解析（收条前不得提示已生效）**', () {                     // :119
  test('解析 applied / msg_type / scale / offset', () async {
    // **真实形状：没有 version，字段在根上。**                             // :127  ⭐
    transport.emit('{"type":"stage-ack","applied":true,"msg_type":"stage-zoom",'
      '"scale":1.21,"offset_x":0.0,"offset_y":0.0}');
    expect(bridge.lastAck, isNotNull);
  });
  test('**applied=false 也要能读出来（渲染面拒绝了）**', …                  // :141  ⭐
    expect(bridge.lastAck!.applied, isFalse););
  test('缺字段/类型不对 → **不抛，缺的为 null**', … );                      // :150
});
// 另 :51  '**容忍无 version 的遗留 stage-ack 与未知 type**'
```
四个可核点：
1. ⭐⭐⭐ **group 名本身就是那条规矩**：「**stage-ack 解析（收条前不得提示已生效）**」
   ⇒ ⇒ **测试名 = 被守的纪律** ⇒⇒ **同 B0313 那条 B0309 `stop()` 排序的注释同款**：
   **纪律写在「防它不守」的那一处** ⇒⇒⇒ **而这让「修法有没有回归」一眼可判**
2. ⭐⭐ **而测试用的是「真实形状」**（`:127` 逐字「**真实形状：没有 version，字段在根上**」）
   ⇒ ⇒ **不是造一个比真实情况更规整的形状** ⇒⇒ **B0274 规则的又一次遵守**（别用一个更漂亮的输入）
3. ⭐⭐ **第二条钉住的是「拒绝」**（`applied=false` 也要能读出来）⇒⇒ **不是只测成功**
   ⇒ ⇒⇒ **同 B0334「每个相位都可达」、B0335「穷举表」** —— **反向分支也是分支**
4. ⭐ **第三条钉住「不抛」**（缺字段 / 类型不对 ⇒ 缺的为 `null`）⇒⇒ **同 B0322「坏 dataURL 永不抛」**族
5. ⭐ 而 `:51` 另有「**容忍无 version 的遗留 stage-ack 与未知 type**」⇒⇒ **向后兼容**也是被测的
   ⇒⇒ **与 rc.3 的「协议新增字段必须向后兼容（缺省即默认值）」呼应**

⇒ ⇒ **0 findings**；⇒ ⭐ **而 B0378 的判断被完整兑现**：
**「不自己算」⇒「真值只有一个来源」⇒「有读回执的回归」⇒「回归名就是纪律」** ⇒⇒ **四层齐全**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~570 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
