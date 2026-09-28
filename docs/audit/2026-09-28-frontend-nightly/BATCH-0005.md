# BATCH-0005 — Phase 1 · lib/ui/ 上半（聊天与消息呈现面 + 共享组件族）

> 账本：`AUDIT-B/`（本 agent 独占）。行号基于 HEAD `5ef879f4`。

## 本批读过
| 文件 | 行 | 一句话 |
|---|---|---|
| lib/ui/chat_panel.dart | 569 | 列表 `ListView.builder` + reverse ✓；视图层 500 条裁剪 ✓；`_ListenButton` 计时器/dispose 对称 ✓；细看后无新发现 |
| lib/ui/message_bubble.dart | 549 | **气泡整条被 `excludeSemantics` 折成一个节点 → 复制/重试/思考折叠全部对读屏不可达（F-0005-4）**；宽度约束改 LayoutBuilder 正确 ✓；流式光标自绘竖条（红线 L）✓ |
| lib/ui/session_sheet.dart | 327 | 指针垫层在位（红线 M）✓；两段式删除不用 dialog（避开 overlay）✓；行内改名控制器 dispose ✓ |
| lib/ui/theme.dart | 466 | 令牌接进 ThemeData 唯一入口；`buildAppTheme(theme, material)` 由 main.dart:215 接偏好 ✓；`appPaletteOf/appColorsOf` 宁可炸不降级 ✓ |
| lib/ui/settings_scaffold.dart | 286 | 三宿主共用同一 widget ✓；未保存提示/保存条在位；`panelAlpha` 读 AppColors 单一真源 ✓ |
| lib/ui/state_pill.dart | 241 | 动画只在 thinking 启停、dispose 对称 ✓；`AnimationBehavior` 默认 normal（注释与实现一致）✓ |
| lib/ui/streaming_indicator.dart | 121 | `_controller!` 只在 thinking 分支用；didUpdateWidget/dipose 对称 ✓ |
| lib/ui/audio_bar.dart | 231 | 三轴命名纪律 + FocusTraversalOrder 固定 Tab 序 ✓；不在静音时置灰滑杆 ✓ |
| lib/ui/connection_badge.dart | 127 | 5 态 图标+字+语义 三通道 ✓；已连接时不渲染横幅（不说谎）✓ |
| lib/ui/inline_notice.dart | 172 | 全仓唯一 notice 外形；至多一个 primary ✓；liveRegion ✓ |
| lib/ui/soft_motion.dart | 185 | `appMotion` 唯一 reduced-motion 出口；`StartupReveal` 只播一次 ✓ |
| lib/ui/section_header.dart / group_card.dart / restart_notice.dart / emphasized_text.dart / theme_picker.dart / confirm_discard_dialog.dart / error_banner.dart / stage_corner_controls.dart | 759 | 逐个读完，无缺陷；confirm_discard_dialog 自垫 StagePointerInterceptor（M 红线的正确做法） |
| 补读接缝 | — | `lib/chat/chat_message.dart`(128) · `turn_liveness.dart`(200) · `shell_chat.dart`(54) · `app/collapsible_panel.dart`(103) · `main.dart:1067-1215` · `app_shell.dart:405-660` · `chat_controller.dart:90-400` |
| test（抽查） | — | `message_bubble_test.dart`(358 全读) · `chat_bubble_labels_test.dart`(174 全读) · `chat_notice_test.dart`(179 全读) · `semantics_test.dart`(315 全读) · `font_subset_test.dart`(341 全读) · `no_backdrop_filter_test.dart:230-253` |

## 发现
- **F-0005-1（P1）** RepaintBoundary 守卫是空转断言 ×2：断言对象是**一条说「不要这么做」的注释**。
- **F-0005-2（P1）** 每个 `text_delta`（项目自述「毫秒级」）重建整棵 AppShell 子树，**含常驻在树里的设置分区**（折叠态也照建）。
- **F-0005-3（P1）** 「空气泡不上屏」测试的结构上恒真断言；且被测行为（空白气泡面）其实**存在**。
- **F-0005-4（P2）** 气泡整条 `excludeSemantics` → 失败轮的**「重试」按钮对读屏不可达**（与 stage_host 已修过的同类 bug 同型）。
- **F-0005-5（P2）** 字体子集门禁只覆盖源码字面量；**模型输出 / 后端文案 / Mod 运行态值**不在门禁内 → 运行时仍会触发 fonts.gstatic.com 回退（断网豆腐块）。
- **F-0005-6（P2）** `chat_notice_test` 接线守卫「气泡层把 ChatRole.system 单独分流」删掉分流分支仍绿。
- **F-0005-7（P2）** `semantics_test` 的 reduced-motion 用例根本没泵 StatePill（连它声称的「状态标签还在」都没断言）。

## 本批核对过、不成发现的（正面记录）
- 发送失败**不丢草稿**是成立的：`_send` 先 `_input.clear()`（shell_chat.dart:42）但 `ChatController.send` 在 clear 之前已把用户消息 `sessions.append` 进会话并落盘，失败时 `_failLocalTurn` 就地收口成占位气泡 + 重试键 ⇒ 文字仍在界面上。与 F-0004-2 的差别在于那里清的是**还没发出去的**导入草稿。
- 气泡宽度约束：2026-09-27 改成 `LayoutBuilder` + `kBubbleReadingWidth.clamp(Space.s2, parent.maxWidth)`，三个断点下都真实生效（旧 `MediaQuery.width*0.92` 从未生效的历史已由注释与测试共同钉住）。
- `kChatHistoryLimit=500` 在**视图层**裁剪而不是数据层，注释给出了理由（导出排查需要完整对话）——与 §A 状态源唯一性不冲突。
- 红色 `⚠` 字形（inline_notice 头注记载的三套写法之一）已收敛为 Material 图标；`visual_language_test` 的星号扫描盲区已在 F-0003-4 登记。
- 字体子集门禁本身质量高：自写词法器（跳注释/解转义/三引号/原始串）+ 字体哈希同源校验 + 校准位断言（`▍`/`⌘` 必须不在子集）。**它的问题是覆盖面，不是判据**（见 F-0005-5）。
- `CollapsiblePanel` 四条通路（IgnorePointer/ExcludeFocus/ExcludeSemantics/TickerMode）齐全，child 提到 builder 外保证 Element 复用——这是本项目「舞台保活」纪律在设置面板上的正确同构。
- `_ListenButton` 的 PTT 三态（点按/按住/取消）计时器在 dispose/cancel 都归零，无泄漏。

## 本批未核实
- F-0005-2 的**幅度**（是否肉眼可见掉帧）需真机 profile；机制层面已由 SDK 源码闭环。
- F-0005-5 需真浏览器实测一次含 emoji 的模型回复（验证是否真的发出 fonts.gstatic.com 请求）；本环境无 CDP。
- `_ChatPanel` 在 expanded/medium/compact 下的实际重建子树大小（各分区 widget 数）未逐个数。
