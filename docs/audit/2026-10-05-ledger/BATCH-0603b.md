# BATCH-0603 落盘（极简）· ⚠⚠ 撤回 B0579b：「showModalBottomSheet 零处真实调用」是错的

## 编号
`BATCH-0603.md` 已存在 ⇒ 本批落 `BATCH-0603b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0603.md
sed -n '98,120p' lib/app/nav_host.dart
```

## 逐字（:100-120）
```
:100 /// 浮层约束。
:102 /// **只有 medium 会调它**（2026-09-11，P2-1）：compact 改成整页过渡之…
:103 /// `SettingsHost.sheet` 只剩 medium 一档。所以这里不再按断点分叉——
:104 /// 留一个「compact 更窄更矮」的分支只会变成没人走、却要跟着维…
:105 /// （P0 刚删掉一整批同类的）。
:106 BoxConstraints sheetConstraintsOf(Size viewport) => BoxConstraints(
:108   maxWidth: NavMetrics.sheetMaxWidthMedium,
:109   maxHeight: viewport.height * NavMetrics.sheetHeightFactorMedium,
:110 );
:112 /// 打开设置浮层（只有 medium 走这条路）。
:114 /// `showModalBottomSheet` **自带焦点陷阱 + Esc**，且 **overlay 不卸载下层**
:115 /// ⇒ 舞台 iframe 不会被重建（规格 §3.4 的保活约束之一）。
:116 Future<void> showSettingsSheet({
:117   required BuildContext context,
:118   required WidgetBuilder builder,
:119 }) => showModalBottomSheet<void>(
:120   context: context,
```

## 三个可核点（本批最重要的一处是自纠）
1. **⚠⚠⚠ 撤回 B0579b 第 1 点**。我当时写：
   「`grep -rn "showModalBottomSheet" shell/flutter/lib/` 的 6 条命中，
   **行首全是 `///`** ⇒ 零处真实调用」⇒⇒ **⇒⇒⇒⇒ **这是错的** ——
   **`:119` 就是真实调用**（`=> showModalBottomSheet<void>(`）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 假阴性来源（B0440 第 1 形态 + 第 8 形态同时中招）**：
   我 `head -6` 只看了前 6 条 ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ **「只看了前 N 条」＋「命中在注释里」
   这两个错叠在一起，结论翻了过来** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 记两条纪律**：
   - **计数类结论不许用 `head -N` 的结果**（必须数全量）
   - **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 「N 条命中全是注释」这句话本身也要再核一遍有没有未读的行**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒⇒⇒ 这条与 B0580 那次「撤回 showModalBottomSheet 指控」是**同一处代码**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 我先撤了指控、又断言「不存在」，两次都错在同一处**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ ⇒ 教训升级**：
   **对「某物不存在」的断言要格外小心 —— 存在容易证明，不存在很难。**
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:103-105` 是一段「主动不做」的决策记录，而且**给了依据**
   （「留一个『compact 更窄更矮』的分支只会变成没人走、却要跟着维护」
   ＋「**P0 刚删掉一整批同类的**」）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据：删代码时把「删的是哪一类」也写下来，
   后来者才知道这不是漏做** ⇒⇒ 与 B0601 `:47-52`「不要加回来」**同族**，
   但这里多了**一个外部证据（P0 那次删改）** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据（升级）：能被外部事件佐证的删除决定，比只能自述的更可信。**
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:114-115` 说明为什么用 `showModalBottomSheet` 而不是自绘**
   「**自带焦点陷阱 + Esc**，且 **overlay 不卸载下层** ⇒ 舞台 iframe 不会被重建
   （规格 §3.4 的保活约束之一）」
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据：用平台 API 的理由往往是「它自带某条你懒得自己实现的性质」** ——
   这里带的是**焦点陷阱**与**不卸载下层**两条 ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 而这条正好接上 B0580 的发现**：
   设置侧板用自绘 `CollapsiblePanel`（要保活），而 medium 浮层用 `showModalBottomSheet`
   （平台**自带**不卸载）⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒⇒ 两条路都要保活，但一个自己写、一个用现成的**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒ 判据：同一个需求有两种实现时，
   注释要写清「为什么这两处选了不同的那个」**（本仓此处**没写**，P3 候选，不立）。

## 未核
`:120` 之后的 `showModalBottomSheet` 参数（isScrollControlled 等）·
`showSettingsSheet` 的调用点 · 规格 §3.4 的原文 · 浮层里有没有聊天气泡
