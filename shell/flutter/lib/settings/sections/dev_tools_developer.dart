part of 'dev_tools_section.dart';

/// 「开发模式」页（2026-10-09 收窄：只剩开关 + 诊断）。
///
/// # 这一页现在只剩两件东西
///
/// 1. **开发者模式开关**（含 `--dev-mode` 强制开启时的不谎报可关）；
/// 2. **诊断**（一级「诊断」取消后收进这里，只在 devMode 为真时渲染）。
///
/// 表情调试 / 动作调试 / **临时幅度** 三块已搬到「扩展 → 导演」卡片的下级
/// （`settings/mods/director_debug_panels.dart`）：它们产出的是动作与表情，
/// 与导演是同一件事的两端。**能力没删**，仍只在开发者模式里显示，
/// 也**没有**接回 `action_tx`。
///
/// 「导演可观测」四栏同样住在导演卡片上（`director_observer_section.dart`）。
class DeveloperSection extends StatelessWidget {
  const DeveloperSection({
    required this.devMode,
    required this.onDevModeChanged,
    required this.forcedByLaunchFlag,
    this.diagnostics,
    super.key,
  });

  final bool devMode;
  final ValueChanged<bool> onDevModeChanged;

  /// **诊断面**（2026-10-09：一级「诊断」取消，内容收进这一页）。
  ///
  /// 传的是**已经带着数据构造好的** DiagnosticsSection（宿主在
  /// shell_settings.dart 里装好）；本页只决定**画不画**——devMode 为真才画。
  /// 这样开发模式页拿到的是同一份诊断组件，不是复制出来的一套。
  final Widget? diagnostics;

  /// 是否由启动参数（`--dev-mode`）强制开启。
  ///
  /// **不谎报成功**：强制开启时开关要显示为「已由启动参数开启」且不可关，
  /// 而不是让用户点一下、看起来关了、其实没关（规格 §13.1-11）。
  final bool forcedByLaunchFlag;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(title: '开发模式'),
        ToggleField(
          label: '开发者模式',
          icon: Icons.terminal_outlined,
          value: devMode,
          enabled: !forcedByLaunchFlag,
          // 2026-10-09：控件下面的功能介绍全删，只留一条**操作规则**——
          // 启动参数强制开启时不能谎报可关（这是事实，不是介绍）。
          description: forcedByLaunchFlag
              ? '当前由启动参数强制开启，无法在界面里关闭'
              : null,
          onChanged: onDevModeChanged,
        ),
        // 诊断只在开发模式里渲染（本 if 块一起消失）。
        // 调试面板与导演可观测**已搬到**「扩展 → 导演」卡片下级
        // （settings/mods/director_debug_panels.dart / director_observer_section.dart）。
        if (devMode && diagnostics != null) ...<Widget>[
          const Divider(),
          diagnostics!,
        ],
      ],
    );
  }
}
