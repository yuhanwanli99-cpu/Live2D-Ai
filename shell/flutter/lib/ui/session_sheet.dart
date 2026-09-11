/// 会话列表（底部浮层）：切换 / 重命名 / 删除 / 新建。
///
/// # 两条刻意的设计取舍
///
/// **① 重命名是「行内改成输入框」，不是弹对话框。**
///
/// 浮层本身已经是 overlay；在它上面再叠一个 `showDialog` 会开**第二个**
/// overlay entry，而那一层**不在** [StagePointerInterceptor] 的罩子内
/// （垫层只罩它包起来的那个盒子，见该文件的头注）——于是重命名对话框
/// 压在舞台上的部分会**看得见、点不着**，正是 2026-09-11 修过的那类 bug。
/// 行内编辑完全不新增 overlay，也就完全绕开了这个问题。
///
/// **② 不用 `PopupMenuButton`。** 同上：弹出菜单是新的 overlay entry。
library;

import 'package:flutter/material.dart';

import '../chat/chat_session.dart';
import '../design/tokens.dart';
import '../live2d/stage_pointer_interceptor.dart';
import 'theme.dart';

/// 打开会话浮层。
Future<void> showSessionSheet({
  required BuildContext context,
  required List<ChatSession> sessions,
  required String? activeId,
  required VoidCallback onNew,
  required ValueChanged<String> onSelect,
  required void Function(String id, String title) onRename,
  required ValueChanged<String> onDelete,
}) => showModalBottomSheet<void>(
  context: context,
  showDragHandle: true,
  isScrollControlled: true,
  constraints: BoxConstraints(
    maxWidth: 520,
    maxHeight: MediaQuery.sizeOf(context).height * 0.72,
  ),
  // 浮层压在舞台下半部 ⇒ 必须垫指针垫层，否则整列条目点不着
  //（与设置抽屉同一处根因，见 `StagePointerInterceptor` 头注）。
  builder: (BuildContext sheetContext) => StagePointerInterceptor(
    child: SafeArea(
      child: SessionSheet(
        sessions: sessions,
        activeId: activeId,
        onNew: onNew,
        onSelect: onSelect,
        onRename: onRename,
        onDelete: onDelete,
        // 用**浮层自己的** context 关掉自己（与设置浮层同一条理由：
        // 用外壳的 context 去 maybePop 依赖「浮层恰好是栈顶」这个巧合）。
        onClose: () => Navigator.of(sheetContext).pop(),
      ),
    ),
  ),
);

/// 会话列表本体（**不依赖 `showModalBottomSheet`**，可在 widget 测试里直接挂）。
class SessionSheet extends StatefulWidget {
  const SessionSheet({
    required this.sessions,
    required this.activeId,
    required this.onNew,
    required this.onSelect,
    required this.onRename,
    required this.onDelete,
    this.onClose,
    super.key,
  });

  /// 会话列表（已按最近使用排序由调用方决定；本组件按传入顺序渲染）。
  final List<ChatSession> sessions;
  final String? activeId;

  final VoidCallback onNew;
  final ValueChanged<String> onSelect;
  final void Function(String id, String title) onRename;
  final ValueChanged<String> onDelete;

  /// 选中/新建之后关闭浮层；`null` 时不自关（测试里用）。
  final VoidCallback? onClose;

  @override
  State<SessionSheet> createState() => _SessionSheetState();
}

class _SessionSheetState extends State<SessionSheet> {
  /// 正在重命名的会话 id（`null` = 没有）。
  String? _editingId;

  /// 重命名输入框的内容。
  late final TextEditingController _titleInput = TextEditingController();

  /// 待确认删除的会话 id（**两段式删除**）。
  ///
  /// 删除是不可逆的，而会话列表里点错一行的代价是「一段聊天没了」。
  /// 不用对话框（见文件头注），改成「点一次变成『确认删除』，再点才真删」。
  String? _confirmingDeleteId;

  @override
  void dispose() {
    _titleInput.dispose();
    super.dispose();
  }

  void _startRename(ChatSession session) {
    setState(() {
      _editingId = session.id;
      _confirmingDeleteId = null;
      // 起过名就填用户的名字；没起过就填空——空串提交等于「恢复自动标题」。
      _titleInput.text = session.title;
      _titleInput.selection = TextSelection(
        baseOffset: 0,
        extentOffset: _titleInput.text.length,
      );
    });
  }

