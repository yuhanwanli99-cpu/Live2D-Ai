# BATCH-0638b 落盘（极简）· 三处调用 = 三种锚点形状：字面量区间 / 常量名 / 常量插值

## 编号
`BATCH-0638.md` 已存在 ⇒ 本批落 `BATCH-0638b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0638.md
sed -n '76,86p' shell/flutter/test/setting_wiring_test.dart
sed -n '147,158p' 同上
```

## `:77-84` 逐字（第二处）
```
:77    final String ui = appearanceSectionSource();
:78    expect(
:79      ui.contains('effectiveFit == DisplayPrefs.fitTile'),
:80      isTrue,
:81      reason: '贴片滑杆必须被 tile 档 gate 住（否则 cover 下它是个…',
:82    );
:83    expect(ui.contains('DisplayPrefs.minTileSize'), isTrue);
:84    expect(ui.contains('DisplayPrefs.maxTileSize'), isTrue);
```

## `:147-158` 逐字（第三处）
```
:147  final String ui = appearanceSectionSource();
:148  for (final int level in <int>[
:149    ScrimLevel.auto, ScrimLevel.none, ScrimLevel.light, ScrimLevel.heavy,
:152  ]) {
:153    expect(
:154      ui.contains('FieldOption<int>(value: $level,'),
:155      isTrue,
```

## 四个可核点
1. **⭐⭐⭐⭐⭐ ⇒⇒⇒⇒ 三处调用把 B0637b 第 ② 点立的三种锚点形状**全部用上了**：
   - **字面量区间**（`:53-57`）：`indexOf("label: '铺法（图）'")` + `indexOf('onChanged:')`
     ⇒⇒ **UI 文案当契约**
   - **常量名**（`:79 / :83 / :84`）：`contains('effectiveFit == DisplayPrefs.fitTile')`、
     `contains('DisplayPrefs.minTileSize')`、… ⇒⇒ **符号名当契约**
   - **常量插值**（`:154`）：`contains('FieldOption<int>(value: $level,')` + `for` 循环
     ⇒⇒ **结构模板当契约，且一次覆盖 4 个值**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（本批最值钱，可复用）：
   **选哪种锚点 = 选「哪种改动会让这条变红」** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 三者的灵敏度不同**：
   - 字面量最灵（改文案就红）、最脆
   - 常量名较稳（改名就红）、不碰文案
   - 常量插值**一次覆盖 N 个值**（`:148-152` 的 4 档）⇒ **不漏档**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（第二句）：
   **「档位类」断言优先用循环 + 插值**，因为它把「漏一档」这件事变成不可能
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒ 与 B0624 核的
   `registry` 遍历 + `length` 计数**（P40）**是同一取向** ——
   ⇒⇒ ⇒⇒ ⇒ 「用循环而不是逐条点名」** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒ 承 B0637b 第 ① 点：
   这三处**都不需要 `substring` 的前置 `expect`**，因为它们只问「这段文本里有没有 X」，
   ⇒⇒ ⇒⇒ ⇒ **不存在「截取区间不存在」这个失败模式** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ 判据（第三句）：
   **需要前置 `expect` 的只有「截取」型；纯 `contains` 型不需要** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 那就是 B0637b 第 ① 点
   的适用边界**
2. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ `:83-84` 两条 `expect` **没有 `reason`**，而同一 test 里 `:81` 有**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（本批第二个可提炼）：
   **`reason` 的覆盖不该随机** —— 一个 test 里的断言**要么都写、要么都不写**，
   混着写会让读者以为「没写 reason 的那两条不重要」
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒ 后果**：
   `:83` 若失败（`minTileSize` 不见了），失败信息只有「Expected: true / Actual: false」，
   而 `:81` 的失败会说明「贴片滑杆必须在 tile 档 gate 住」⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒ 严重度候选 P3**（本批不立）——
   ⇒⇒ ⇒⇒ ⇒ 但这是**可量化的**：`grep -c "expect(" 与 grep -c "reason:"` 的比值能给出全仓的
   `reason` 覆盖率 ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒ 下一批可跑这个数。**
3. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `:154` 的锚点末尾带一个逗号**（`value: $level,`）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（细节，可复用）：
   **锚点末尾的标点提高唯一性** ——
   `value: 0` 会同时匹配 `value: 0,` 与 `value: 01`；而 `value: 0,` 只匹配前者
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ 承 B0631b 核的
   `type` 带 `:`、`value == DisplayPrefs.fitTile` 那种带结构上下文** ——
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 「锚点要长到只匹配一处」**
   ⇒⇒ ⇒⇒ ⇒ 与 B0637b 第 ② 点同一原则的**加强版**。
4. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `:148-152` 的循环体里有 4 个 `ScrimLevel.*` 字面量** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（承 B0623 核的「`registry` 让新增自动进测试」）：
   **用 `ScrimLevel.*` 常量而不是 `0/1/2/3` 数字，循环才有意义** ——
   若写成 `<int>[0,1,2,3]`，新增第 5 档时循环不会跟着长
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒ 判据：
   **「遍历型断言」的前提是「被遍历的集合是活的」** ——
   ⇒⇒ ⇒⇒ ⇒ 活集合有两种：常量类（B0623 的 `registry`）与枚举（本处）⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒ 二者都优于字面量数组。**

## 未核
`display_prefs_test.dart` 那份 `appearanceSectionSource` 有没有被调用 ·
全仓 `expect(` 与 `reason:` 的比值 · `_p4NewFieldsTests()` 是什么
