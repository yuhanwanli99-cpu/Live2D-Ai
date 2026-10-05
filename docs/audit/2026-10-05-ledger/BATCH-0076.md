# BATCH-0076 · 第 4 处清单（`toJson`/`fromJson`）机械对账 —— **0 条新发现**

Phase 1 · 域覆盖 · 前端

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/display_prefs.dart` — 1157（`fromJson` 体 :570-640 定点）

## 跑过的命令（全部只读）
```
python3 - <<'PY'   # 四路机械集合差：ctor / == / toJson / fromJson
                   # 并对每路做非空断言（assert fields and eqf and tj and fr）
PY
grep -n "DisplayPrefs.fromJson(" display_prefs.dart
sed -n '570,640p' display_prefs.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。**未执行任何 Dart 代码**（纯文本集合运算）。

## 本批产出：**0 条新发现**；`DisplayPrefs` **四处清单全部一致（24 字段）**
`==`(22 标量 + `backgrounds` + `stagePlaylist`) · `toJson`(24) · `fromJson`(24) · `copyWith`(25 含合成标志)
⇒ **风险陈述（B0074/B0075 已记）**：清单今天全对，**但没有任何测试会在它们开始漂的那天告诉你**。

## ⚠ 第 4 次「没看到 ≠ 没有」（本轮当场抓住）
机械差集先报「`fromJson` 少读 `backgroundSource` / `backgrounds`」。按规则 3 **先查解析**：
```dart
// display_prefs.dart:578-579（fromJson 体内）
backgrounds: readBackgrounds(json),
backgroundSource: _readBackgroundSource(json),
```
⇒ 两者**是通过辅助函数读的**，我的正则只匹配内联 `json['key']` ⇒ **解析漏了，不是代码漏了**。
⇒ 若直接收下那条差集，就会记下一条**假发现**（且它看起来相当可信 ——
「背景图在重载后丢失」是 P1 级的叙述）。

## 顺带核到（正面）：`fromJson` **读时钳位**
`:580-587`：`clampBackgroundOpacity(_readDouble(json['backgroundOpacity'], …))` ·
`clampBackgroundBlur(…)` · `_clampInt(json['backgroundScrim'], …)`
⇒ **落盘值在读取时被消毒**，损坏/越界的持久化数据不会带进运行态。
（这与 F-0067-01「Mod config 的数字宿主侧不钳」是**不同面**：一个是**写入侧**无钳，
一个是**读取侧**有钳 —— 两者不矛盾，但值得在修 F-0067-01 时确认期望落点。）

## 未核实项
1. `background_hydration.dart:140-255` 未读
2. `settings_controller.dart`(409) / `app_shell.dart` 余段未读
3. `appearance_background.dart`(1551) 未读（只核过 :1264）
4. 其余 Mod 面板（4 个 / 2284 行）未读
5. `background_logic_test.dart` / `background_copy_test.dart` 未读
6. `readBackgrounds` / `_readBackgroundSource` 两个辅助函数**本体**未读
   （只确认它们被调用；其内部是否也做了钳位/丢弃，未核）

## 本批新增
**0 条**
