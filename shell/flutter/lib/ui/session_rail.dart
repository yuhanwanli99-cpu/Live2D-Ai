/// 左侧会话栏（2026-11-18）：常驻在舞台左边、**不是浮层**。
///
/// # 为什么要有它（对应审计「宽屏上会话信息只挂在顶栏两个字上」）
///
/// 2026-09-11 删掉左侧**设置** rail 是对的（用户裁决：宽度还给舞台），但那一刀
/// 顺带让「当前是哪一段对话」失去了常驻落点：宽屏只有顶栏一个「会话」按钮，
/// 设置侧板一收起，聊天列里既没有标题也没有条数——用户看不出自己在哪段对话里。
///
/// 本栏只承担**会话导航**，不承担设置导航（设置仍是面板内的 chip 行），因此
/// 它与那条被删的 `SectionRail` 不是同一件事：不新增设置的第二入口。
///
/// # 两种宽度都是**布局宽度**，不是浮层
///
/// - expanded：248 px 的列表栏（可以「收起」）；
/// - 收起后（以及 medium）：52 px 的窄轨，**留下当前会话名字**（竖排、可读）。
///
/// 窄轨必须留在布局里而不是变成抽屉：它一旦是浮层，收起时就又回到「什么都看不
/// 见」的起点，而且浮层压在舞台上要垫指针垫层（见 `StagePointerInterceptor`）。
///
/// medium（900–1279）只给窄轨：那一档「248 + 舞台 + 320 聊天」会把舞台挤到
/// 480 px 的 `NavMetrics.stageMinWidth` 以下。窄轨 52 px 是能同时满足舞台下限的
/// 最大宽度。medium 上点窄轨 = 打开会话浮层（唯一的管理入口仍在浮层里）。
///
/// # 为什么列表不在这里做改名 / 删除
///
/// 行内改名与两段式删除已经有一份实现（`ui/session_sheet.dart`），而且它的两条
/// 设计约束（不新开 overlay、不弹对话框）都是为「压在舞台上」准备的。外壳的会话
/// 栏离舞台只有 1 px 分隔线，再写第二套编辑交互只会长出两套不一致的语义；这里
/// 只做「切换 / 新建 / 打开完整列表」。
library;

import 'package:flutter/material.dart';

import '../chat/chat_session.dart';
import '../design/breakpoints.dart';
import '../design/tokens.dart';
import 'theme.dart';

/// 展开态宽度（只有 expanded 会用到）。
const double kSessionRailExpandedWidth = 248;

/// 收起态 / medium 的窄轨宽度。
///
/// 取值理由：52 = 48（Material 最小可点面积）+ 4 的呼吸；再宽就把 medium 的
/// 舞台压到 `NavMetrics.stageMinWidth` 以下了（900 - 52 - 320 - 2 = 526）。
const double kSessionRailCollapsedWidth = 52;

/// 会话栏在某个断点 / 状态下占的**布局宽度**（0 = 不渲染）。
///
/// 纯函数：外壳与测试都读它，避免「宽度写死在两个地方」。
double sessionRailWidthFor(SizeClass sizeClass, {required bool expanded}) {
  if (sizeClass.isCompact) return 0;
  if (expanded && sizeClass.isExpanded) return kSessionRailExpandedWidth;
  return kSessionRailCollapsedWidth;
}

/// 左侧会话栏。
class SessionRail extends StatelessWidget {
  const SessionRail({
    required this.sessions,
    required this.activeId,
    required this.expanded,
    required this.canExpand,
    required this.onToggle,
    required this.onSelect,
    required this.onNew,
    required this.onManageSessions,
    super.key,
  });

  /// 已按最近使用排序（外壳传 `byRecency`）。
  final List<ChatSession> sessions;
  final String? activeId;

  /// `true` = 248 px 列表；`false` = 52 px 窄轨。
  final bool expanded;

  /// expanded 才允许在栏内展开 / 收起；medium 永远窄轨（舞台宽度不够）。
  final bool canExpand;

  final VoidCallback onToggle;
  final ValueChanged<String> onSelect;
  final VoidCallback onNew;

  /// 打开完整会话浮层（改名 / 删除 / 新建仍在那里）。
  final VoidCallback onManageSessions;

  ChatSession? get _active {
    for (final ChatSession s in sessions) {
      if (s.id == activeId) return s;
    }
    return null;
  }

