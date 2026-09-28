# BATCH-0018 — Phase 2 · 横扫 F：无障碍 / 键盘 / 字体门禁覆盖面

> 账本：`AUDIT-B/`。

## 一、字体子集门禁的扫描根（假设「覆盖面不足」→ 核到底）

三道源码扫描门禁（`font_subset_test` / `design_tokens_lint_test` / `visual_language_test`）都以 `Directory('lib')` 为根，**不含 `web/` 与 `tool/`**。本批专门验证这是不是缺口：

| 根外文件 | 里面的界面文案 | 会不会走 CanvasKit | 判定 |
|---|---|---|---|
| `web/index.html` | `<title>Live2D Ai</title>`、manifest 的中文描述 | **不会**——浏览器标签页由系统字体渲染，不经过 Flutter/CanvasKit | **不构成缺口** ✓ |
| `tool/visual_review_test.dart` | 测试自己拼的标签（`Text('${s.label} · 字段 $i')`） | 会（它真的渲染），但产物是 PNG、**不随产物发布** | **不构成缺口** ✓ |

⇒ 红线 L 的门禁范围（`lib/**` 的界面文案）与红线的适用范围**恰好重合**——这是本批最值得记的一条「查了但没问题」。

## 二、无障碍盘点

| 项 | 盘点结果 |
|---|---|
| 图标按钮标签 | 全仓 6 个 `IconButton`，**5 个带 `tooltip:`**，第 6 个（发送/停止圆键）外面包 `Tooltip(message: '发送（Enter）'/'停止本轮')` ⇒ **全覆盖 ✓** |
| 滑杆 | `field_row.dart:205` 有 `semanticFormatterCallback`（念「标签 + 值」）+ `audio_bar.dart:105` 同样处理 ⇒ ✓ |
| 舞台 | `StageHost` 的 `Semantics(label: 'Live2D 舞台，正在显示 X')` + 覆盖层各自有标签（F-0005-4 的对照组）✓ |
| 折叠面板 | `CollapsiblePanel` 折起时 `ExcludeSemantics` ⇒ 收起的设置内容不会被读屏念到 ✓ |
| 主题卡 | `Semantics(button: true, selected: …, label: '黑，纯黑舞台，冷白强调')` ✓ |
| 状态胶囊 / 连接徽标 | `Semantics(excludeSemantics: true, label: '状态：思考中')` / `'实时通道：…'` ✓ |
| 焦点序 | 全壳 `FocusTraversalGroup(OrderedTraversalPolicy)`（app_shell:782）；聊天面板与音频条内部有 `NumericFocusOrder` 固定「音量 → 静音」✓ |
| **气泡内的动作条** | **✗ F-0005-4（已登记）**：`excludeSemantics` 吃掉复制/重试 |
| **滑杆刻度与输入框 hint** | **✗ F-0006-2（已登记）**：`contentFaint` 当文字色，白色主题 3.96–4.07 |

## 发现
- **F-0018-1（P3）** `api_client._errorFrom` 在响应体非 JSON 时合成 `http_<status>` 码——这个码**在后端日志里搜不到**（后端永远发标准错误体），与「用户拿界面上的码去日志里搜」这条契约冲突；且 `message` 会是整段 HTML。

## 本批核对过、不成发现的（正面记录）
- `text_scale_test.dart`(205) 已覆盖大字号下的溢出/截断；`keyboard_test.dart` 覆盖 4 条快捷键的绑定与作用域（`Semantics` 之外的那半条路径）。
- 焦点序的**全局 + 局部**两层都有定义，没有「顺序跟随 Widget 树、重构时静默改变」的空档。

## 本批未核实
- 真机读屏（NVDA/VoiceOver）实际体验无法在本环境验证（无 CDP/无浏览器）；本批全部是**代码层**盘点。
