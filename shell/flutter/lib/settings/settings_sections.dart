/// 设置分区清单：**纯声明，无 web 依赖**（规格 §4.0「单点真相」）。
///
/// 新增一个分区 = 改**一处**枚举 + 加一个 pane builder。
/// **不要**把 order/标题分散到 N 个文件——改一次顺序要动 N 个文件，
/// 且重号不报错（静默失效，原则 P4）。
///
/// 顺序 = 声明顺序（枚举天然有序）。
///
/// # 2026-10-09：一级从 8 项收成 **7 项**（本轮口径）
///
/// | 顺序 | 一级 | 里面有什么 |
/// | --- | --- | --- |
/// | 1 | 模型对话（枚举仍是 persona） | 系统提示词 + 记住几轮。**只改前端显示名** |
/// | 2 | 模型库 | 控件不动 |
/// | 3 | 模型服务 | 原「对话」+ 原「语音合成」合成一项；页内两组（语言模型 / 语音合成） |
/// | 4 | 主题 | 从「外观与互动」拆出：配色 + 背景 |
/// | 5 | Live2D 设置（枚举仍是 motion） | 舞台与口型 + 允许拖动与缩放 + 渲染档位 |
/// | 6 | 扩展 | 不动入口（导演卡片多收了两块） |
/// | 7 | 开发模式 | 开关仍在这一项；诊断内容收进来 |
///
/// 被取消的两项：一级**「诊断」**（DiagnosticsSection 挪进开发模式页，
/// 仅 devMode 为真时渲染）；一级**「对话」「语音合成」**（合成「模型服务」）。
///
/// # 分区枚举没有持久化
///
/// （localStorage 只存 DisplayPrefs 与聊天会话，见 app/browser_io.dart 的两个存储键），
/// 所以改名/删项不影响老用户——打开时恒落在 main.dart 的默认分区上，
/// 不存在「按 index/name 读回旧分区」的路径。
///
/// 每项**强制带 description**：用户不该先学会产品黑话才能看懂一个分区是干什么的。
/// 由 settings_sections_test.dart 断言非空 + 无重号 + 不复读 label。
library;

import 'package:flutter/material.dart';

/// 「模型服务」页内**语音合成**组的稳定定位值（2026-10-09）。
///
/// 错误横幅的「去语音合成设置」不能靠滚动碰运气：它把目标分区与这个组值
/// 一起交给外壳，ServiceSection 用 GlobalKey + Scrollable.ensureVisible 落地。
/// 值住在声明层（不是 service_section.dart）：ui/error_actions.dart 也要用它，
/// 而那一层不该 import 一个 widget 库。
const String kServiceVoiceGroup = 'voice';

/// 7 个设置分区。
enum SettingsSection {
  // 2026-10-09：label 从「人设」改成「模型对话」——**只是显示名**，
  // 枚举值、跳转与 byIndex(0) 仍落在 persona 上（口径见计划书「模型对话」节）。
  persona('模型对话', '系统提示词与记住的轮数', Icons.badge_outlined),
  models('模型库', '导入、激活与舞台显示配置', Icons.view_in_ar_outlined),
  // 原 llm（「对话」）+ 原 tts（「语音合成」）合成这一项。
  service('模型服务', '语言模型与语音合成', Icons.hub_outlined),
  // 原 appearance（「外观与互动」）拆成 theme + motion 两项。
  theme('主题', '配色与背景', Icons.tune),
  // 2026-10-10：显示名由「Live2D 动作」改为「Live2D 设置」（枚举值不改）。
  motion('Live2D 设置', '舞台与口型、拖动缩放', Icons.accessibility_new),
  mods('扩展', '额外能力的开关', Icons.extension_outlined),
  developer('开发模式', '高级参数与开发者工具', Icons.terminal_outlined);

  const SettingsSection(this.label, this.description, this.icon);

  /// 分区标题（导航与标题栏共用，**唯一**文案来源）。
  final String label;

  /// 一句话说明（一句话，短；**不是**功能介绍段落）。
  ///
  /// 2026-10-09 口径收紧：分区说明只留一句短的——控件下面那些功能介绍
  /// 全删（见 SectionHeader / FieldRow 的 description 面）。
  final String description;

  /// 导航图标。
  final IconData icon;

  // 分区索引直接用 Enum.index（增强枚举自带，等于声明顺序）——
  // **不要**再声明一个同名 getter：Enum 已经有 index，重名是编译错误。
  // NavigationRail.selectedIndex 直接吃它。

  /// 按索引取分区；越界回落第一个（**不抛**——导航状态不该能让页面崩）。
  static SettingsSection byIndex(int index) =>
      (index >= 0 && index < SettingsSection.values.length)
      ? SettingsSection.values[index]
      : SettingsSection.values.first;
}

/// **可见**分区（顺序 = 枚举声明顺序）。
///
/// # 为什么现在与 devMode 无关（2026-10-09 起）
///
/// 过去只有「诊断」是 devOnly：它是整屏排障面，没开开发模式时藏起来。
/// 本轮**取消了一级「诊断」**，它的内容挪进「开发模式」页并且只在
/// devMode 为真时渲染——于是**没有任何一级分区需要按 devMode 过滤**。
///
/// 参数 devMode 保留：调用点有 20 余处（导航 / 测试 / 外壳），
/// 它是「设置面当前是否处于开发者模式」的既有信号；这里不再用它过滤，
/// 但**不要**据此以为还能藏某个分区：
///
/// - 「开发模式」这一项**永远在**——它是打开 dev_mode 的唯一入口，
///   一旦跟着自己藏起来，界面上就再也打不开（旧实现论证见 git 历史）；
/// - 要藏某个**页面内的块**，判据在那块的 builder 里（例如 DiagnosticsSection
///   与 DirectorObserverSection 都各自短路一次），不在这里。
List<SettingsSection> visibleSections({bool devMode = false}) {
  // devMode 目前不参与过滤（见头注）；保留形参是为了调用点签名不变。
  return List<SettingsSection>.unmodifiable(SettingsSection.values);
}
