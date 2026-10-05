# BATCH-0419 · ⭐⭐⭐⭐ **「清空」与「作废在途」是两个失效** —— 而只有后者需要**世代号**

Phase 4 · **证伪**（读这个测试的**断言体** · B0418 留的 166 行）

## 跑的命令（全部只读）
```
grep -nE "test\(|expect\(|_reset|group\(" test/transient_results_test.dart | head -14
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**测试体比头注承诺的更强**（0 条新发现）
```
:80   group('P2-2：切分区**只有一条路**')
:81     test('`_section =` 在 main.dart 里**只出现在 `_gotoSection` 一处**')
:96     expect(writers.single, contains('_section = next'));          // 只有一个写者、且是它
:99     test('`_gotoSection` **真的清了结果（不是只改分区）**')
:117  group('P2-2：四个一次性结果**全在**清理范围内')
:118    test('清的就是这四个（**不是只清了一两个**）')
:141      expect(body.contains('$flag = false'), isTrue, reason: '$flag 没有复位');   // 逐字段 + 逐条 reason
:145    test('**在途的**异步结果也会作废（`_resultEpoch` 对账）')
```
四个可核点：
1. ⭐⭐⭐⭐ **「只有一条路」被机械核过** ⇒⇒ **全文件搜 `_section =`、断言**只有一个写者**、
   且**那个写者**是 `_gotoSection`
   ⇒⇒⇒⭐ **⇒ 与 B0416 核的「零个 `setState`」是同一种手法：让「唯一」**可被 grep 验证****
2. ⭐⭐⭐ **「不是只改分区」被写进**测试名** ⇒⇒ **测试名 = 被守的纪律**（B0379 核的 `stage-ack` group 名同族）
3. ⭐⭐⭐ **逐字段核、每个字段带一条 `reason`**（`'$flag 没有复位'`）⇒⇒ **断言失败时直接告诉你是哪个字段**
   ⇒⇒ **同 B0369 核的「去重只去同值，真变化不许去」那条 `reason`**
4. ⭐⭐⭐⭐⭐ **而第四个场景我没预期：「在途的异步结果也会作废（`_resultEpoch` 对账）」**
   ⇒⇒ **⇒ 不只是「切走时清空」，还要让**飞行中的响应回来时作废**
   ⇒⇒⇒⭐⭐ **⇒ 机制是 `_resultEpoch`（世代号）⇒⇒⇒ **⇒ 与 B0310 核的 `epoch` 是同一条机制**，
   而「**跨代作废**」是它的新用途
   ⇒⇒⇒⭐ **⇒ B0418 头注里那句「响应几秒后才回来」在此有了断言**

⇒ ⇒⭐⭐⭐⭐ **而本批最值钱的是第 4 点**
> **「清空已有状态」与「作废在途结果」是两个不同的失效** ——
> **前者是显示问题、后者是「写入时机」问题** ⇒⇒ **⇒ 而只有后者需要世代号**
> ⇒⇒⇒⭐ **⇒ 于是这条测试的存在，证明了一个设计选择**：
> **清空不足以解决竞态，必须给每个异步结果一个世代号**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `_resultEpoch` 的**本体**（它在哪些地方自增、哪些地方对账 · **未读**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
