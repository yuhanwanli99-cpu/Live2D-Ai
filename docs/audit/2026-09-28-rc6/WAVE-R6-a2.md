# 审查记录 · 波次 R6-a2（背景域三条 P1）· 2026-09-28

> **审查者**：编排者（Lead）。**实施者**：teammate `r6-a2-bgaudit`。
> 纪律同 `WAVE-R6-a.md`：**下面每个数字都是编排者自己重跑得到的**；实施者自述只作待核验清单。

## 1. 范围（`ORCHESTRATOR-PROMPT` §3.1 的背景域 P1/P2）

| 发现 | 等级 | 内容 |
|---|---|---|
| F-0006-1 / F-0003-1 | **P1** | 壳背景图在**每个流式 delta** 上 base64Decode + 整图重解码（`MemoryImage` 恒 cache-miss） |
| F-0002-1 | **P1** | 偏好一变就全量重发 `stage-bg` + 全量落盘（音量滑杆每帧触发） |
| F-0013-1 / F-0001-3 | **P1** | 启动水合窗口：整体覆盖导致偏好/库改动回滚（+ 占位内存库吞字节） |
| F-0002-2 | P2 | 早期加图写进占位内存库，「已记住」是假话 |
| F-0002-3 | P3 | `_openStore` 的超时/退路在 web 上是死代码，注释与真实兜底不符 |

## 2. 交付（自述，仅登记文件与行数）

| 文件 | 行数 |
|---|---|
| `shell/flutter/lib/main.dart` | 1239 → **1284** |
| `shell/flutter/lib/app/shell_prefs.dart` | 478 → **500**（正好撞上限） |
| `shell/flutter/lib/live2d/live2d_bridge.dart` | 508 → **547**（加「行数豁免」头注，理由=协议客户端是一个整体） |
| `shell/flutter/lib/ui/shell_backdrop.dart` | 463 → **458**（只留委托） |
| `shell/flutter/lib/ui/audio_bar.dart` | 231 → **296**（Stateless→Stateful：草稿值 + onChangeEnd） |
| `shell/flutter/lib/data/background_hydration.dart` | 164 → **255** |
| `shell/flutter/lib/data/background_decode_cache.dart` | 新建 **105** |
| `test/shell_backdrop_decode_cache_test.dart` | 新建 **159** |
| `test/stage_bg_resend_dedupe_test.dart` | 新建 **146** |
| `test/audio_bar_volume_commit_test.dart` | 新建 **183** |
| `test/background_hydration_window_test.dart` | 新建 **280** |

新增 **23** 条回归 ⇒ 基线 1314 → **1337**。

## 3. 编排者独立复核（原始输出）

```
$ cd /home/skystar/Live2D-Ai-fe/shell/flutter && flutter analyze
Analyzing flutter...
No issues found! (ran in 2.2s)                      ← issue 数 = 0 ✅

$ flutter test
00:27 +1337: All tests passed!                       ← 1337 passed / 0 failed，exit 0 ✅
                                                       （我独立跑出来的总数与实施者自述一致）

$ ls -la shell/flutter/build/web/main.dart.js
-rw-r--r-- 1 skystar skystar 3271418 Sep 28 20:33 shell/flutter/build/web/main.dart.js
  ⇒ 产物 mtime 20:33 **晚于**最后一个被改 .dart（main.dart 20:32）✅

$ grep -c 'gstatic.com/flutter-canvaskit' build/web/index.html build/web/main.dart.js
index.html:0 / main.dart.js:0                        ← 断网红线 0/0 ✅
```

### 3.1 **旧树污染检查（本波次新增的强制项）**

实施者报告「不带 `workdir` 的 bash 有时会落到旧基线 `/home/skystar/Live2D-Ai`」。
编排者**独立核验**旧树**未被写入**：

```
$ cd /home/skystar/Live2D-Ai && git rev-parse --abbrev-ref HEAD   → mod/persona-polish
$ git status --porcelain=v1 | wc -l                              → 136      （会话开始时同为 136）
$ git status --porcelain=v1 | grep -c '^??'                      → 23       （会话开始时同为 23）
$ find . -path ./.git -prune -o -type f -newermt '2026-09-28 18:00' -print | head -10
  → （空）
```

**结论：旧树 136 脏 / 23 未跟踪，与会话开始时逐项一致，且无 18:00 之后的文件改动 ⇒ 无污染** ✅
（该隐患已写进 R6-b 的提示词首条：每条 bash 必须显式 `workdir`。）

## 4. 红-绿双向（实施者报告，编排者抽样核验其「绿」端）

