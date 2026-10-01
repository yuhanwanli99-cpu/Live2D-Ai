# GROUNDING · W-VERIFY-1b（独立复核 W1-c / W1-d / W1-d2 / W1-e）

> 复核者：`verifier`（非实施者，**只读仓库**；唯一写入处 = `docs/audit/2026-10-01-debloat/**`）
> 复核对象（W1 剩余已提交波次）：`cbbedef0`(W1-c) · `1d0c6e66`(W1-d) · `7eed27f2`(CI) · `77da64e5`(W1-d2) · `d140604f`(W1-e)
> **工作树纪律**：`dart-fixer-a` 正在改 `main.dart`/`field_row.dart`/两个 section（W1-b）⇒ 本复核**不在工作树跑任何 flutter**。
> 全部命令在 `git archive <sha> | tar -x -C /tmp/…` 的**副本**里跑；每个副本的改动文件都与
> `git show <sha>:<path>` 做了 md5 对照（**全部 SAME**，见 `raw/w1b_flutter_all.txt` 头部）。
> 本轮**未改任何源码/已有文档、未 commit、未碰 `/home/skystar/Live2D-Ai`**。

---

## 0. 两个前置更正（先把账对齐）

1. **W1 区间是 5 个 commit，不是 4 个**：`93416f0c..d140604f` 之间还有
   **`7eed27f2 ci(flutter): 字体运行态子集门禁接住 crates/*/src 文案改动（F-0005-5 另一半缺口）`**
   （只改 `.github/workflows/flutter-checks.yml`，1 行 `paths` + 头注）。它不在你给的清单里，
   我单独复核（§7）。
2. **测试条数阶梯（我实测，权威值）**：`1407 → 1422 → 1468 → 1474 → 1484`，见 §1。
   你给的「1407 → 1417 → 1468」里 **1417 对不上**（W1-c 那一步实测是 **1422**）；
   这个 1417 来自 W1-e 的 commit body（它写「净 +10（1407 → 1417）」——净 +10 对，
   但绝对数是 **1474 → 1484**，它沿用了 W1-a 时的旧基准）。见 F-1b-1。

## 1. 归档副本上的逐波门禁（`/tmp/w1b_<sha>`）

```console
$ for sha in cbbedef0 1d0c6e66 77da64e5 d140604f; do
>   git archive "$sha" | tar -x -C "/tmp/w1b_$sha"
>   (cd "/tmp/w1b_$sha/shell/flutter" && flutter pub get && flutter analyze && flutter test)
> done
```

| commit | 波次 | 副本完整性 | `flutter pub get` | `flutter analyze` | `flutter test` | 判定 |
|---|---|---|---|---|---|---|
| `cbbedef0` | W1-c | 4/4 SAME | exit 0 | `No issues found!`（3.5s） | **`+1422` All tests passed!** exit 0 | ✅ |
| `1d0c6e66` | W1-d | 6/6 SAME | exit 0 | `No issues found!`（3.4s） | **`+1468` All tests passed!** exit 0 | ✅ **与你实测一致** |
| `77da64e5` | W1-d2 | 5/5 SAME | exit 0 | `No issues found!`（3.4s） | **`+1474` All tests passed!** exit 0 | ✅ |
| `d140604f` | W1-e | 5/5 SAME | exit 0 | `No issues found!`（3.5s） | **`+1484` All tests passed!** exit 0 | ✅ |

原文：`raw/w1b_flutter_all.txt`（含每波 md5 对照与全文）

### 1.1 测试条数账（**只增不减**）

| 时点 | 运行总数 | 增量 | 来源 |
|---|---:|---:|---|
| `93416f0c`（W1-a） | 1407 | — | W1-a 复核（`W1/GROUNDING.md`） |
| `cbbedef0`（W1-c） | **1422** | +15 | 新增 2 文件：`message_bubble_semantics_reachability_test.dart`(8) + `mod_config_draft_test.dart`(7) |
| `1d0c6e66`（W1-d） | **1468** | +46 | **逐项对得上**：`content_contrast_test` 22（11 条声明，含 4 主题×3 面循环 ⇒ 运行 22）+ `font_runtime_subset_test` 13 + `design_tokens_lint_test` 5→16（+11）= **46** ✅ |
| `77da64e5`（W1-d2） | **1474** | +6 | `no_backdrop_filter_test.dart` 11 → **17**（钉子 11 组由 2 条扩到 9 条） |
| `d140604f`（W1-e） | **1484** | +10 | `memory_panel_bucket_switch_test`(3) + `stage_progress_fps_rebuild_test`(7) |
| 合计 | 1484 | **+77** | = 15 + 46 + 6 + 10 ✅ |

