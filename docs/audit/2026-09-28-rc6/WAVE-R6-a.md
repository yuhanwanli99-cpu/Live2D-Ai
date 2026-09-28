# 审查记录 · 波次 R6-a（Stage B 模型与渲染）· 2026-09-28

> **审查者**：编排者（Lead）。**实施者**：teammate `r6-a-model`（fresh）。
> **纪律**：审查**不得由实施者自评**。本文件里的每个数字都是**编排者自己重跑**得到的原始输出；
> 实施者的自述只作为「待核验清单」，**不作为证据**。
> 上游账本：`docs/audit/2026-09-28-frontend-nightly/`（**只读**）。本目录是**实施/审查记录**，不回写账本。

## 1. 波次范围（`ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28` §4 的 R6-a）

Stage B 的模型/渲染面：`imageFit` 四档（cover/contain/stretch/tile）+ `tileSize`、
逐图样式覆盖（opacity/fit/align）、全局 `background.enabled`（DEC-4）、
DEC-1 `slideInterval` 端点夹持、DEC-5 坏 dataURL 收紧。

## 2. 实施者交付（自述，仅登记）

| 文件 | 行数 | 变化 |
|---|---|---|
| `shell/flutter/lib/settings/display_prefs.dart` | 1006 → **1157** | +151 |
| `shell/flutter/lib/ui/shell_backdrop.dart` | 224 → **463** | +239 |
| `shell/flutter/lib/design/background_item.dart` | 274 → **495** | +221 |
| `shell/flutter/lib/app/app_shell.dart` | 892 → **899** | +7（**编排者批准的 2 行传参** `:771 enabled:`、`:775 tileSize:` + 注释） |
| `shell/flutter/test/display_prefs_background_fit_test.dart` | 新增 **645** | 42 条新回归 |
| `shell/flutter/test/display_prefs_slide_index_test.dart` | 97 → 82 | DEC-1 语义更新 |
| `shell/flutter/test/display_prefs_test.dart` | 882 → **885** | DEC-1/DEC-5 预期后果 |
| `shell/flutter/test/setting_wiring_test.dart` | 368 → 378 | 上界 1→3 + 1 处假 dataURL |
| `shell/flutter/test/app_shell_background_test.dart` | 159 → 161 | 2 处假 dataURL（DEC-5 后果） |

## 3. 编排者独立复核（原始输出，不采信自述）

```
$ cd /home/skystar/Live2D-Ai-fe/shell/flutter && flutter analyze
Analyzing flutter...
No issues found! (ran in 2.1s)                      ← issue 数 = 0 ✅

$ flutter test
00:25 +1314: All tests passed!                       ← 1314 passed / 0 failed，exit 0 ✅
                                                       （rc.5 基线 1272 → +42；zz_ 计数 0）

$ cd /home/skystar/Live2D-Ai-fe && git diff --stat
 shell/flutter/lib/design/background_item.dart      | 243 ++++++++++++++++-
 shell/flutter/lib/settings/display_prefs.dart      | 189 +++++++++++++--
 shell/flutter/lib/ui/shell_backdrop.dart           | 297 +++++++++++++++++++--
 shell/flutter/lib/app/app_shell.dart               |   7 +
 shell/flutter/test/app_shell_background_test.dart  |   6 +-
 shell/flutter/test/display_prefs_slide_index_test.dart | 72 ++---
 shell/flutter/test/display_prefs_test.dart         |   9 +-
 shell/flutter/test/setting_wiring_test.dart        |  24 +-
```

