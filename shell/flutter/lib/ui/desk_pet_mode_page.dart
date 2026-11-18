/// 桌宠模式页（2026-11-18）：网页工作台在切桌宠后**不丢掉会话**。
///
/// # 审计现场
///
/// 切桌宠后整棵 `AppShell` 被换成一句「已进入桌宠，请关闭此页」：会话列表、
/// 当前标题、输入框一起卸掉，宠物右键菜单里也没有会话——这段时间不能看、也不能
/// 切换会话。而浏览器那页通常还开着（`window.close()` 对非脚本打开的窗口无效），
/// 用户就停在一句占位文案上。
///
/// # 这一页的边界
///
/// - **不重建对话**：会话数据在 `ChatSessionStore`（宿主 State 里），本页只是
///   同一份数据的第二个视图，切换 / 新建走宿主同一条回调；
/// - **不叠在舞台 iframe 上**：切桌宠时外壳整棵离树（避免网页与宠物窗两只皮套同时
///   跑），所以本页是独立页，不是浮层；
/// - **回工作台要顺带关宠物窗**（宿主做）：否则回去以后又是两只皮套。
///
/// 会话标题、条数、时间都由宿主算好传进来（本页不读 `DateTime.now()`——那样
/// widget 测试会随真实时钟漂移）。
library;

import 'package:flutter/material.dart';

import '../chat/chat_session.dart';
import '../design/tokens.dart';
import 'theme.dart';

class DeskPetModePage extends StatelessWidget {
  const DeskPetModePage({
    required this.sessions,
    required this.activeId,
    required this.activeTitle,
    required this.activeMeta,
    required this.onSelect,
    required this.onNew,
    required this.onBack,
    super.key,
  });

  /// 已按最近使用排序（宿主传 `byRecency`）。
  final List<ChatSession> sessions;
  final String? activeId;

  /// 当前会话标题 / 副标题（「N 条 · 5 分钟前」）；没有会话时为空。
  final String? activeTitle;
  final String? activeMeta;

  final ValueChanged<String> onSelect;
  final VoidCallback onNew;

  /// 回网页工作台（宿主负责顺带关掉宠物窗）。
  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    return Scaffold(
      // 与工作台同一张底：切过去不是「进了一个新应用」，只是同一件事换了一页。
      backgroundColor: appPaletteOf(context).stage,
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(Space.s4),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  Text('已进入桌宠', style: theme.textTheme.titleLarge),
                  const SizedBox(height: Space.s2),
                  Text(
                    '网页工作台暂停显示，对话还在后台；桌宠是另一个窗口。',
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: colors.contentMuted,
                    ),
                  ),
                  const SizedBox(height: Space.s4),
                  // ── 当前会话：这一页存在的理由就是它不能丢 ──
                  Container(
                    padding: const EdgeInsets.all(Space.s3),
                    decoration: BoxDecoration(
                      color: appPaletteOf(context).raised,
                      borderRadius: BorderRadius.circular(AppRadius.lg),
                      border: Border.all(color: colors.hairline),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          '当前会话',
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: colors.contentMuted,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          activeTitle ?? kUntitledSessionLabel,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.titleSmall,
                        ),
                        if (activeMeta != null) ...<Widget>[
                          const SizedBox(height: 2),
                          Text(
                            activeMeta!,
                            style: theme.textTheme.bodySmall?.copyWith(
                              color: colors.contentMuted,
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                  const SizedBox(height: Space.s4),
                  Text('切换会话', style: theme.textTheme.titleSmall),
                  const SizedBox(height: Space.s1),
                  if (sessions.isEmpty)
                    Text(
                      '还没有会话。',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: colors.contentMuted,
                      ),
                    )
                  else
                    // 高度封顶：会话多时本页仍能一屏放下「回工作台」。
                    ConstrainedBox(
                      constraints: const BoxConstraints(maxHeight: 220),
                      child: ListView(
                        shrinkWrap: true,
                        children: <Widget>[
                          for (final ChatSession s in sessions)
                            ListTile(
                              dense: true,
                              selected: s.id == activeId,
                              title: Text(
                                s.displayTitle,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                              subtitle: Text(
                                s.messages.isEmpty
                                    ? '空会话'
                                    : '${s.messages.length} 条',
                                style: theme.textTheme.bodySmall?.copyWith(
                                  color: colors.contentMuted,
                                ),
                              ),
                              onTap: () => onSelect(s.id),
                            ),
                        ],
                      ),
                    ),
                  const SizedBox(height: Space.s4),
                  Row(
                    children: <Widget>[
                      TextButton(onPressed: onNew, child: const Text('新建会话')),
                      const Spacer(),
                      FilledButton(
                        onPressed: onBack,
                        child: const Text('返回工作台'),
                      ),
                    ],
                  ),
                  const SizedBox(height: Space.s2),
                  Text(
                    '桌宠窗口右键菜单的「回到 web 版本」也会关掉它并打开网页。',
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: colors.contentMuted,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
