/// 设置分区清单：**纯声明，无 web 依赖**（规格 §4.0「单点真相」）。
///
/// 新增一个分区 = 改**一处**枚举 + 加一个 pane builder。
/// **不要**像某些同类项目那样把 `order`/标题分散到 N 个文件——改一次顺序要动
/// N 个文件，且重号不报错（静默失效，原则 P4）。
///
/// 顺序 = 声明顺序（枚举天然有序）：
/// **角色（人设 + 模型）→ 能力（LLM + TTS）→ 外观与互动 → 扩展/诊断/开发**。
///
/// 2026-09-11：`actions` 分区项改名为 `appearance`（label 不变）。
/// **分区枚举没有持久化**（`localStorage` 只存 `DisplayPrefs` 与聊天会话，
/// 见 `main.dart` 的两个存储键），所以改名不影响老用户——打开时恒落在
/// `main.dart` 的默认分区上，不存在「按 index/name 读回旧分区」的路径。
///
/// 每项**强制带 `description`**：用户不该先学会产品黑话才能看懂一个分区是干什么的。
/// 由 `settings_sections_test.dart` 断言非空 + 无重号。
library;

import 'package:flutter/material.dart';

/// 8 个设置分区。
enum SettingsSection {
  persona('角色卡', '人设、开场白与历史轮数', Icons.badge_outlined),
  models('模型库', '导入、激活与舞台显示配置', Icons.view_in_ar_outlined),
  llm('LLM', '对话模型的服务地址、模型名与密钥', Icons.hub_outlined),
  tts('语音合成', 'TTS 服务、音色与采样参数', Icons.record_voice_over_outlined),
  appearance('外观与互动', '配色主题、舞台口型、拖动缩放与展台图', Icons.tune),
  mods('Mod', '扩展模块的启停与配置', Icons.extension_outlined),
  diagnostics('诊断', '连接、延迟、帧率与日志', Icons.monitor_heart_outlined),
  developer('开发模式', '高级参数与开发者工具', Icons.terminal_outlined);

  const SettingsSection(this.label, this.description, this.icon);

  /// 分区标题（导航与标题栏共用，**唯一**文案来源）。
  final String label;

  /// 一句话说明（AIRI 的每张卡片都有人话说明，用户不需要先学会黑话）。
  final String description;

  /// 导航图标。
  final IconData icon;

  // 分区索引直接用 `Enum.index`（增强枚举自带，等于声明顺序）——
  // **不要**再声明一个同名 getter：`Enum` 已经有 `index`，重名是编译错误。
  // `NavigationRail.selectedIndex` 直接吃它。

  /// 按索引取分区；越界回落第一个（**不抛**——导航状态不该能让页面崩）。
  static SettingsSection byIndex(int index) =>
      (index >= 0 && index < SettingsSection.values.length)
      ? SettingsSection.values[index]
      : SettingsSection.values.first;

}

/// **全部**分区（顺序 = 枚举声明顺序）。
///
/// # 为什么不再按 `dev_mode` 过滤（2026-09-11 修）
///
/// 「渐进披露」在这里指的是**分区内部的高级字段**只出现在第二层
/// （规格 §4.1 的「层」列），**不是**把「开发模式」这一整个分区藏起来。
///
/// 藏起来会形成一个走不出去的死循环：唯一能打开 `dev_mode` 的开关
///（`DeveloperSection` 里的那个 Toggle）**自己就在被藏起来的分区里**。
/// 结果是 `dev_mode` 在界面上**永远打不开**——只有 curl / `--dev-mode`
/// 启动参数能开。这跟「点了没反应」是同一种病：控件在那里，但到不了。
///
/// 所以：分区清单恒定；`dev_mode` 只影响**分区内部**哪些字段出现。
List<SettingsSection> visibleSections() =>
    List<SettingsSection>.unmodifiable(SettingsSection.values);