**用了删除行的只有 3 个既有测试文件**（`design_tokens_lint_test.dart` −14、`font_subset_test.dart` −15、
`no_backdrop_filter_test.dart` −17），但：

```console
$ git diff -U0 93416f0c..d140604f -- shell/flutter/test | grep -E "^-.*\b(test|testWidgets)\("
-    test('没有把 contentFaint 传给任何 Text / TextStyle', () {
-    test('图标用 contentFaint 是允许的（并确认确实有人这么用）', {
```
⇒ 只有这两条**旧用例被改写**（不是静默删除）：钉子 11 组由 **2 条 → 9 条**
（`扫描确实覆盖到了源码` / `判据本身有效` / `跨行写法也算` / `白名单对账` / 合成用例①~⑤），
文件整体 11 → 17 条；运行时计数单调上升 ⇒ **红线 8「测试条数不降」成立**。

## 2. 逐 commit 范围（`git show --name-status` 逐条核）

| commit | 声明文件 | 实际改动 | 判定 |
|---|---|---|---|
| `cbbedef0` | 2 lib + 2 test | `lib/ui/message_bubble.dart`、`lib/settings/sections/dev_tools_section.dart`、`test/{message_bubble_semantics_reachability,mod_config_draft}_test.dart` | ✅ 完全一致 |
| `1d0c6e66` | 2 lib + 4 test | `lib/design/tokens.dart`、`lib/ui/theme.dart`、`test/{content_contrast(N),design_tokens_lint(M),font_runtime_subset(N),font_subset(M)}.dart` | ✅ |
| `7eed27f2` | 1 CI | `.github/workflows/flutter-checks.yml` | ✅ |
| `77da64e5` | 2 lib + 3 test | `lib/ui/state_pill.dart`、`lib/settings/sections/appearance_background.dart`、`test/{content_contrast,font_runtime_subset,no_backdrop_filter}_test.dart` | ✅ |
| `d140604f` | 3 lib + 2 test | `lib/live2d/live2d_bridge.dart`、`lib/live2d/live2d_stage.dart`、`lib/settings/mods/memory_panel.dart`、`test/{memory_panel_bucket_switch,stage_progress_fps_rebuild}_test.dart` | ✅ |

**无第 9 个文件、无跨波次夹带**；W1 区间改动路径只有 `shell/flutter/{lib,test}` + `.github/workflows`。

## 3. 红线（整区间一次核）

```console
$ git diff --name-only 93416f0c..d140604f -- \
    shell/flutter/lib/api/ws_frame.dart shell/flutter/lib/api/ws_status.dart crates | wc -l
0
```
⇒ **WS 帧定义 / 连接状态枚举 / 全部 Rust 源码在 5 个 commit 上零改动** ✅（红线 1、`clean_for_tts`、
密钥、A1–A8、`IdleState`、`mod_count` 全部结构性未触碰）。
无新增第三方依赖（`pubspec.yaml` 不在任何 commit 的改动里）。

## 4. 你要点名的第 1 项：**F-0006-2 的数值断言可复算**（✅ 成立）

我用**自己实现的 WCAG 公式**（sRGB 线性化 + 0.2126/0.7152/0.0722；半透明前景按 `onSurface@α`
合成到不透明面，与 `Color.alphaBlend` 同语义）复算四主题 × 三面：

```text
[white] surface 0.60=4.079(4.068) 0.66=4.892 0.68=5.207(5.190)   AA OK
        surfaceAlt 0.60=3.959(3.964) 0.66=4.720 0.68=5.013(5.020) AA OK
        raised     0.60=3.802(3.809) 0.66=4.497 0.68=4.762(4.766) AA OK
[black] surfaceAlt 0.60=5.928 → 0.68=7.222   · [blue] 5.312 → 6.375   · [gray] 4.877 → 5.787
```

