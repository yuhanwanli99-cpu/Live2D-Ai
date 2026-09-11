/// 设置导航宿主：**同一个 `SettingsScaffold`，只换宿主**（规格 §4.2）。
///
/// | 断点 | 宿主 |
/// |---|---|
/// | ≥1280 expanded | 内联侧板（400 px，浮在舞台列右缘） |
/// | 900–1279 medium | `showModalBottomSheet(isScrollControlled, maxWidth 560, maxHeight 0.78h)` |
/// | <900 compact | 抽屉列 8 项 → 选中后再弹 sheet（严格 2 层） |
///
/// 「Esc / 点空白关闭」两条**只在浮层成立**（`showModalBottomSheet` 自带 Esc；
/// 障碍层在舞台区域点不着，见 `StagePointerInterceptor` 头注）；内联侧板只有
/// **✕ / Esc**，因为左侧 rail 已删（2026-09-11 用户裁决）。
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
import '../live2d/stage_pointer_interceptor.dart';
import '../ui/glass_rim.dart';
import '../ui/theme.dart';
import 'collapsible_panel.dart';

/// 浮层的宽度/高度（**唯一定义点**，宿主与测试都读这里）。
abstract final class NavMetrics {
  /// 内联侧板宽度（expanded）。
  static const double paneWidth = 400;

  /// 浮层最大宽度（只有 medium 用到浮层）。
  static const double sheetMaxWidthMedium = 560;

  /// 浮层最大高度比例。
  static const double sheetHeightFactorMedium = 0.78;

  /// ~~左侧分区导航的宽度~~（**2026-09-11 已删除：用户裁决「把左边的这些设置
  /// 一级选项去掉留给舞台」**）。
  ///
  /// 分区导航现在只有**面板内部**那一行 chip（`SettingsScaffold`），
  /// 所以不再需要第二套宽度常量。留这条注释是为了让「以前有过 rail、
  /// 为什么没了」可被检索到——不要把它当占位值重新加回来。

  /// 常驻聊天面板宽度（expanded）。
  static const double chatWidthExpanded = 340;

  /// 常驻聊天面板宽度（medium）。
  static const double chatWidthMedium = 320;

  /// 舞台列最小宽度。
  static const double stageMinWidth = 480;
}

/// 设置宿主形态（由断点决定，**纯函数**，可单测）。
///
/// | 断点 | 宿主 | 为什么 |
/// | --- | --- | --- |
/// | expanded | [inline] | 侧板浮在舞台列右缘，左边仍能看到舞台（拖滑杆立刻看反应） |
/// | medium | [sheet] | 舞台与聊天已并排，剩下给设置的空间不够放侧板 |
/// | compact | [page] | 见下 |
///
/// # compact 为什么是「整页过渡」而不是「抽屉 + 浮层」（2026-09-11，P2-1）
///
/// 旧做法是**两步跳**：弹一个 8 项抽屉选分区，再弹底部浮层显示内容。
/// 观感上像「弹了两次」，而且抽屉选完就消失、用户还要再等一次动画。
///
/// 现在 compact 直接把整个工作台**页内换掉**（[PageCrossFade]）：
/// 一次过渡到位，分区切换用面板内那一行 chip（宽窄本来就共用同一套）。
/// 顺带把「最多 2 层披露」变成 **1 层**——这比原来更符合渐进披露的要求。
///
/// 舞台仍然保活：`PageCrossFade` 用 `Offstage`（不 paint，但 Element 还在）。
enum SettingsHost {
  /// 内联侧板（浮在舞台列之上，不占列宽）。
  inline,

  /// 页内整页切换（compact）。
  page,

  /// 底部浮层（medium）。
  sheet,
}

/// 断点 → 宿主。
SettingsHost settingsHostOf(SizeClass sizeClass) {
  if (sizeClass.hasInlineSettings) return SettingsHost.inline;
  if (sizeClass.isCompact) return SettingsHost.page;
  return SettingsHost.sheet;
}

/// 浮层约束。
///
/// **只有 medium 会调它**（2026-09-11，P2-1）：compact 改成整页过渡之后，
/// `SettingsHost.sheet` 只剩 medium 一档。所以这里不再按断点分叉——
/// 留一个「compact 更窄更矮」的分支只会变成没人走、却要跟着维护的死代码
///（P0 刚删掉一整批同类的）。
BoxConstraints sheetConstraintsOf(Size viewport) => BoxConstraints(
  maxWidth: NavMetrics.sheetMaxWidthMedium,
  maxHeight: viewport.height * NavMetrics.sheetHeightFactorMedium,
);

