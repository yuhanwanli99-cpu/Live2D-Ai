# BATCH-0625 · 模型库分区：两处错误渲染走**两个不同的控件**，而其中一个带日期与编号

## 跑的命令（全部只读）
```
sed -n '180,196p' lib/settings/sections/dev_tools_section.dart
```

## 逐行
```
:182  const SectionHeader(title: '模型库',
:183    description: '模型由你合法导入，本仓库不捆绑任何模型二进制…')
:186  if (error != null) Padding(
:189    // 同上：统一走 `InlineNotice`（2026-09-11，P1-5）。
:190    child: InlineNotice(message: error!))
:193  if (activateMessage != null) Padding(
:196    child: EmphasizedText(activateMessage!, …))
```

## 四个可核点
1. ⇒ `error` 走 **InlineNotice**、`activateMessage` 走 **EmphasizedText**
   ⇒ 而 `:190` 的注释「**同上：统一走 `InlineNotice`（2026-09-11，P1-5）**」说的是**这个分区**
   ⇒⇒ **⇒ 同一段里两个「有话要说」的地方用了两个控件** ⇒ 与 B0527 核的「错误文案三套写法」**同族**
   （那一条是「三处写法」，这一条是「同一处两个控件」⇒ **根因不同、不合并**）
2. ⇒ 而「同上」**指的不是上一行**（上一行是 `SectionHeader`）⇒ 它指的是**别的分区/别处的同一条整改**
   ⇒ ⇒ 「同上」是**跨分区的指路**（与 B0609 核的「那个常量住在 render_events 而 C 栏 import 它」**同族**）
3. ⇒ 「（2026-09-11，**P1-5**）」⇒ 注释带**日期 + 编号** ⇒ 与 B0492 核的「三处引用 P2 / P6 / rc.2」**同族**
4. ⇒ 而 `EmphasizedText` 是 B0450 核过的那一个（AGENTS 记「UI 文案露出 Markdown `**`（15 处）→ 新增 `EmphasizedText` 真渲染 + 扫描规则」）
   ⇒ ⇒ **`activateMessage` 里的 `**` 会被真渲染，而 `InlineNotice` 里的不会** ⇒ ⇒ **两处的文案能力不同**
   ⇒ ⇒ **⇒⇒⇒⇒** 这一点**未核**（`InlineNotice` 是否也渲染 `**`，**需读它的实现**）

## 未核
`InlineNotice` 是否渲染 `**`（本批最值得核的一条）· 模型库分区的其余 body ·
`_okMessage` 之外的 activate 路径 · feature.yml · PULL_REQUEST_TEMPLATE.md