与声称逐条对照（13 组）：**全部命中，最大偏差 0.005**（8 bit 取整 vs 浮点合成）：

| 对照点 | 声称 | 我算 | 差 |
|---|---:|---:|---:|
| 白 surface 0.68 / 0.60 | 5.207 / 4.079 | 5.207 / 4.079 | 0.000 |
| 白 surfaceAlt 0.68 / 0.60 | 5.013 / 3.959 | 5.013 / 3.959 | 0.000 |
| 白 raised 0.68 / 0.66 / 0.60 | 4.762 / 4.497 / 3.802 | 4.762 / 4.497 / 3.802 | 0.000 |
| 黑/蓝/灰 surfaceAlt（0.60→0.68） | 5.93→7.22 / 5.31→6.38 / 4.88→5.79 | 5.928→7.222 / 5.312→6.375 / 4.877→5.787 | ≤0.005 |

- **「四主题 × 三面全部 ≥4.5」我独立复核成立**：最小值 = 白 `raised` **4.762** ≥ 4.5 ✅；
  旧值 0.60 在白主题三面 **4.079 / 3.959 / 3.802 全部 < 4.5** ⇒ 缺陷本体真实、修复方向正确。
- ⚠ **文档漏报一例**（F-1b-2）：**灰主题 `raised` 在 0.60 也只有 4.448 < 4.5**，
  而 `tokens.dart` 的头注只举了白主题（「白色主题实测 3.96–4.07」）。修复后灰 raised = 5.223 ✅，
  即**同一缺陷在暗色主题上也存在过**——结论不变，但文档的「暗色三套只升不降」说法不完整
  （「只升不降」为真，但不等于「旧值本来就达标」）。

## 5. 你要点名的第 2 项：**W1-d2 判别力自证**（✅ 成立，且我把行号对上）

我在 `/tmp/w1b_probe_judge`（`git archive 77da64e5`）里把 `appearance_background.dart:778`
的 `colors.contentMuted` **临时回退成 `contentFaint`**，然后在**同一份源码**上并排跑两个判据
（新判据直接用被测文件里的真函数 `nb.findFaintTextUses`，不是重写一份）：

```console
$ flutter test test/zz_judge_compare_test.dart
OLD_REGEX_HITS=0 []
NEW_JUDGE_HITS=[778:宿主 Text]
ALL_FAINT_USES=[778:copyWith, 974:Icon]
+2: All tests passed!       JUDGE_EXIT=0
```
⇒ **旧同行正则对这三处零命中（0），新结构判定命中（778，理由「宿主 Text」）**——
「零命中 = 通过」这条假绿灯确实被关掉了 ✅。

**行号 774 的来源**（我追出来了，不是矛盾）：对**迁移前**的源码（`1d0c6e66` 版，`contentFaint`
当时真的在 774 行）跑同一判据：

```console
$ flutter test test/zz_judge_pre_migration_test.dart
PRE_MIGRATION OLD_REGEX_HITS=0 []
PRE_MIGRATION NEW_JUDGE_HITS=[774:宿主 Text, 1143:宿主 Text]
```
⇒ 实施者的 `[774:宿主 Text]` 是**迁移前**的测量（迁移时插入了 4 行注释，行号顺移到 778）✅。
⚠ 但他引用的证据串**不完整**（F-1b-3）：同一份迁移前源码上判据实际报 **2 条**（774 与 1143），
他只列了 1 条。**结论不受影响**：那两处都在 W1-d2 里迁成了 `contentMuted`
（diff 里两条 `-color: colors.contentFaint` → `+color: colors.contentMuted`）。

**独立复算的全仓账**（我用判据文件里的真函数扫 `lib/**`）：

