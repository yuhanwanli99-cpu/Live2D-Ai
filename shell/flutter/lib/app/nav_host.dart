/// 设置导航宿主：**同一个 `SettingsScaffold`，只换宿主**（规格 §4.2）。
///
/// | 断点 | 宿主 |
/// |---|---|
/// | ≥1280 expanded | 内联侧板（400 px，浮在舞台列右缘），Esc / 点空白关闭 |
/// | 900–1279 medium | `showModalBottomSheet(isScrollControlled, maxWidth 560, maxHeight 0.78h)` |
/// | <900 compact | 抽屉列 8 项 → 选中后再弹 sheet（严格 2 层） |
///
/// # 为什么必须共用一个内容 Widget
///
/// 同类项目里最典型的崩坏是「同一个数写 3 遍」——440px/24px 出现在 3 个文件里，
/// 改一处忘两处【调研 survey §4.1】。这里的分区清单在
/// `settings/settings_sections.dart`（枚举）、尺寸在本文件的 [NavMetrics]，
/// 宿主函数只决定「往哪放」。
///
/// # 为什么 compact **不**用 `NavigationBar`
///
/// M3 的 `NavigationBar` 是 3–5 个 destination，而我们有 **8 个分区**；
/// 用「5 项 + 更多」会变成「设置 → 更多 → 分区」= **3 层**，直接违反 NN/g 的
/// 官方硬约束「超过 2 层披露通常可用性很低」。抽屉 + 弹层恰好 2 层。
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../design/breakpoints.dart';
import '../design/tokens.dart';
import '../settings/settings_sections.dart';

/// 浮层的宽度/高度（**唯一定义点**，宿主与测试都读这里）。
abstract final class NavMetrics {
  /// 内联侧板宽度（expanded）。
  static const double paneWidth = 400;

  /// medium 浮层最大宽度。
  static const double sheetMaxWidthMedium = 560;

  /// compact 浮层最大宽度。
  static const double sheetMaxWidthCompact = 520;

  /// medium 浮层最大高度比例。
  static const double sheetHeightFactorMedium = 0.78;

  /// compact 浮层最大高度比例。
  static const double sheetHeightFactorCompact = 0.72;

  /// 展开的 rail 宽度。
  static const double railExtendedWidth = 232;

  /// 图标 rail 宽度。
  static const double railIconWidth = 80;

  /// 常驻聊天面板宽度（expanded）。
  static const double chatWidthExpanded = 340;

  /// 常驻聊天面板宽度（medium）。
  static const double chatWidthMedium = 320;

  /// 舞台列最小宽度。
  static const double stageMinWidth = 480;
}

/// 设置宿主形态（由断点决定，**纯函数**，可单测）。
enum SettingsHost {
  /// 内联侧板。
  inline,

  /// 底部浮层。
  sheet,
}

/// 断点 → 宿主。`compact` 与 `medium` 都是浮层，区别只在宽度/高度。
SettingsHost settingsHostOf(SizeClass sizeClass) =>
    sizeClass.hasInlineSettings ? SettingsHost.inline : SettingsHost.sheet;

/// 浮层约束（按断点给，避免各处硬编码尺寸）。
BoxConstraints sheetConstraintsOf(SizeClass sizeClass, Size viewport) {
  final bool compact = sizeClass.isCompact;
  return BoxConstraints(
    maxWidth: compact
        ? NavMetrics.sheetMaxWidthCompact
        : NavMetrics.sheetMaxWidthMedium,
    maxHeight:
        viewport.height *
        (compact
            ? NavMetrics.sheetHeightFactorCompact
            : NavMetrics.sheetHeightFactorMedium),
  );
}

/// 打开设置浮层（medium / compact 共用）。
///
/// `showModalBottomSheet` **自带焦点陷阱 + Esc**，且 **overlay 不卸载下层**
/// ⇒ 舞台 iframe 不会被重建（规格 §3.4 的保活约束之一）。
Future<void> showSettingsSheet({
  required BuildContext context,
  required SizeClass sizeClass,
  required WidgetBuilder builder,
}) => showModalBottomSheet<void>(
  context: context,
  showDragHandle: true,
  isScrollControlled: true,
  constraints: sheetConstraintsOf(sizeClass, MediaQuery.sizeOf(context)),
  builder: builder,
);

/// compact：抽屉列 8 个分区（第 1 层）。
///
/// 返回被选中的分区；用户直接关掉抽屉则返回 `null`。
Future<SettingsSection?> showSectionDrawer({
  required BuildContext context,
  required bool devMode,
  SettingsSection? selected,
}) => showModalBottomSheet<SettingsSection?>(
  context: context,
  showDragHandle: true,
  constraints: BoxConstraints(
    maxWidth: NavMetrics.sheetMaxWidthCompact,
    maxHeight:
        MediaQuery.sizeOf(context).height * NavMetrics.sheetHeightFactorCompact,
  ),
  builder: (BuildContext sheetContext) {
    final List<SettingsSection> sections = visibleSections(devMode: devMode);
    return SafeArea(
      child: ListView(
        shrinkWrap: true,
        children: <Widget>[
          for (final SettingsSection section in sections)
            ListTile(
              leading: Icon(section.icon),
              title: Text(section.label),
              subtitle: Text(section.description),
              selected: section == selected,
              onTap: () => Navigator.of(sheetContext).pop(section),
            ),
        ],
      ),
    );
  },
);

/// 设置侧板（expanded 的内联浮层）。
///
/// 铺在舞台列**之上**（不是新增一列）：1280 下「232 rail + 400 设置 + 340 聊天」
/// 会把舞台压到 268 px，而舞台是产品核心（原则 P1）。浮在右缘时左侧仍有
/// 约 300 px 舞台可见——「拖滑杆立刻看到舞台反应」这个唯一必须保住的手感还在。
class InlineSettingsDock extends StatelessWidget {
  const InlineSettingsDock({
    required this.child,
    required this.onDismiss,
    super.key,
  });

  final Widget child;
  final VoidCallback onDismiss;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.centerRight,
      child: Padding(
        padding: const EdgeInsets.all(Space.s3),
        child: SizedBox(
          width: NavMetrics.paneWidth,
          child: CallbackShortcuts(
            bindings: <ShortcutActivator, VoidCallback>{
              // Esc 关闭。浮层由 `showModalBottomSheet` 自带；内联侧板要自己接。
              const SingleActivator(LogicalKeyboardKey.escape): onDismiss,
            },
            child: Focus(
              autofocus: true,
              child: Material(
                elevation: 8,
                borderRadius: BorderRadius.circular(AppRadius.lg),
                clipBehavior: Clip.antiAlias,
                child: child,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
