# BATCH-0600 落盘（极简）· 两个档的聊天宽都 < 480 ⇒ 那道上限只在「被拉宽」时生效

## 编号
`BATCH-0600.md` **不存在**（本批首次占用该号）⇒ 直接落盘。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0600.md
grep -rn "paneWidth|chatWidth|340" lib/app/nav_host.dart lib/app/app_shell.dart
```

## 四处命中
```
nav_host.dart:40  static const double paneWidth = 400;
nav_host.dart:56  static const double chatWidthExpanded = 340
nav_host.dart:59  static const double chatWidthMedium = 320;
nav_host.dart:166 extent: NavMetrics.paneWidth,
```

## 四个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 两个档的聊天宽都小于 480**：
   `chatWidthExpanded = 340` < 480 ✓、`chatWidthMedium = 320` < 480 ✓
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 结论：`kBubbleReadingWidth = 480` 在
   **三档里两档（expanded / medium）都是空转的**，
   只有「面板被**拉宽**超过 480」（用户拖宽窗口、或更宽的屏）才生效
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 上一批第 ③ 点的疑问**完全落地了**：不只是 expanded，medium 也是
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 修正上一批「本条的适用性只能说到 expanded 档」** ——
   **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ 现在可以说两档。**
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 而 `paneWidth = 400` 也 < 480** ⇒⇒ 三处宽度全在 480 以下
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：给一个「可拉宽」的组件配固定上限时，
   先确认默认值离上限有多远** —— 差得远（340 vs 480，差 41%）⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 这条上限平时根本不参与决策，只有拉宽后才生效**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒ 好处：默认布局不复杂，异常宽时才收口。**
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:56-59` 两处都带**档名**（`chatWidthExpanded` / `chatWidthMedium`）**
   ⇒⇒ 与 B0594b 核的「枚举项文档写「它对应哪一档」」**是同一手法**，
   只是这次用在**常量名**上 ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据：
   **同类值成组出现时，名字里要带区分维度**（哪一档 / 哪个端），
   否则读者只能靠定义顺序猜。**
4. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:40` 与 `:56/:59` 的**前缀不同**：
   `paneWidth`（无类名）vs `chatWidth*`（无类名）⇒⇒
   **⇒⇒⇒⇒ ⇒ 都在 `NavMetrics` 里**（`:166` 写 `NavMetrics.paneWidth`）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 与 B0599b 的 `Space.s2` vs `kBubbleReadingWidth` 对比：
   `NavMetrics.*`（度量）vs `Space.*`（间距）vs `k*`（本地）⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 三种前缀对应三种来源，本仓分得清。**

## 未核
`:40 paneWidth = 400` 上面有没有 `///` · medium 档聊天面板是 320 还是别的值被覆盖
（`:59` 与 `:56` 之间只差 20 px，像是刻意贴近）· `nav_host.dart:129` 那段注释