```text
TOTAL=21  text=2  decoration=19
  TEXT  lib/ui/field_row.dart:228  host=copyWith  reason=宿主 Text
  TEXT  lib/ui/field_row.dart:236  host=copyWith  reason=宿主 Text
  DECO  …chat_panel.dart:513 Icon / appearance_background.dart:974 Icon / dev_tools_section.dart:125 Icon …
```
⇒ 与实施者自报的 **「text=2 只剩 field_row.dart:228/:236（W1-b 债务）」逐字一致** ✅；
非令牌文件的装饰用法恰好 **5 处**（3 个 `Icon` + `theme.dart:376/:419`），与白名单「5 条全在用」对得上。

**附加判别力（我的 T4 篡改）**：把 `state_pill.dart` 空闲态 `tone` 改回 `contentFaint`：

```console
$ flutter test test/content_contrast_test.dart   → +21 -1，T4_EXIT=1
  Expected: Color(alpha: 0.7400 …)   Actual: Color(alpha: 0.6800 …)
$ flutter test test/no_backdrop_filter_test.dart → +16 -1，T4B_EXIT=1
```
⇒ 两条守卫（接线断言 + 结构判定白名单）**都**会红 ⇒ 这套测试有牙齿 ✅。
（顺带证实 W1-d2 自报的「绿着说谎」修正是真的：旧断言硬编码 `colors.contentFaint` 算比值，
迁移后它会继续绿着量一个产品里已不存在的组合；现在改成读 `uiPhaseView(UiPhase.idle,…).tone`
并断言 `== contentMuted`，**跟着接线走**。）

## 6. 你要点名的第 3 项：**W1-e 的审计修正**（✅ 成立，但只对一半）

`git blame` 独立核（`77da64e5` 的 `live2d_stage.dart:320-330`）：

```console
$ git blame -L 320,330 77da64e5 -- shell/flutter/lib/live2d/live2d_stage.dart
^a11fe515 (yuhanwanli99-cpu 2026-09-12 07:49:27 +0800 327)     setState(() {});
$ git show a11fe515 --format='%h %ad %s' --no-patch
a11fe515 Sat Sep 12 07:49:27 2026 +0800 release!: v0.1.0-rc.1 — 核心链路基线（公开历史重新起算）
```
- `_onBridgeChanged`（`:301`）末尾的**无条件 `setState(() {})` 在 `:327`**，blame 到 **2026-09-12**
  （远早于 rc.7 的 2026-09-28）⇒ **F-0010-2 的 fps 那一半在 HEAD 确实已经会刷新**；
  审计引用的 301–326 行**差一行正好漏掉 327** ⇒ 实施者的修正**成立** ✅。
  ⇒ **release note 里 F-0010-2 的描述需要修正**（fps 半边不是缺陷；真正的交付物是「最小重建面收窄」）。
- ⚠ **但只有一半**（F-1b-4）：**progress 那一半的消费口在 `d140604f` 仍未接**——

```console
$ git show d140604f:shell/flutter/lib/main.dart | grep -n "onProgress"
（无输出）
```
  ⇒ `Live2DStage.onProgress` 只是**提供了出口**，宿主没接 ⇒ 真机上「模型加载中… N%」**仍然冻住**
  （宿主只在自己重建时读快照）。实施者的诚实栏第 1 条已如实自报（消费口在 `main.dart`，属 W1-b，
  已派 task-18）。**因此 F-0001-4 / progress 这一半在 `d140604f` 不算闭环**，release note 不能写成已修。

## 7. 你要点名的第 4 项：**W1-e 的最小重建面会不会漏场景**（我未找到反例）

把舞台 `build()` 里**所有**依赖穷举出来，逐个对收窄条件：

| build 读的东西 | 位置 | 谁触发重建 | 收窄后是否覆盖 |
|---|---|---|---|
| `_stageColorMotion` / `_displayedStageColor` | `:618-623`（`AnimatedBuilder`） | 动画控制器自己通知 | ✅ 与 `setState` 无关 |
| `_generation` | `:610` `ValueKey<int>(_generation)` | `:549 setState(() => _generation++)` | ✅ 自带 setState |
| `_bridge`（null→非 null） | `:609` | `_attach` 里 `setState(() {})` | ✅ |
| `_bridge.isReady` | `:647`（角标显隐） | 收窄条件 `ready != _lastBuiltReady`（`:367`） | ✅ |
| `bridge.fpsListenable` | `_FpsBadge` 的 `ValueListenableBuilder` | 订阅口自己通知 | ✅ |
| 其余（`lastAck`/`errorMessage`/`progress`） | **不参与本组件 build**，走 `onAck`/`onError`/`onProgress` 回调给宿主 | — | ✅ 不需要重建 |

