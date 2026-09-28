# BATCH-0016 — Phase 2 · 横扫 D：build() 重活 / 列表 / 同步重计算

> 账本：`AUDIT-B/`。D 轴已有三条登记（F-0005-2 / F-0006-1 / F-0006-4），本批**不重复**，转向尚未覆盖的面。

## 一、CustomPainter 清单（shouldRepaint 覆盖率）

| Painter | 位置 | shouldRepaint | 判定 |
|---|---|---|---|
| `LiquidRimPainter` | glass_rim.dart:157 | ✓ 五个字段全比 | ✓ |
| `BackgroundPatternPainter` | background_patterns.dart:24 | ✓ 四个字段全比 | ✓ |
| （无第三处） | `grep "extends CustomPainter"` 全仓仅 2 个 | — | **100% 覆盖 ✓** |

## 二、长列表盘点

| 列表 | 构造 | item 代价 | 判定 |
|---|---|---|---|
| 消息列表 | `ListView.builder` + `reverse`（chat_panel:233） | `MessageBubble`（含 `LayoutBuilder` + 可选 Markdown 解析） | ✓ 合理（只建可见项） |
| 背景库 | `ReorderableListView.builder`（appearance:864） | `_ImageTile`：**每 build 重解码 base64**（:1140） | **F-0016-1** |
| Mod 列表 / 会话列表 | builder 化 | 轻 | ✓ |
| 导演观测记录 | `ListView.builder` + `itemExtent: 20`（observer:511-513） | 纯 Text | ✓（连 itemExtent 都钉了） |

## 三、`preset_status` 的架构决策（正面）
`PresetStatus` 头注明写：v0 曾用 `Timer.periodic(200ms)` 本地推演「还剩多少毫秒」，那是**前端镜像**，阶段4d 删掉了——现在快照**只由渲染面 ack 生成**。这正是「派生值必须有权威源」的自我修正，与 F-0010-2（桥的 ack 有出口）刚好构成一对：**该有的出口有，不该有的镜像删了**。

## 发现
- **F-0016-1（P2，与 F-0006-1 同根因）** 背景库的每一张缩略图 `_ImageTile` 在**每次 build** 都重新 `base64Decode` 整张图并构造**新** `MemoryImage` ⇒ 与 F-0005-2 复合后是「N 张图 × 每个流式 delta 一次解码」，并额外造成 `ImageCache` 键抖动（每张图每秒新增一条缓存条目，300 条上限很快把仍在用的条目挤掉 ⇒ 缩略图会闪烁/重载）。

## 本批核对过、不成发现的（正面记录）
- `render_events.dart`(700) 与 `action_scales_sync.dart`(194)：前者是有界日志（`maxLines=200`）且观测列表是 builder + itemExtent；后者是纯防抖器（`_key` 拼串 + Timer），**都不在 build 里做重活**。
- `DirectorEventLog.lines` 每次访问会 `List.unmodifiable` 复制一份、`text` 每次 join——但读取点在 `ListView.builder` 之外且仅开发模式分区常驻，代价可忽略（记此备忘，不成发现）。

## 本批未核实
- F-0016-1 的**实际掉帧幅度**（N=8、图 400 KB 时每秒 240 次 base64 解码）需真机 profile；机制层面已闭环。
- `ImageCache` 抖动是否真的造成可见闪烁（取决于 300 条上限与图片解码耗时）需真机观察。
