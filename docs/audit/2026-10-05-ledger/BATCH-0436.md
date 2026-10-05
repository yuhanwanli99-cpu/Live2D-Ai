# BATCH-0436 · ✅ **+1 条 P3：清单里多了一个不存在的名字**

Phase 4 · **证伪**（按 B0435 说的失败模式「清单会过期」**机械检验**那四条是否还在）

## 跑的命令（全部只读）
```
for s in StageHost shortcutHelp LiveRegionThrottle EpochGate; do
  printf "%-20s 声明:%s 使用:%s" "$s" "$(grep -rl "class $s|$s(" lib/ --include=*.dart | wc -l)" \
    "$(grep -rn "$s" lib/ --include=*.dart | grep -v "class $s" | wc -l)"
done
sed -n '77,88p' test/wiring_test.dart
grep -rn "EpochGate" --include=*.dart .
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0436-01 (P3)**
```
StageHost            声明:2  使用:9      ✔
shortcutHelp         声明:2  使用:4      ✔
LiveRegionThrottle   声明:2  使用:4      ✔
EpochGate            声明:0  使用:0      ⚠ 零命中
```
```dart
test('**`EpochGate` 的判据被 `ChatController` 用上**（钉子 13 真的生效）', () {   // :77
  expect(referencedOutside('**mustInterruptAudio(**', 'lib/audio/epoch_gate.dart'), isTrue);
  expect(referencedOutside('**decideAudioFrame(**',   'lib/audio/epoch_gate.dart'), isTrue);
});
```
五个可核点：
1. ⭐⭐⭐⭐ **而 `EpochGate` 全仓只出现在这一行测试名里** ⇒⇒ **它不是一��存在的类型名**
   ⇒⇒ 而 `lib/audio/epoch_gate.dart` **存在**（被两条断言按路径引用）⇒⇒ **文件名是对的**
2. ⭐⭐⭐⭐⭐ **⇒⇒ 测试名承诺的**强于**断言给出的**：
   - 名字：「**被 `ChatController` 用上**」⇒⇒ 读者会以为断言里出现了 `ChatController`
   - 断言：「`mustInterruptAudio(` / `decideAudioFrame(` **被 `epoch_gate.dart` 文件外引用**」
     ⇒⇒ **任何一个别的文件引用它们，两条断言都通过**
3. ⭐⭐⭐ **而这条测试**不是废的**：它守的是「**判据不被内联回调用方**」⇒⇒ **断言本身有意义**
   ⇒⇒ **问题是名字与断言的落差** ⇒⇒ **P3、非 P1/P2**
4. ⭐⭐⭐⭐ **⇒ 而这与本仓自己那条原则构成**反面用法**（B0379 核的「测试名 = 被守的纪律」）：
   > **① 组名承诺「每个都写清后果」② 成员的测试名承诺「被 `ChatController` 用上」**
   > **③ 而断言只保证「被文件外引用」** ⇒⇒⇒ **三层承诺里，第三层最弱**
5. ⭐⭐⭐⭐ **⇒⇒ 而这正是 B0435 说的「清单会过期」在**同一条清单**上的第一次实证**：
   **一个承诺「每个都写清后果」的清单，它的第一个条目用了一个仓库里没有的名字**
   ⇒⇒⇒⭐ **⇒⇒ 这不是「漏一项」，是「多了一个不存在的项」**

⇒ ⇒ **修法各一行**：**改名**成它真正断言的那句 · **或**补第三条断言把 `ChatController` 点名；并把 `EpochGate` 换成文件/函数名。
⇒ ⇒ ⚠ **反证已写**：若 `referencedOutside` 的实现比名字暗示的更强（例如带「只认某个调用方」的参数），
则**建议的另一半变成「删掉那个多余的参数」** ⇒⇒ **但本条不受影响的那部分（`EpochGate` 是不存在的名字）仍然成立**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. ⭐ **`referencedOutside` 的实现**（F-0436-01 反证留：**它带不带「只认某个调用方」的参数**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~455）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
