/// 全局快捷键（规格 §9.3）。
///
/// 用 `CallbackShortcuts` 挂在根：最省样板，且**不吞**未匹配的键
/// （`Shortcuts` + `Actions` 那套要写一堆 `Intent`/`Action` 配对）。
///
/// # 为什么必须显式补 `Ctrl/Cmd + Enter`
///
/// `TextField` 的 `TextInputAction.send` 在 `maxLines > 1` 时 Enter 是**换行**，
/// 不会触发 `onSubmitted`。聊天输入框是多行的（最多 4 行），所以
/// 「Enter 发送」在多行输入框里其实并不成立——必须补显式快捷键。
///
/// # Esc 的优先级
///
/// 顺序是**模态优先**：模态弹层（`showModalBottomSheet` / `showDialog`）自带
/// Esc 处理，它先把键消费掉，我们的回调根本不会跑——这是对的。
/// **不要**为了「统一处理」去覆盖它，否则会出现「按一次 Esc 关了两层」。
/// 只有当模态不在最上层时，Esc 才轮到「关内联侧板 → 否则停止本轮」。
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

/// 快捷键帮助里的一条（**唯一文案来源**：帮助弹层与测试都读它）。
class ShortcutHelpEntry {
  const ShortcutHelpEntry({
    required this.keys,
    required this.action,
    this.note,
  });

  /// 展示用的按键文本。
  final String keys;
  final String action;
  final String? note;
}

/// 按平台给按键前缀（macOS → `Cmd`，其余 → `Ctrl`）。
///
/// **用 ASCII 的 `Cmd` 而不是 `⌘`（U+2318）**（2026-09-11）：那个符号不在
/// 自托管的中文子集里，一旦上屏就会让 CanvasKit 去 `fonts.gstatic.com` 拉
/// 回退字体——踩中「断网即豆腐块」的硬约束（同批修掉的还有流式光标 `▍`）。
/// `Cmd` 在任何字体里都有，代价只是不够「原生」。
String shortcutModifierLabel({required bool isMacOS}) => isMacOS ? 'Cmd' : 'Ctrl';

/// 快捷键清单。
List<ShortcutHelpEntry> shortcutHelp({required bool isMacOS}) {
  final String mod = shortcutModifierLabel(isMacOS: isMacOS);
  return <ShortcutHelpEntry>[
    ShortcutHelpEntry(
      keys: '$mod + Enter',
      action: '发送消息',
      note: '多行输入框里 Enter 是换行，所以要用组合键',
    ),
    const ShortcutHelpEntry(
      keys: 'Esc',
      action: '关闭侧板 / 停止本轮',
      note: '模态弹层优先（它自带 Esc）',
    ),
    ShortcutHelpEntry(keys: '$mod + ,', action: '打开设置'),
    ShortcutHelpEntry(keys: '$mod + /', action: '快捷键帮助'),
  ];
}

/// 快捷键动作的回调集合。
///
/// 传 `null` 的动作**不注册**该键（而不是注册一个空回调）——
/// 后者会让键被静默吞掉。例如「没有可停止的一轮」时 Esc 什么都不该做，
/// 但也不该阻止事件继续传播。
@immutable
class AppShortcutCallbacks {
  const AppShortcutCallbacks({
    required this.isMacOS,
    this.onSend,
    this.onStop,
    this.onDismissOverlay,
    this.onOpenSettings,
    this.onHelp,
  });

  /// macOS 用 `meta`（⌘），其余用 `control`。
  ///
  /// **不要两个都绑**：在 Windows 上 `meta` 是 Win 键，绑上去会出现
  /// 「按 Win+Enter 发消息」这种事。
  final bool isMacOS;

  final VoidCallback? onSend;

  /// 停止本轮（Esc 的第二优先级）。
  final VoidCallback? onStop;

  /// 关闭当前**非模态**覆盖层（内联侧板 / 抽屉）。
  ///
  /// 返回 `true` 表示它确实关掉了 → 不再触发 [onStop]。
  /// 不区分的话一次 Esc 会「关侧板 + 停止对话」两件事一起做。
  final bool Function()? onDismissOverlay;

  final VoidCallback? onOpenSettings;
  final VoidCallback? onHelp;

  /// 组装成 `CallbackShortcuts` 要的绑定表。
  Map<ShortcutActivator, VoidCallback> bindings() {
    final Map<ShortcutActivator, VoidCallback> out =
        <ShortcutActivator, VoidCallback>{};

    void bindModifier(LogicalKeyboardKey key, VoidCallback action) {
      out[SingleActivator(key, control: !isMacOS, meta: isMacOS)] = action;
    }

    if (onSend != null) {
      bindModifier(LogicalKeyboardKey.enter, onSend!);
      // 小键盘 Enter 也要能用（很多人用数字键盘发消息）。
      bindModifier(LogicalKeyboardKey.numpadEnter, onSend!);
    }
    if (onOpenSettings != null) {
      bindModifier(LogicalKeyboardKey.comma, onOpenSettings!);
    }
    if (onHelp != null) {
      bindModifier(LogicalKeyboardKey.slash, onHelp!);
    }
    if (onDismissOverlay != null || onStop != null) {
      out[const SingleActivator(LogicalKeyboardKey.escape)] = () {
        // 覆盖层优先：它关掉了就不再停止本轮。
        if (onDismissOverlay?.call() ?? false) return;
        onStop?.call();
      };
    }
    return out;
  }
}