  /// 窄轨上留下的那一行：**当前会话自己起的名字**（没起名就用自动标题）。
  String get currentTitle =>
      _active?.displayTitle ?? (sessions.isEmpty ? '还没有会话' : kUntitledSessionLabel);

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    return Semantics(
      container: true,
      label: '会话栏',
      child: SizedBox(
        width: expanded ? kSessionRailExpandedWidth : kSessionRailCollapsedWidth,
        child: DecoratedBox(
          // 与聊天列同一张面：两栏都是「工作台的一部分」，不是浮在舞台上的卡片。
          decoration: BoxDecoration(
            color: appPaletteOf(context).surface,
            border: Border(right: BorderSide(color: colors.hairline)),
          ),
          child: expanded ? _buildExpanded(context) : _buildCollapsed(context),
        ),
      ),
    );
  }

  Widget _buildExpanded(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.only(
            left: Space.s3,
            top: Space.s2,
            right: Space.s2,
          ),
          child: Row(
            children: <Widget>[
              Expanded(
                child: Text('最近会话', style: theme.textTheme.titleSmall),
              ),
              if (canExpand)
                TextButton(
                  onPressed: onToggle,
                  style: TextButton.styleFrom(
                    visualDensity: VisualDensity.compact,
                    minimumSize: const Size(48, 32),
                  ),
                  child: const Text('收起'),
                ),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.only(
            left: Space.s2,
            right: Space.s2,
            bottom: Space.s1,
          ),
          child: Row(
            children: <Widget>[
              TextButton(
                onPressed: onNew,
                style: TextButton.styleFrom(
                  visualDensity: VisualDensity.compact,
                  minimumSize: const Size(48, 32),
                ),
                child: const Text('新建'),
              ),
              TextButton(
                onPressed: onManageSessions,
                style: TextButton.styleFrom(
                  visualDensity: VisualDensity.compact,
                  minimumSize: const Size(48, 32),
                ),
                child: const Text('全部会话'),
              ),
            ],
          ),
        ),
        Divider(height: 1, color: colors.hairline),
        Expanded(
          child: sessions.isEmpty
              ? Padding(
                  padding: const EdgeInsets.all(Space.s3),
                  child: Text(
                    '还没有会话。\n发一条消息或点「新建」。',
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: colors.contentMuted,
                    ),
                  ),
                )
              : ListView.builder(
                  padding: const EdgeInsets.symmetric(vertical: Space.s1),
                  itemCount: sessions.length,
                  itemBuilder: (BuildContext context, int index) =>
                      _row(context, sessions[index]),
                ),
        ),
      ],
    );
  }

  Widget _row(BuildContext context, ChatSession session) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final bool isActive = session.id == activeId;
    final int n = session.messages.length;
    final DateTime? at = session.lastMessageAt;
    final String meta = (n == 0 || at == null)
        ? '空会话'
        : '$n 条 · ${relativeTime(at, now: DateTime.now())}';
    return Semantics(
      selected: isActive,
      button: true,
      label: '${session.displayTitle}，$meta',
      child: InkWell(
        onTap: () => onSelect(session.id),
        child: Container(
          color: isActive
              ? appPaletteOf(context).accent.withValues(alpha: 0.12)
              : null,
          padding: const EdgeInsets.fromLTRB(
            Space.s3,
            Space.s2,
            Space.s2,
            Space.s2,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                session.displayTitle,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodyMedium?.copyWith(
                  fontWeight: isActive ? FontWeight.w600 : FontWeight.w400,
                ),
              ),
              const SizedBox(height: 2),
              Text(
                meta,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: colors.contentMuted,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildCollapsed(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final String title = currentTitle;
    return Column(
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.only(top: Space.s1),
          child: Tooltip(
            message: canExpand ? '展开会话栏' : '打开会话列表',
            child: IconButton(
              onPressed: canExpand ? onToggle : onManageSessions,
              icon: Icon(
                canExpand ? Icons.chevron_right_rounded : Icons.list_rounded,
                size: 20,
              ),
            ),
          ),
        ),
        Divider(height: 1, color: colors.hairline),
        // 窄轨的**唯一职责**：让人一眼看出「现在在哪一段对话里」。
        // 竖排（`quarterTurns: 3`）是因为 52 px 放不下一行汉字；名字再长也只占
        // 一行、超出省略，完整的名字在 tooltip / 会话列表里。
        Expanded(
          child: Tooltip(
            message: '当前会话：$title',
            child: Semantics(
              label: '当前会话：$title',
              child: Center(
                child: RotatedBox(
                  quarterTurns: 3,
                  child: Text(
                    title,
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: colors.contentMuted,
                      letterSpacing: 1,
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
        IconButton(
          onPressed: onNew,
          tooltip: '新建会话',
          icon: const Icon(Icons.add_rounded, size: 20),
        ),
        const SizedBox(height: Space.s2),
      ],
    );
  }
}
