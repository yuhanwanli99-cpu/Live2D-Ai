# BATCH-0001 — Phase 1 · 入口与壳域（main.dart + lib/app/ 全部 12 文件）

日期：2026-09-28 · 基线 HEAD `5ef879f4`（代码同 157e30ab）
维度覆盖：A B C D E G I M N P（F/J 部分）；K 红线抽查（同源 /render，无外链）

## 本批读过（行数=实测 wc）
| 文件 | 行 | 一句话 |
|---|---|---|
| lib/main.dart | 1239 | 组合根，接线密集但守卫齐全；发现 F-0001-1/2/3/4 |
| lib/app/app_shell.dart | 892 | 布局/保活/指针垫层齐全；死参数 ×2 |
| lib/app/shell_prefs.dart | 478 | 偏好下发唯一漏斗 _applyPrefs；背景库删改不重置运行时索引 |
| lib/app/shell_settings.dart | 437 | 8 分区 pane 接线；串行 PATCH 链设计合理 |
| lib/app/shell_admin.dart | 256 | 管理面接线（待 batch7 复核非 ApiException 卡 loading 假设） |
| lib/app/shell_chat.dart | 54 | 发送/停止/播报，正常 |
| lib/app/nav_host.dart | 218 | InlineSettingsDock 已套垫层（红线 M ✓） |
| lib/app/page_cross_fade.dart | 195 | Offstage 保活两页（DOM 级证据见下「已证伪」） |
| lib/app/collapsible_panel.dart | 103 | 四路关断齐全 |
| lib/app/browser_io.dart | 157 | localStorage 读写永不抛；无泄密 |
| lib/app/app_shortcuts.dart | 130 | 快捷键；ASCII Cmd ✓ |
| lib/app/shortcut_help_dialog.dart | 66 | 正常 |
| 补读：ui/stage_host.dart 199 · ui/error_actions.dart 51 · ui/shell_slideshow.dart 102 · api/api_client.dart 267（_guard 全覆盖 ✓）· live2d/live2d_host_web.dart 88 · live2d_bridge.dart 局部 · live2d_stage.dart 局部 | — | 为 F-0001-1/4 建调用链 |

## 发现
- **F-0001-1（P1）** 错误横幅「去 LLM/语音合成设置」在面板关闭态只换分区状态、不打开面板——默认态下按钮点了没反应。见 FINDINGS。
- **F-0001-2（P2）** 轮播双索引漂移：`_jumpBackground`/缩略图预览不驱动 `ShellSlideshow.jumpTo`，删/清空库也不重置 `_backgroundIndex`。
- **F-0001-3（P2）** 启动 `_hydrateBackgrounds` 竞态：水合完成时用 initState 时的旧 prefs 整体覆盖，吞掉窗口期内用户改动，migrate 时还把旧对象写回磁盘。
- **F-0001-4（P2）** 加载覆盖层进度不刷新：bridge.progress 有 notifyListeners，但 stage→宿主只报 phase/ack/error，进度只在宿主偶然重建时被 GlobalKey 快照读一次。
- **F-0001-5（P3）** AppShell.onBackgroundIndex/onBackgroundJump 全仓零使用；app_shell.dart:177 头注指向不存在的 `ShellBackgroundHost`。

## 已证伪 / 反向证据（重要，防日后重提）
- **Offstage 会不会重载 iframe（compact 打开设置）？——证伪。** page_cross_fade 用 `Offstage` 隐藏工作台（含舞台）；
  Flutter 自 2.5 起平台视图内容常驻 `flt-glass-pane` shadow root 之外、只操作 slot 元素，「`flt-platform-view` 元素永不成为框架 DOM 操作的目标」，
  iframe 不因不合成而脱离文档 ⇒ 无重载。证据：https://docs.flutter.dev/release/breaking-changes/platform-views-using-html-slots-web ；
  本地 SDK Flutter 3.47.3 远晚于此。stage_keepalive_test 用 initState 计数钉 Dart 侧，DOM 侧由上述机制兜底。
- `_testLlm/_testTts` 只 catch ApiException：ApiClient._guard 把所有传输异常包成 ApiException、_decodeObject 永不抛、
  SettingsTestOutcome.fromJson 待 batch7 复核（候选：非 ApiException 逃逸 → testing 卡 true）。
- `_recordPresetRequest` 虚记假设：三处调用点都有 `stage != null` 守卫（main.dart:575/717/810），撤销。
- 导演 cue 调用点（红线 P）仍在 main.dart:604 `_audio.sentenceStarts.listen(_applyDirectorCueForSeq)`；
  asset_guard_director_cue_test 剥注释+自带判别力自证，不是空转断言。
- lib/app/ 无硬编码颜色（design_tokens_lint 扫裸值，app_shell 用 appColors/appPalette/Space）。

## 本批命令
git log/status；find+wc 全量清点；grep（syncSlideshow/jumpTo/openSettings/addListener/Color(0x/File(）；
curl 18080（200/302 活）；web_fetch flutter 官方 breaking-change 文档（Offstage/iframe 证伪）。

## 环境
- 浏览器工具不可用（127.0.0.1:9222 无 CDP，本机无 Chrome）——只读取证暂不可做，已记 STATE。
- 服务 18080 在跑（GET /app/ 200，GET / 302）——产物新鲜度未比对（不审计产物）。

## 本批未核实
- SettingsTestOutcome/ModelsApi 等解析层是否会抛非 ApiException（→ _loadAdmin/_testLlm 卡死候选）——batch7。
- nav_host 用裸 `AppRadius.xl` 而 app_shell 用 `appColorsOf().radius()`——半径缩放一致性，归 design/tokens 批。
- compact 真机上 PageCrossFade 切换观感（DOM 证伪后仅剩观感问题，待人类肉眼）。