实施者给的是三段式：**实现前 RED → 实现后 GREEN → 注入旧行为再 RED**，并声称还原后文件 sha256 与注入前逐字相同。
编排者**能独立确认的部分**：最终 `flutter test +1337 全绿`（§3）、
「注入回红」无法在不改源码的前提下复核 ⇒ 归入 **R6-d 之后统一做的独立红-绿复核**
（工作树静止时，由编排者亲自注释实现、确认变红、再还原）。

| 发现 | 实施者报告的 RED（原始） | 实施者报告的 GREEN |
|---|---|---|
| F-0006-1/F-0003-1 | `Expected: true / Actual: <false>`（identical）；`Expected: MemoryImage(#cb968) / Actual: MemoryImage(#cc983)` | `00:00 +6: All tests passed!` |
| F-0002-1 | 桥 `Expected: <1> Actual: <2>`；滑杆 `Expected: <1> Actual: <10>`；方向键 `Expected: <0.5> Actual: <0.55>` | `00:01 +48: All tests passed!` |
| F-0013-1/F-0001-3 | `Expected: <0.31> Actual: <0.8>`；`Expected: '...BBBB' Actual: <null>`；`length <2> Actual <1>`；`Expected: <0.42> Actual: <null>` + 3 条结构性守卫 | `00:00 +20: All tests passed!` |

## 5. 编排者对实施者 7 条未决的裁决

| # | 实施者未决 | 编排者裁决 |
|---|---|---|
| 1 | 音量改**松手提交**（onChangeEnd），拖动中不再逐帧改**播出**音量 | **接受**。理由：`_update` 的 bool 是「有没有真写进存储」的诚实性契约（rc.3 §9.1），防抖会让它说谎；且 `main.dart` 在 VM 侧加载不了（`package:web`），时序没有回归能钉。**拖动中读数跟手**已由实施者的断言保住。已记为用户可见行为变更，写进 rc.6 说明。 |
| 2 | appearance 区其余滑杆仍每帧落盘 | **转派 R6-b**（那是它的文件）：统一到 onChangeEnd/防抖 + 「一次拖动只提交一次」断言（已写进 R6-b 提示词第 9a 条） |
| 3 | 可选「背景库水合中」可观测值**没做** | **接受不做**（数据丢失已由 a)/b) 根治）。**明确不许**为它加一个没人接线的死参数（那正是本轮在修的 P1 形态）。转派 R6-b 作为**可选**项，做不做都要如实写。 |
| 4 | 保留 `BackgroundHydration.prefs` 派生视图（供 rc.5 的 P0-1/P0-2 回归） | **暂留**。线上落点已由结构性守卫钉死为 `applyBackgroundHydration(latest, h)`；彻底删视图需同时改 `background_store_test.dart` 6 处调用 ⇒ **记 Stage C3 一并处理**，不在 rc.6 动已收口的 P0 回归。 |
| 5 | bash 无 `workdir` 会落到旧树 | **已独立核验旧树无污染**（§3.1），并把「每条 bash 显式 workdir + 用绝对路径」写进后续所有提示词首条 |
| 6 | `main.dart` 1284 行、`live2d_bridge.dart` 547 行 | `main.dart` 超 1000 已如实标注为**既有债**（不是豁免申请）⇒ **Stage C3**；`live2d_bridge` 547 有头注理由，**接受** |
| 7 | 资产红线复核（未动 WS 帧/字段名、`clean_for_tts`、mod_count、V12/Q1） | **采信并登记**；R6-d 会再跑一次 `asset_guard_*` 与 `mod_count_is_five` 实证 |

## 6. 未决 / 风险（转给 R6-d 与 Stage C）

| # | 项 | 归口 |
|---|---|---|
| 1 | **独立红-绿复核尚未做**（注入式回红无法在实施者自述里复核）⇒ 工作树静止后由编排者**亲手**做 | **R6-d** |
| 2 | 音量「松手才改播出音量」是用户可见行为变更 —— 需在 rc.6 说明里写明；Win 肉眼确认可接受 | **R6-d 肉眼** |
| 3 | `live2d_bridge.dart` 547 行（豁免头注）、`main.dart` 1284 行（既有债）、`shell_prefs.dart` 500 行（正好上限） | **Stage C3** |
| 4 | `BackgroundHydration.prefs` 派生视图的去留 | **Stage C3** |
| 5 | 未做「背景库水合中」界面提示（诚实性加成项，非数据安全问题） | R6-b 可选 / 记 residual |

## 7. 结论

R6-a2 **通过**（analyze 0 / test 1337 全绿 / build 与断网复核一致 / 旧树无污染）。
三条 P1 均落地并带三段式证据；**独立红-绿复核**与**音量语义变更的肉眼确认**排在 R6-d。
