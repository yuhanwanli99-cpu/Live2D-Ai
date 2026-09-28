# BATCH-0020 — Phase 2 · 横扫 H：路由 / 懒加载 / 舞台保活

> 账本：`AUDIT-B/`。**本批无新发现**（如实记录并换轴；§9 反空转）。

## 一、舞台保活（N 红线）——三条路径逐条核

| 宿主 | 机制 | iframe 是否离开树 | 证据 |
|---|---|---|---|
| expanded 内联侧板 | `InlineSettingsDock(expanded:)` + `CollapsiblePanel`（`widthFactor: 0` 而不是条件插入） | **否** | collapsible_panel.dart:21-24 注释 + 实现；`stage_keepalive_test` 覆盖 |
| medium 底部浮层 | `showModalBottomSheet` 挂在 **overlay**（导航栈之上），外壳 `body` 不变 | **否**（结构上不可能变） | app_shell.dart:477-495 |
| compact 整页 | `PageCrossFade` **两页都常驻**，隐藏那页用 `Offstage` | **否** | page_cross_fade.dart:128-139（「与 Morrow 的偏离」专段说明就是为了保活） |

- `test/stage_keepalive_test.dart` 的 6 个场景含**跨断点往返**（1400→1000→500→1400→899→1280，断言 `inits == 1`）与**外壳 setState**（消息追加），并且有一条**反向对照**（:192「换成不同的父结构 → initState 真的会再调一次」）——**有反向对照的计数型断言才是可信的计数型断言**，这是本仓测试做得最好的一处。
- 补核一处 0001 留下的疑问：「`PageCrossFade` 的 `first: body` 每次 build 都是新实例，会不会把 iframe 重建？」**不会**：平台视图的 DOM 元素只在 Element **挂载**时创建，而两个页面永远在 `Stack` 里（`Offstage` 不卸载），重建只走 `update` 不走 `mount`。

## 二、懒加载

| 触发 | 实现 | 判定 |
|---|---|---|
| 打开设置 | `onEnsureSectionLoaded` → `_ensureSettingsLoaded()`（`shell_settings.dart:14-23`：`Future.wait([_settings.load(), _loadAdmin(), _loadPresetLabels()])`） | ✓ 一次性 + 幂等锁 |
| 首失败后重试 | `shell_settings.dart:224` 的「重试」按钮直接调 `_settings.load()` | ✓ **不**会被 `_settingsLoadedOnce` 永久锁死（这是本批专门去查的一个潜在坑，查完**不成立**） |
| 换分区 | `_gotoSection` → `_ensureSettingsLoaded()`（幂等，无重复请求） | ✓ |
| 分区内容 | `sectionBuilder` 只构**当前**分区 | ✓ |

## 三、本批核对过、不成发现的（正面记录）
- 「没有路由表」这件事在本项目不是缺陷而是简化：入口 `/app/` 由 Rust 托管，壳内三宿主共用同一棵子树，于是「断点切换会不会重建舞台」这个高风险问题**被结构消灭了**（同一棵 `Flex` 只换 direction/flex）。
- `PageCrossFade` 为 compact 偏离了参考实现（Morrow 用条件插入），理由写在代码里且**与本项目红线一致**——这类「刻意偏离参考实现」都写了理由，是可审计的资产。

## 本批未核实
- 真实浏览器里 iframe 在 Offstage 期间的**渲染节流**行为（Offstage 的子树仍参与 layout，平台视图在 Chrome 上的合成行为）需真机观察；这不影响「不重建」结论，只影响「折叠时舞台还在画多少」。
