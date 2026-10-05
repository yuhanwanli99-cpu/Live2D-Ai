# BATCH-0190 · ⭐ **F-0189-01 的升级条件被排除**（维持 P2），但**危害被改写成更重的一条**

Phase 1 · 域覆盖 · `settings_controller.dart`（`save()` 主体 + `_draft` 的全部重置点）

## 跑的命令（全部只读）
```
sed -n '279,300p' lib/settings/settings_controller.dart
grep -n "_draft = " lib/settings/settings_controller.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**结清 F-0189-01 的未决点，并改写其危害陈述**
### ① 升 P1 的条件**被排除**（quote）
```dart
Future<SaveOutcome> save() async {                       // :280
  /// 保存。失败时**保留草稿**。                            // :280 头注
  final SettingsPatch patch = _draft.toPatch();          // :281  ← **由 `_draft` 派生**
```
⇒ **patch 来自 `_draft`，不是 `_remote`** ⇒ **过期的 `_remote` 永远到不了线上**
⇒ **不存在「旧响应 → 保存 → 写回旧值」** ⇒ **本条维持 P2**（不再有升级悬念）

### ② ⚠ 而真正的危害比原措辞**更重**（这是我原记录**不够准**的地方）
```
$ grep -n "_draft = " settings_controller.dart
231:  SettingsDraft _draft = SettingsDraft();     ← 初始
267:      _draft = SettingsDraft();             ← ⭐ **load() 里，无条件**
284:      _draft = SettingsDraft();             ← save() 的 noChange 早退
296:      _draft = SettingsDraft();             ← save() 成功后
327:  _draft = SettingsDraft();                 ← 另一处置
```
⇒ **`load()` :267 的 `_draft = SettingsDraft();` 是无条件执行的**
⇒ ⇒ **一个迟到的响应会直接丢掉用户未保存的草稿** —— 而用户**只按了一次按钮**，只是第二次慢
⇒ 而 `save()` 头注写着「失败时**保留草稿**」⇒ **本项目自己在意不丢草稿**
⇒ ⇒ **被一个迟到响应丢掉，与他们自己的口径冲突** ⇒ 这比「界面显示旧值」**更接近数据丢失**
⇒ **所以危害陈述改为**：① **迟到响应丢弃未保存草稿**（主）② 面板显示较旧的值（次）

### ③ 修法范围随之**明确**（比原建议更准）
守卫必须**同时**保护 **`_remote`** 与 **`_draft`**
⇒ **只挡 `_remote` 的一半，挡不住真正的危害**
⇒ 建议仍是那条自增序号 + `await` 后早退，但**早退点要在两次赋值之前**

### ④ 顺带把「设计行为」与「副作用」分开（B0188 那条测试只覆盖前者）
B0188 的第二条测试把「**load 会丢弃未保存草稿**」钉为**设计行为** ⇒ 那是**用户主动**的顺序 load
⇒ 对**迟到**响应而言，这个丢弃是**副作用**、不是用户意图
⇒ ⇒ **两者必须分开**，否则「设计如此」会变成「 bug 是特性」的挡箭牌。

## 未核实项
1. `:327` 那处置 `_draft` 的上下文（另一处置，未读）
2. `ApiClient.fetchSettings()` 本体未读（是否内部串行化 ⇒ 反证 (b)）
3. `settings_controller.dart:120-222` / `:296-409` 其余未读
4. 新露出的 208 条自我设防名只看了 11 条；`dev_tools_section.dart`(1874) / `tokens.dart`(928) 未读
5. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
