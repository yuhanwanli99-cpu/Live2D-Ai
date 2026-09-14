/// Mod 自有「产品面板」的扩展点（产品级加强波次）。
///
/// # 为什么要有这一层
///
/// 五个已注册 Mod 各有自己的产品级诉求（external-input 的计数、memory 的清空、
/// persona 的卡导入、director 的决策面板……）。如果每个都直接改
/// `sections/dev_tools_section.dart`，五条并行轨就必然在同一段 widget 树上冲突。
/// 于是把「每个 Mod 的产品面板」拆成**一个 Mod 一个文件**，由这里的注册表按 id 分派：
/// 各轨道只改自己那个文件，共享文件零改动。
///
/// # 契约
///
/// - `modId` 必须与 `ModDescriptor.id` 逐字一致；
/// - `stateLabels` 把该 Mod 的运行态字段翻成中文（未知 key 仍然不隐藏）；
/// - `build` 返回 null = 这个 Mod 不需要额外面板（只靠配置表单 + 运行态）。
library;

import 'package:flutter/widgets.dart';

import '../../api/mods_api.dart';

/// 面板可用的上下文：当前 Mod、运行态快照、以及两个**危险动作**回调。
///
/// 为什么用回调而不是直接给 `ModsApi`：与同层 `ModStateLoader` 同款——
/// 宿主接线一次，widget 测试注入 fake 即可覆盖「点清空 → 计数归零」这类交互，
/// 且面板不需要知道 base URL。
class ModPanelContext {
  const ModPanelContext({
    required this.mod,
    required this.state,
    required this.stateLoading,
    required this.stateError,
    required this.onRefreshState,
    required this.onCommand,
  });

  /// 当前 Mod 的列表项（含 `enabled` / `config` / `settings_spec`）。
  final ModInfo mod;

  /// 最近一次运行态快照；`null` = 还没取到（或取失败）。
  final Map<String, Object?>? state;

  /// 运行态是否正在读取。
  final bool stateLoading;

  /// 运行态读取失败时的**可处置**文案（已带错误码）。
  final String? stateError;

  /// 重新拉一次运行态（清空/保存后刷新计数用）。
  final Future<void> Function() onRefreshState;

  /// `POST /api/v1/mods/{id}/command`：一次性动作（清空 / 导出 / 自检……）。
  final Future<ModCommandResult> Function(
    String command, [
    Map<String, Object?> args,
  ])
  onCommand;

  /// 该 Mod 当前是否启用（面板据此决定按钮是否可点）。
  bool get enabled => mod.enabled;
}

/// 一个 Mod 的产品面板。
abstract class ModPanel {
  const ModPanel();

  /// 负责的 Mod id（= `ModDescriptor.id`）。
  String get modId;

  /// 该 Mod 运行态字段的中文标签（合并进统一的运行态渲染）。
  Map<String, String> get stateLabels => const <String, String>{};

  /// 产品面板本体；返回 null = 不需要额外面板。
  Widget? build(BuildContext context, ModPanelContext ctx);
}