⇒ **我没有构造出「需要重建却被收窄掉」的场景**；实施者还留了对照测试
（`stage_progress_fps_rebuild_test.dart` 里 `**最小重建面**：fps 推送不重建平台视图那一层` +
`对照：阶段/就绪变化**仍然**重建舞台`），我的穷举与它一致。
**唯一需要留意**：这条「完整性」依赖「舞台 build 不读别的 bridge 状态」——将来谁在 build 里加一个
bridge 字段的直读，就必须同步把它加进 `needsBuild`（建议在 `_onBridgeChanged` 头注里加一句纪律）。

## 8. 你要点名的第 5 项：**W1-c 语义树**（✅ 真泵成立，且无重复/漏播报）

我用**自己的探针**（真泵 `MessageBubble` + 遍历 `SemanticsOwner.rootSemanticsNode` 收集所有 label）
在 `d140604f` 副本上跑 5 条：

```console
① labels=[助手说：独角兽专属正文ZZZ, 复制]                        → 正文恰好 1 个节点
② 泄漏=[] 提到思考=[助手说：正文A（含思考 160 字）, 展开思考（160 字）] → 思考正文零泄漏 + 有告知/可展开
③（失败轮）find.semantics.byLabel('重试'/'复制') findsOne 且 tester.semantics.tap 生效（retried==1）
④ 「未收尾」=[未收尾：本轮语音未合成完，以上是已生成的正文]（恰好 1 个节点）
⑤（流式）labels=[助手说：流式正文YYY]
00:00 +5: All tests passed!
```
原文：`raw/w1b_semantics_probe.txt`；探针源码：`raw/zz_semantics_probe_test.dart`

- **真实泵路径成立**：用的是 `find.semantics.byLabel` + `tester.semantics.tap`（走 `SemanticsAction.tap`，
  不是命中测试），与实施者的测试同一手法，我独立复现 ✅。
- **无重复播报**：正文只出现在外层 label 一处（内层 `_MessageBody` 被 `ExcludeSemantics` 包住）——
  ①②⑤ 都断言了「含正文的节点恰好 1 个」，全部通过。
- **无漏播报**：正文（身份+内容）、思考告知（含字数）、失败轮的 `重试`/`复制`、`未收尾` 说明行，
  四个都在语义树里可 `find` 到 ✅。
- **思考正文不念**：`泄漏=[]`（20 遍「秘密思考串QQQ」零命中）✅。

## 9. 第 6/7 项：红线与范围（见 §2/§3）+ 第 5 个 commit `7eed27f2`

```console
$ git show 7eed27f2:.github/workflows/flutter-checks.yml | sed -n '/^on:/,/workflow_dispatch/p'
on:
  pull_request:
    branches: [main]
    paths:
      - "shell/flutter/**"
      - "crates/*/src/**"
      - ".github/workflows/flutter-checks.yml"
$ python3 -c "yaml.safe_load(...)"  → YAML OK; paths = ['shell/flutter/**', 'crates/*/src/**', '.github/workflows/flutter-checks.yml']
```
- 只改 1 行 `paths` + 头注 ✅；YAML 可解析、列表与自报一致 ✅。
- 它补的缺口是真的：W1-d 的 `font_runtime_subset_test.dart` 会扫 `../../crates/**` 的字面量，
  而原来的 `paths` 只匹配 `shell/flutter/**` ⇒ 只改后端文案的 PR 不会触发该门禁。
- 实现者自报的残留（**多触发**：`crates/*/src/**` 含 Rust 测试源，GitHub `paths` 无排除模式）我认同其
  方向（fail-open 不利少跑）；**我无法在本机运行 GitHub 自带 paths 匹配器**，只能核对等价 glob 语义。

## 10. 发现索引