**关键实现点的 grep 复核**（编排者执行）：
```
$ grep -n "backgroundEnabled\|tileSize" shell/flutter/lib/app/app_shell.dart
771:            enabled: widget.prefs.backgroundEnabled,
775:            tileSize: widget.prefs.tileSize,
  ⇒ 批准的那 2 行**真的接上了**（不是只加了参数没传）✅

$ grep -n "maxImageFit" shell/flutter/lib/settings/display_prefs.dart
380:  static const int maxImageFit = BackgroundStyleRange.maxFit;      ← 不再是写死的 1 ✅
598:        maxImageFit,                                                  ← fromJson 上界随之放开 ✅

$ grep -n "_clampIntToRange" shell/flutter/lib/settings/display_prefs.dart
832:  /// **本轮不动它**（DEC-1 明确）：要端点夹持的字段用 [_clampIntToRange]。
842:  static int _clampIntToRange(int value, int min, int max) {
867:    return _clampIntToRange(
  ⇒ DEC-1 用**新函数**实现，_clampInt 未被改（scrim 语义保住）✅

$ grep -n "isRenderable" shell/flutter/lib/design/background_item.dart
244:  bool get isRenderable => state == BackgroundImageState.ready;    ← DEC-5 收紧到状态判据 ✅
```

## 4. 编排者在实施过程中做的三次**裁决**（原文见会话；此处只记结论）

1. **批准实施者补 app_shell.dart:765 的 2 行传参**（DEC-4 / tileSize 的接线点）。
   理由：R6-a2 当时未开工 ⇒ 无并发写；且不批准则「开关只有模型没有渲染路径」= 半成品。
   约束：只改传参那 2 行，不碰 app_shell 其他部分；这两行必须与 R6-b 后续改动**正交**。
2. **`data/background_hydration.dart:113` 不得由 R6-a 改**（越界），改为**R6-a2 的硬要求**：
   `BackgroundImage(id:…, dataUrl:…) → item.copyWith(dataUrl: read.dataUrl)`，
   否则每次水合会**静默清空**用户刚设的逐图 opacity/fit/align。
3. **DEC-5 的预期副作用批准**：3 个既有测试文件里用假 dataURL（`dataUrl:'x'`）当「画得出来」的地方
   会被收紧后变红 —— 批准实施者**只做最小改动**（把串改成合法形态），**不许顺手改断言语义**；
   并**额外要求**一条负向断言（非法形态 ⇒ `isRenderable==false` 且 `decodeDataUrlBytes==null`）。

## 5. 未决 / 风险（继承给 R6-d 与 Stage C）

| # | 项 | 归口 |
|---|---|---|
| 1 | `tile` **只有算术与结构级验证，没有像素级回归**——widget 测试的 fake async 推不动 fake zone 里发起的 ImageStream。实施者**没有伪造绿灯**（如实标注），四档**像素级只能靠 Win 肉眼**：`R6-d` 验收 | **R6-d 肉眼** |
| 2 | `display_prefs.dart` **1157 行**：超 `≤500`、也超出 `≤1000` 豁免带。**既有债**（本波次前 1006），头注已如实写明 | **Stage C3** |
| 3 | `display_prefs_test.dart` **885 行** > 800 上限（本波次只 +3）；新测试已另建新文件（645 行），未继续塞进它 | **Stage C3** |
| 4 | DEC-5 的「坏图」约定 = **字段式**（`BackgroundImage.state: ready/pending/corrupt` + `isCorrupt/bytesPending`），无回调。R6-b 直接读字段 | 已交接 R6-b |
| 5 | 本波次**未**做外观区 UI（铺法四档选项 / tileSize 滑杆 / 逐图样式编辑器）、DEC-2/DEC-6/DEC-7、D1 —— 按文件归属归 R6-b | **R6-b** |

## 6. 结论

R6-a **通过**（analyze 0 / test 1314 全绿 / 接线与 clamp grep 复核一致）。
**待补**：`tile` 像素级肉眼（R6-d）；超长文件（Stage C3）。
**未做红-绿独立复核的原因**：实施者报告了「旧档 2/3 迁移改前红 → 改后绿」，
编排者的**独立**红-绿复核排在 `R6-a2/R6-b` 全部收口、工作树静止之后统一做
（并发改源码会把别人的 `flutter test` 弄红 —— 见 §7 计划）。