  void _commitRename(String id) {
    widget.onRename(id, _titleInput.text);
    setState(() => _editingId = null);
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // ── 标题行：说明 + 「新建」 ──
        Padding(
          padding: const EdgeInsets.fromLTRB(Space.s4, Space.s1, Space.s2, Space.s2),
          child: Row(
            children: <Widget>[
              Expanded(
                child: Text('会话', style: theme.textTheme.titleSmall),
              ),
              // 文字按钮（用户裁决「尽量少用图片用文字做按钮」）。
              TextButton(
                onPressed: () {
                  widget.onNew();
                  widget.onClose?.call();
                },
                child: const Text('新建'),
              ),
            ],
          ),
        ),
        Divider(height: 1, color: colors.hairline),
        if (widget.sessions.isEmpty)
          Padding(
            padding: const EdgeInsets.all(Space.s5),
            child: Text(
              '还没有会话。发一条消息，或点上面的「新建」。',
              textAlign: TextAlign.center,
              style: theme.textTheme.bodySmall?.copyWith(
                color: colors.contentMuted,
              ),
            ),
          )
        else
          Flexible(
            child: ListView.builder(
              shrinkWrap: true,
              itemCount: widget.sessions.length,
              itemBuilder: (BuildContext context, int index) {
                final ChatSession session = widget.sessions[index];
                return _row(session, theme, colors);
              },
            ),
          ),
      ],
    );
  }

  Widget _row(ChatSession session, ThemeData theme, AppColors colors) {
    final bool isActive = session.id == widget.activeId;

    // ── 重命名中：整行变成输入框 ──
    if (_editingId == session.id) {
      return Padding(
        padding: const EdgeInsets.fromLTRB(Space.s3, Space.s2, Space.s3, Space.s2),
        child: Row(
          children: <Widget>[
            Expanded(
              child: TextField(
                controller: _titleInput,
                autofocus: true,
                textInputAction: TextInputAction.done,
                onSubmitted: (_) => _commitRename(session.id),
                decoration: const InputDecoration(
                  isDense: true,
                  hintText: '留空则用第一条消息作标题',
                ),
              ),
            ),
            const SizedBox(width: Space.s2),
            TextButton(
              onPressed: () => _commitRename(session.id),
              child: const Text('确定'),
            ),
            TextButton(
              onPressed: () => setState(() => _editingId = null),
              child: const Text('取消'),
            ),
          ],
        ),
      );
    }

    // ── 待确认删除 ──
    if (_confirmingDeleteId == session.id) {
      return ListTile(
        title: Text('删除「${session.displayTitle}」？'),
        subtitle: Text(
          '${session.messages.length} 条消息，删除后无法恢复',
          style: theme.textTheme.bodySmall?.copyWith(color: colors.contentMuted),
        ),
        trailing: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            TextButton(
              onPressed: () => setState(() => _confirmingDeleteId = null),
              child: const Text('取消'),
            ),
            TextButton(
              onPressed: () {
                widget.onDelete(session.id);
                setState(() => _confirmingDeleteId = null);
                // 删掉的正好是当前会话时，列表内容已经变了；
                // 不自动关浮层——用户可能还要继续删。
              },
              style: TextButton.styleFrom(
                foregroundColor: appPaletteOf(context).danger,
              ),
              child: const Text('删除'),
            ),
          ],
        ),
      );
    }

    // ── 常态 ──
    return ListTile(
      title: Text(
        session.displayTitle,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: Text(
        _subtitle(session),
        style: theme.textTheme.bodySmall?.copyWith(color: colors.contentMuted),
      ),
      selected: isActive,
      onTap: () {
        widget.onSelect(session.id);
        widget.onClose?.call();
      },
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          TextButton(
            onPressed: () => _startRename(session),
            child: const Text('改名'),
          ),
          TextButton(
            onPressed: () => setState(() {
              _confirmingDeleteId = session.id;
              _editingId = null;
            }),
            child: const Text('删除'),
          ),
        ],
      ),
    );
  }

  /// 副标题：条数 + 最后活动时间。
  String _subtitle(ChatSession session) {
    final int n = session.messages.length;
    final DateTime? at = session.lastMessageAt;
    if (n == 0 || at == null) return '空会话';
    return '$n 条 · ${relativeTime(at, now: DateTime.now())}';
  }
}