| ID | 级别 | 一句话 | 状态 |
|---|---|---|---|
| F-1b-1 | P4 | 测试条数「1407 → **1417**」是旧基准；实测 **1474 → 1484**（净 +10 正确）。W1-e commit body 与你的账都需更正 | 待更正文字 |
| F-1b-2 | P4 | F-0006-2 文档只举白主题旧值；实测**灰主题 `raised` 在 0.60 也只有 4.448 < 4.5**（同一缺陷的暗色实例） | 已在本文留证 |
| F-1b-3 | P4 | 判别力证据串 `NEW_JUDGE_HITS=[774:宿主 Text]` 不完整：同一份迁移前源码上判据实际报 **2 条**（774、1143） | 结论不变 |
| F-1b-4 | **P3（交付缺口，非代码缺陷）** | F-0010-2 的 **progress 半边在 `d140604f` 仍无消费者**（`main.dart` 无 `onProgress`）⇒ 真机「加载中 N%」仍冻住；只有 fps 半边本来就工作 + 重建面收窄 | 待 task-18（依赖 W1-b）；release note 不得写成已修 |
| F-1b-5 | P4 | lead 清单漏了第 5 个 commit `7eed27f2`（CI-only，已单独复核通过） | 已补核 |

## 11. 未核实栏（**不许空**）

1. **`progress` 端到端不可验**：消费口在 `main.dart`（W1-b 在飞）⇒ 「真机加载幕布百分比会动」在
   `d140604f` **无法验**（也**尚未接线**，见 F-1b-4）。前提：task-18 落地 + 重建产物 + 真服务。
2. **产物未重建**：`shell/flutter/build/web/main.dart.js` 仍是 2026-09-28 的旧产物 ⇒
   **`/app/` 上看不到 W1-c/d/d2/e 的任何效果**；所有 `/app/` 结论都缺这一步（与 W1-a 复核同）。
3. **无真机/无浏览器**：对比度、语义树、最小重建面全部是 **VM 算术 + widget 树**证据；
   「运行时零 gstatic 请求」「读屏器实际怎么念」「观感是否变差（`state_pill` 空闲态 0.68→0.74）」
   **均未验**（实施者已自报第 4 条）。
4. **PageStorage 草稿的边界未验**：W1-c 自报「compact 下 `showModalBottomSheet` 是独立路由 ⇒
   关掉浮层再打开草稿不保留，只有分区切换保得住」——这是**路由级**行为的运行时前提，
   我**未起界面验证**，也未找到反例（VM 里 `PageStorage` 语义与真机路由一致，但没有真机对照）。
5. **`font_runtime_subset_test.dart` 的「89 个 Rust 产品文件 / 4,797 字面量 / 1,251 非 ASCII」未独立复算**：
   本轮只跑了全量门禁（绿）与 `analyze`，未逐项重算它的扫描统计数字。
6. **`ignite.sh --check` 未跑**（无服务，同前两轮）：托管层 `/` 302、`/app/` 200、HTTP 字节不含 gstatic
   三条**未验**。
7. **`7eed27f2` 的 GitHub `paths` 匹配器未验**（本机无法运行 GitHub 的匹配实现），只核了等价 glob 语义与 YAML。

---

## 附：原始输出清单（`docs/audit/2026-10-01-debloat/W1/raw/`）

| 文件 | 内容 |
|---|---|
| `w1b_flutter_all.txt` | 4 个 commit 的归档副本门禁全文（含每波 md5 对照） |
| `run_w1b_flutter.sh` | 上面那条循环的脚本（可原样重跑） |
| `w1b_judge_compare.txt` | 第 2 项：旧同行正则 vs 新结构判定（并排）+ 迁移前源码对照（解释 774） |
| `zz_judge_compare_test.dart` / `zz_judge_pre_migration_test.dart` | 上面两条探针的源码 |
| `w1b_semantics_probe.txt` | 第 5 项：语义树 5 条探针 + 全仓 `contentFaint` 账 |
| `zz_semantics_probe_test.dart` / `zz_faint_account_test.dart` | 上面两条探针的源码 |
| `w1b_tamper_t4.txt` | 第 5 项附加：把 `state_pill` 空闲态 tone 改回 `contentFaint` ⇒ 两条守卫都红 |
| （另见 `W1/GROUNDING.md` 的 `raw/`） | W1-a 的既有证据 |
