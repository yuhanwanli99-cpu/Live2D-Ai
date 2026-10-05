# BATCH-0178 · ⭐ **第七次换根 → 前端 Dart**；三个大测试文件：**本审计见过最强的测试文化**

Phase 1 · 域覆盖 · `shell/flutter/test/`（按 §8i：**先看测试的对照面**）

## 跑的命令（全部只读）
```
python3 - <<'PY'   # 机械列未审 .dart（带自检）
PY
for f in test/display_prefs_test.dart test/action_scales_wiring_test.dart test/memory_panel_test.dart; do
  grep -oE "test\('[^']+'" $f; done
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ① 覆盖率（机械 + 自检）：**218 个 .dart，已审 31，未审 187**
⚠ **自检发现与我账本的 35 不一致**（差 4）：原因是两次统计的**路径归一方式不同**
（INDEX 里既有 `lib/x.dart` 也有 `flutter/lib/x.dart` 形式，去重后基数不同）
⇒ **如实记录：权威值以本次机械口径为准（31/218）**，**不选一个更好看的数**。
（这是 B0147 那条「脚本输出与既有认知冲突时先怀疑脚本」**第二次生效**——只是这次是口径差异，不是逻辑错误。）

## ② 本批产出：**0 条新发现**；⭐ **Dart 测试文化强于 Rust**，且强在三点
### ① `display_prefs_test.dart`(892) —— **测试名写出「被否掉的另一种做法」**
```
**非有限数回落默认（不是夹到边界）**     ← ⭐ 名字**点名它拒绝的方案**
**越界值夹到区间内**                     ← **反向对照**（相邻 case、不同策略、两个都命名）
**null / 空 map 回落默认** · **类型不符回落默认**
**旧版本存档（没有 volume/muted 字段）按新默认读**
**整数形式的 JSON 数值也能读（localStorage 常见）**   ← ⭐ 真实世界 case
**toJson → fromJson 幂等**               ← round-trip
```
⇒ NaN/Infinity ⇒ 回落默认、有限但越界 ⇒ 夹到区间：**两个相邻 case、两种策略、都被命名**
⇒ 这比 P7（配反向对照）**更进一步**：P7 只要求「有对照」，这里**把对照的理由写进了名字**。

### ② `action_scales_wiring_test.dart`(737) —— **跨层契约 + 精确 JSON + 源码扫描**
```
**③ 重启后仍在：PATCH 形状 == 服务端 D40 契约**；GET 往返能算有效值
**④ 恢复跟随全局：body == {"action":{"models":{"bai":null}}}**    ← **精确相等**，不是 contains
**⑤ 合并防抖：连续 onChanged 只落 1 次 PATCH，且只发被改过的键**
**值没变不重复下发；拖动只发一帧**（临时覆盖期间同样成立）      ← 负向断言
接线：宿主把 syncer 的**显式 pin 只读**传进面板（**源码扫描**）   ← **结构性**测试，且名字说明
```
⇒ ③ ④ 把**跨层契约从前端侧钉住**（PATCH 的 body 形状 == 服务端契约）
⇒ ⑤ 正好覆盖我在 B0069 **撤回** F-0030-01 时用的那套推理（150ms 防抖 + 只发被改的键）
⇒ ⭐ 而最后那条是**源码扫描型**测试 —— 不是行为测试，**且名字里注明了**（「源码扫描」）

### ③ `memory_panel_test.dart`(852) —— **测试名写出「诚实性原则」**
```
memoryCountText：数字照抄，**缺失/非数字 → —（不假装是 0）**      ← ⭐ 诚实性写进名字
memoryCommandErrorMessage：**每个码都带码且可处置**                ← 从前端侧钉住 B0161 的 hint 纪律
memoryParseRecords：**未知形状 → 空列表，不崩**
memorySummaryStatusLine：未启用 / 无摘要 / 有摘要 / **失败原因四种口径**
```
⇒ 「**不假装是 0**」——**这是正面模式 P1（理由要写「用户会看到什么」）用在了测试名上**
⇒ 而「每个码都带码且可处置」把 B0161 核的 hint 纪律**从前端这一端也钉住了**

## ③ ⭐ 由此**再精化一次 §8i**
我此前记「同一层里生产与测试质量是两件事」（`web_api`：生产 7 条 P1 / 测试最规范）。
现在看到**Dart 侧测试文化比 Rust 侧更强**（上面三点 Rust 都没有）⇒
而团队自己的 45 条审计里有**两处假绿** ⇒ **假绿是局部例外，不是文化缺陷**。
⇒ **跨语言共同的形态**：**例外是局部的，周边文化往往很强。**
⇒ **方法论推论**：**按「典型文件」抽样会系统性高报**；
> **要找到例外，必须刻意去找** —— 去找**名字里带「不是…」「不假装…」的那种断言**，
> 因为**写下了这种名字的人，正是知道自己想防什么的人**。
⇒ 而这类断言**恰好也是最容易被实现悄悄破坏的**（因为破坏它不会让任何旧测试变红）。

## 未核实项
1. 上述三个测试文件的**断言体**未读（本批只读测试名）
2. 未审 `.dart` 仍 187 个；最大者：`dev_tools_section.dart`(1874) · `tokens.dart`(928) ·
   `live2d_stage.dart`(666) · `message_bubble.dart`(577) · `memory_panel.dart`(694) ·
   `persona_panel.dart`(608) · `director_observer_section.dart`(668) · `chat_panel.dart`(569)
3. Mod crates 逐文件（约 53/61 未读）· `shared/` · 根 `tests/`(3 py) · `verification/`
4. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
5. `runtime/src/lib.rs` 的 `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体未读
6. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