/// 打开设置浮层（只有 medium 走这条路）。
///
/// `showModalBottomSheet` **自带焦点陷阱 + Esc**，且 **overlay 不卸载下层**
/// ⇒ 舞台 iframe 不会被重建（规格 §3.4 的保活约束之一）。
Future<void> showSettingsSheet({
  required BuildContext context,
  required WidgetBuilder builder,
}) => showModalBottomSheet<void>(
  context: context,
  showDragHandle: true,
  isScrollControlled: true,
  constraints: sheetConstraintsOf(MediaQuery.sizeOf(context)),
  builder: builder,
);

/// 设置侧板（expanded 的内联浮层）。
///
/// 铺在舞台列**之上**（不是新增一列）：1280 下「400 设置 + 340 聊天」会与舞台
/// 分宽度，而舞台是产品核心（原则 P1）。浮在右缘时左侧仍有约 540 px 舞台可见
/// ——「拖滑杆立刻看到舞台反应」这个唯一必须保住的手感还在。
/// （2026-09-11 删掉左侧 168 px 的 rail 之后，这里的可见舞台从约 370 px 涨到约 540 px。）
///
/// # 收起是**折叠**，不是卸载（2026-09-11，P1-1）
///
/// 过去 `app_shell` 写的是 `if (settingsOpen && inlineSettings) Positioned.fill(...)`：
/// 关一次设置 = 面板整棵子树重建 = 滚动位置回到顶部、分区草稿与内联测试结果
/// 全没了。现在侧板**常驻在树里**，用 [CollapsiblePanel] 把宽度折到 0——
/// 再打开时还在原来的位置。回归：`test/settings_panel_keepalive_test.dart`。
///
/// 代价是那层指针垫层也会留着，所以它跟着 [expanded] 一起 `enabled`——
/// 否则收起之后舞台右缘那一条会**永远拖不动模型**。
class InlineSettingsDock extends StatelessWidget {
  const InlineSettingsDock({
    required this.child,
    required this.onDismiss,
    this.expanded = true,
    super.key,
  });

  final Widget child;
  final VoidCallback onDismiss;

  /// `false` = 折叠到 0 宽（但不卸载）。
  final bool expanded;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.centerRight,
      child: Padding(
        padding: const EdgeInsets.all(Space.s3),
        child: CollapsiblePanel(
          expanded: expanded,
          axis: Axis.horizontal,
          extent: NavMetrics.paneWidth,
          child: CallbackShortcuts(
            bindings: <ShortcutActivator, VoidCallback>{
              // Esc 关闭。浮层由 `showModalBottomSheet` 自带；内联侧板要自己接。
              const SingleActivator(LogicalKeyboardKey.escape): onDismiss,
            },
            // `autofocus` 只在展开时才有意义：收起的子树被 `CollapsiblePanel`
            // 的 `ExcludeFocus` 挡住，不会抢焦点。
            child: Focus(
              autofocus: true,
              // **不用 `Material(elevation: 8)`**：Material 的 elevation 会投一层
              // 弥散阴影到舞台上，看起来像「贴了一张卡片」。改成实色面 + 1 px
              // 描边——与本项目所有其它容器同一套层级表达（`GlassPanel`）。
              //
              // 侧板整块压在舞台列上 ⇒ **必须**垫指针垫层：没有它，整块设置
              // 面板都是「看得见、点不着、滑不动」（2026-09-11 用户报告的正是它）。
              child: StagePointerInterceptor(
                enabled: expanded,
                // 一圈**跟着指针走的边缘高光**（P3-1）。它只画描边、
                // 不模糊也不采样，所以压在舞台 iframe 上完全安全。
                child: GlassRim(
                  borderRadius: BorderRadius.circular(AppRadius.xl),
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: Theme.of(context).colorScheme.surface,
                      borderRadius: BorderRadius.circular(AppRadius.xl),
                      border: Border.all(color: appColorsOf(context).hairline),
                    ),
                    child: ClipRRect(
                      borderRadius: BorderRadius.circular(AppRadius.xl),
                      child: child,
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
