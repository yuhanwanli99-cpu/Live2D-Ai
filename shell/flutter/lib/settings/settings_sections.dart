/// 设置分区清单：**纯声明，无 web 依赖**（规格 §4.0「单点真相」）。
///
/// 新增一个分区 = 改**一处**枚举 + 加一个 pane builder。
/// **不要**像某些同类项目那样把 `order`/标题分散到 N 个文件——改一次顺序要动
/// N 个文件，且重号不报错（静默失效，原则 P4）。
///
/// 顺序 = 声明顺序（枚举天然有序）：
/// **角色（人设 + 模型）→ 能力（LLM + TTS）→ 行为（动作与互动）→ 扩展/诊断/开发**。
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
  actions('动作与互动', '舞台口型、动作试演与拖动缩放', Icons.touch_app_outlined),
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

  /// 是否只在 `dev_mode = true` 时渲染。
  ///
  /// 只有「开发模式」分区本身受此约束；分区**内部**的 dev 字段由各 pane
  /// 自行按 `devMode` 显隐（规格 §4.1 的「层」列）。
  bool get isDevOnly => this == SettingsSection.developer;
}

/// 按 `devMode` 过滤出可见分区（顺序不变）。
List<SettingsSection> visibleSections({required bool devMode}) =>
    SettingsSection.values
        .where((SettingsSection s) => devMode || !s.isDevOnly)
        .toList(growable: false);
