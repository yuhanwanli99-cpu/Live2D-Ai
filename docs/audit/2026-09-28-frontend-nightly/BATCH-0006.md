# BATCH-0006 — Phase 1 · lib/ui/ 下半 + lib/design/ 全量

> 账本：`AUDIT-B/`。行号基于 HEAD `5ef879f4`。

## 本批读过
| 文件 | 行 | 一句话 |
|---|---|---|
| lib/design/tokens.dart | 928 | 重点段：AppPalette(92-300) · AppColors(400-650) —— 四套配色、12 字段 copyWith/lerp/==/hashCode/toValuesMap **齐全**（radiusScale 刻意不入令牌面且有注释）✓；`panelAlphaFor` 纯函数 + 可读性下界 0.55 ✓ |
| lib/design/typography.dart | 126 | 8 槽位单一真源 + `sizeOfSlot` 声明↔生效对账 ✓；`buildAppTextTheme` 用 copyWith 派生（不新建 TextStyle，保住 fontFamily）✓ |
| lib/design/breakpoints.dart | 58 | 900/1280 单一真源 + 边界含等号 ✓；全仓无旁路比较（grep `.width [<>]` 零命中） |
| lib/ui/glass_rim.dart | 236 | 三层描边 + 固定数字；`shouldRepaint` 五字段齐全 ✓；onHover→setState（见 F-0006-4） |
| lib/ui/background_patterns.dart | 150 | 四种图案 CustomPainter + 固定种子（不沸腾）✓；`shouldRepaint` 四字段 ✓；预览包 RepaintBoundary ✓ |
| lib/ui/background_logic.dart | 209 | 轮播下一索引纯函数（随机绝不返回当前）✓；`patternColorsFor` 混合比例 ≤0.35（图案比 UI 安静）✓ |
| lib/ui/shell_backdrop.dart | 224 | 补审：`decodeDataUrlBytes` 永不抛 ✓；**每次宿主重建整图重解码**（F-0003-1，本次确认其触发频率是**每个 delta** → F-0006-1） |
| lib/ui/field_row.dart | 634 | 补审关键段：滑杆两端的 min/max 刻度标签用 `contentFaint` 当文字色（F-0006-2） |
| 门禁 | — | `test/design_tokens_lint_test.dart`(206) 全读：6 条规则 + 「扫描确实覆盖到源码」自检 + 豁免面收窄（裸间距只扫 EdgeInsets）——**质量高**；但断点规则的模式有洞（F-0006-3） |
| 对照测试 | — | `test/theme_palette_test.dart`（逐主题对比度断言清单）——`contentMuted`/`contentFaint` 压在 `dangerSurface`/`raised` 的组合**无断言**（我按 WCAG 公式实算：contentMuted 全部 ≥5.67 通过；contentFaint 白色主题 3.96–4.07 **不通过**） |
| 门禁扫描旁路 | — | `Color.fromARGB`（live2d_stage.dart:631，注释说明是从 CSS 字符串还原颜色，非设计决策）/ `Color.lerp`（tokens + stage 动效）——**不是绕过**（前者有正当理由、后者是从令牌推导） |

## 发现
- **F-0006-1（P1，升级 F-0003-1）** 壳背景图在**每个流式 delta** 上被 `base64Decode` + 整图重解码（1.5 M 字符预算 ⇒ 每次约 1.1 MB）。
- **F-0006-2（P2）** `contentFaint`（令牌自注「仅装饰/图标，不得承载文字信息」）被当**文字色**用在 3 处；白色主题实测对比度 **3.96–4.07 < 4.5**。
- **F-0006-3（P3）** 断点 lint 规则只匹配字面量 `maxWidth`，最自然的旁路写法不被拦。
- **F-0006-4（P3）** `GlassRim.onHover` 把**指针频率**的信号灌进 `setState`。

## 本批核对过、不成发现的（正面记录）
- F-0003-1 的**修法方向**在本批得到确证：`AppShell.build` 里 `ShellBackdrop(...)` 是即时构造（app_shell.dart:765），所以「宿主每次重建 = 整图重解码」这条链是真的；但严重度要按 F-0005-2 的频率重估（见 F-0006-1）。
- `GlassRim` 的 setState **不会**把 `widget.child` 一起重建：`Element.updateChild` 在 `child.widget == newWidget` 时直接复用元素（SDK `framework.dart:4027-4034`），而 hover 路径里 `widget.child` 是同一实例 ⇒ 只重跑 Stack/TweenAnimationBuilder/CustomPaint 四个 widget。这条**排除了**「鼠标移动重建整个设置面板」的更坏猜想。
- InlineNotice 的 danger 档文案虽然用 `contentMuted`（不是 `danger`），但实算四套主题 6.10–7.80，全部 ≥4.5 —— **不是**发现（此前怀疑已用 WCAG 公式证伪）。
- `MemoryImage.==` 按 `bytes` 身份比较已复核（SDK `image_provider.dart:1726-1734`）——F-0006-1 的机制证据。
- `no_backdrop_filter_test` 的毛玻璃规则与 lint 规则不重复（两者互指且有断言）✓；毛玻璃零命中的门禁是真的在扫（不是空转）。
- AppColors 的 ThemeExtension 五件套（copyWith/lerp/==/hashCode/toValuesMap）12 字段无一遗漏，`fieldCount = 12` 供结构枚举 —— 主题动画插值不会漏字段。

## 本批未核实
- F-0006-1 的**实际耗时**（每次 delta 多少毫秒）需真机 profile；机制层面已闭环。
- F-0006-2 的对比度是我按 WCAG 相对亮度公式实算（与 `theme_palette_test` 里的 `contrast()` 同一算法），**未在真机截屏复核**；白色主题的输入框 hint 与滑杆刻度在实际渲染下是否被抗锯齿/次像素影响，需要肉眼确认。
- `lib/design/background_item.dart`（274）B2 已全读，本批只复核 id/内容哈希路径，未重读。
