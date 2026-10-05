# BATCH-0089 · 第二处「同形」检查：**不成立**（可空回调在生产里全部接上）

Phase 1 · 域覆盖 · 前端 `settings/sections/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/sections/appearance_section.dart` — 710（**定点 205-244**：
   `_BackgroundBlock` 的**生产构造实参** + 舞台单图轮播块头注）

## 跑过的命令（全部只读）
```
sed -n '205,244p' appearance_section.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**0 条新发现**；B0088 留的「第二处同形」**排除**
`_BackgroundBlock` 在 :275-290 声明的 **6 个可空回调** + 4 个非空回调，在
**生产构造点** `appearance_section.dart:211-227` 里**逐个接上**：
```dart
_BackgroundBlock(
  prefs: prefs,
  message: shellImageMessage ?? stageImageMessage,   // :215
  failed: shellImageFailed || stageImageFailed,       // :216
  onAddImage: onPickShellImage,          onClearLibrary: onClearShellImage,
  onPickStageImage: onPickStageImage,    onClearStageImage: onClearStageImage,
  onRemoveItem: onRemoveBackground,      onAddPattern: onAddPattern,
  onChanged: onPrefsChanged,             onReorderItem: onReorderBackground,
  onRemoveMany: onRemoveBackgrounds,     onPreviewItem: onPreviewBackground,
)
```
⇒ **可空签名是给「测试 / 复用」留的**（`appearance_background.dart:12` 也解释了
`part` 的理由是「只搬不改、可见性零变化」），**不是**某个功能静默不可达的迹象。
⇒ 与 B0087 核出的 F-0034-01 形成**明确对照**：
那一条的回调**接上了**、缺陷在**下游的判等**；这一批的回调**也接上了**、下游也没问题
⇒ **「回调未接上」这一族在本区只有一个实例**（就是 F-0034-01 那条，而它的根因不是接线）。

## 顺带两个正面样本（同一次读到的 :213-216 / :231-239）
1. **两条消息合并成一条**（:213-215），理由写明症状：
   「现在只有一个『背景』，**两个来源的消息轮流出现会让用户以为刚才那条是上一张图的**」
2. **两套轮播按来源互斥**（:231-239），并引用了**已记录的事故编号**：
   「两套列表、两套预算、两条下发通道，同时摆出来用户只会以为它们是同一个开关
   —— **rc.5 §9.2 记的就是这个重叠**」；还专门**改名**把「舞台单图轮播」与「壳背景轮播」分清
⇒ 与 `background_hydration` 的 P0-1/P0-2、`app_shell:443` 的 P0/2026-09-20 同一族：
**每次事故都留下编号与推理**，便于日后判断「这条约束还在不在」。

## 未核实项
1. `onPickShellImage` / `onRemoveBackground` / `onAddPattern` 等**回调自身**的实现
   （在 `shell_prefs.dart` / `shell_admin.dart`）未逐个核 —— 本批只核了「**接上了**」
2. `appearance_background.dart:46-275`（`BackgroundRuntimeScope` 全文 + `_CommitSliderField`）未读；
   16 个类里仍只碰了 6 个
3. `settings_controller.dart:120-322` / `:372-409` 未读
4. `background_store_web.dart` 余 ~190 行未读；其余 Mod 面板（4 个 / 2284 行）未读
5. `app_shell.dart` 全文未读；`main.dart` 其余 ~1000 行未读

## 本批新增
**0 条**
