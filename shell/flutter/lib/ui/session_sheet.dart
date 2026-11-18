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

/// 会话列表顶部那行说明（2026-09-13 立；2026-11-18 #14 改写）。
///
/// 它必须说清两件事，否则用户会误读：
/// ① 这些会话只在本机（localStorage）——换设备 / 清站点数据就没了；
/// ② 模型看到的是**这一段的最近几轮**：每轮请求会把本会话最近 8 轮历史一起
///    发给服务端（`POST /api/v1/chat` 的可选 `history`），服务端不保存会话。
///    所以「换会话 = 换上下文」是真的，「跨设备还记得」是假的。
///
/// 与 `chat/chat_session.dart` 的模型说明同源，放在 UI 侧只因为它是**给人看的文案**。
const String kSessionsAreLocalNote =
    '这些会话只存在本机；每轮会把本会话最近几轮一起发给模型，换会话就是换上下文。';

/// 打开会话浮层。
///
/// # 为什么读的是回调 + [Listenable]，不是一份快照（2026-10-08 修）
///
/// `showModalBottomSheet` 的 builder **只跑一次**：从前把 `sessions.byRecency`
/// 与 `activeId` 抄进闭包，`ChatSessionStore.delete` 改了 store 也通知不了浮层
/// ——于是「删掉一行，行还在」（新建 / 重命名开着浮层时同样是旧的）。
///
/// 现在 builder 内部包一层 [ListenableBuilder]：每次 store 变动都重跑 builder，
/// 且每次都**现读** [sessionsOf] / [activeIdOf]。删掉的行立刻消失，浮层不关。
Future<void> showSessionSheet({
  required BuildContext context,
  required Listenable listenable,
  required List<ChatSession> Function() sessionsOf,
  required String? Function() activeIdOf,
  required VoidCallback onNew,
  required ValueChanged<String> onSelect,
  required void Function(String id, String title) onRename,
  required ValueChanged<String> onDelete,

  /// 存档裁剪提示（`ChatSessionStore.trimNotice`，#8）。
  String? trimNotice,

  /// 读盘警告（`ChatSessionStore.loadWarning`，#10）。
  String? loadWarning,

  /// 落盘失败（`ChatSessionStore.saveFailed`，#9）：显示「只保留在本次页面里」。
  bool saveFailed = false,
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
  builder: (BuildContext sheetContext) => ListenableBuilder(
    listenable: listenable,
    builder: (BuildContext context, Widget? _) => StagePointerInterceptor(
      child: SafeArea(
        child: SessionSheet(
          sessions: sessionsOf(),
          activeId: activeIdOf(),
          onNew: onNew,
          onSelect: onSelect,
          onRename: onRename,
          onDelete: onDelete,
          trimNotice: trimNotice,
          loadWarning: loadWarning,
          saveFailed: saveFailed,
          // 用**浮层自己的** context 关掉自己（与设置浮层同一条理由：
          // 用外壳的 context 去 maybePop 依赖「浮层恰好是栈顶」这个巧合）。
          onClose: () => Navigator.of(sheetContext).pop(),
        ),
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
    this.trimNotice,
    this.loadWarning,
    this.saveFailed = false,
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

  /// 存档裁剪提示（`ChatSessionStore.trimNotice`）：超上限自动丢了什么（#8）。
  ///
  /// 由组合根从 store 读出来传进来——本组件不认识 store，才能在 widget 测试里
  /// 直接摆出「有提示 / 没提示」两种状态。
  final String? trimNotice;

  /// 读盘警告（`ChatSessionStore.loadWarning`）：有存档条目读不出来（#10）。
  final String? loadWarning;

  /// 落盘失败（`ChatSessionStore.saveFailed`）：无痕模式 / 配额满（#9）。
  ///
  /// 为 `true` 时显示 `kSessionsNotPersistedNote`——用户必须知道「这次页面一关，
  /// 记录就没了」，否则会以为和平时一样能刷新回来。
  final bool saveFailed;

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

  /// 重命名提交后的**行内确认**文案（#13；`null` = 不显示）。
  ///
  /// 只对「清空标题 → 恢复自动标题」显示：那是唯一一种「提交了但看起来像没改」
  /// 的操作。不用 SnackBar / Dialog——新 overlay 在指针垫层外点不着（见文件头）。
  String? _renameNotice;

  /// [_renameNotice] 属于哪个会话（换一行改名时旧提示要清掉）。
  String? _renameNoticeId;

  @override
  void dispose() {
    _titleInput.dispose();
    super.dispose();
  }

  void _startRename(ChatSession session) {
    setState(() {
      _editingId = session.id;
      _confirmingDeleteId = null;
      // 开始新一轮改名：上一条确认文案已经过时。
      _renameNotice = null;
      _renameNoticeId = null;
      // 起过名就填用户的名字；没起过就填空——空串提交等于「恢复自动标题」。
      _titleInput.text = session.title;
      _titleInput.selection = TextSelection(
        baseOffset: 0,
        extentOffset: _titleInput.text.length,
      );
    });
  }

  void _commitRename(String id) {
    final String submitted = _titleInput.text;
    widget.onRename(id, submitted);
    setState(() {
      _editingId = null;
      if (submitted.trim().isEmpty) {
        // 空串 = **恢复自动标题**（语义在 ChatSessionStore.rename）。必须给一句
        // 行内确认：否则「留空提交」与「按了取消」看起来一模一样（#13）。
        _renameNoticeId = id;
        _renameNotice = '已恢复自动标题：${_displayTitleOf(id)}';
      } else {
        _renameNotice = null;
        _renameNoticeId = null;
      }
    });
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
          padding: const EdgeInsets.fromLTRB(
            Space.s4,
            Space.s1,
            Space.s2,
            Space.s2,
          ),
          child: Row(
            children: <Widget>[
              Expanded(child: Text('会话', style: theme.textTheme.titleSmall)),
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
        // ── 「本地记录 ≠ 模型记忆」的明示（rc.3 N3，2026-09-13）──
        //
        // 多会话只存在**本机**（localStorage）；每轮发出去的仍然只有当前这一句
        // （`POST /api/v1/chat` 的 body 是 `{text}`）。不写清楚，用户会以为
        // 「切了会话 = 模型换了记忆」——那是两件事。
        Padding(
          padding: const EdgeInsets.only(
            left: Space.s4,
            top: Space.s2,
            right: Space.s4,
          ),
          child: Text(
            kSessionsAreLocalNote,
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.contentMuted,
            ),
          ),
        ),
        // #8/#9/#10 的提示：只显示非空的那几条（不占地方、不喧宾夺主）。
        ..._noticeLines(theme),
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
        padding: const EdgeInsets.fromLTRB(
          Space.s3,
          Space.s2,
          Space.s3,
          Space.s2,
        ),
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
                  // #13：留空**不是**「没标题」——是「恢复自动标题」。这句说明
                  // 只在编辑态可见，所以必须写进 hint，别指望别处解释。
                  hintText: '留空 = 恢复自动标题（用第一条消息）',
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
        title: Text(
          '删除「${session.displayTitle}」？',
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
        ),
        subtitle: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              '${session.messages.length} 条消息，删除后无法恢复',
              style: theme.textTheme.bodySmall?.copyWith(
                color: colors.contentMuted,
              ),
            ),
            _actionsRow(<Widget>[
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
            ]),
          ],
        ),
      );
    }

    // ── 常态 ──
    //
    // #6（2026-11-18）：两个动作按钮**不能**放在 `ListTile.trailing` 里。
    // trailing 是行内固定宽度，320 px 宽时标题被挤成省略号、按钮组还可能溢出；
    // 而这里不能用 PopupMenuButton（新 overlay 在指针垫层外点不着，见文件头）。
    // 改成把按钮放在 subtitle 的**第二行、右对齐**：标题独占整行，按钮永远有
    // 地方放，窄屏也不会溢出。
    return ListTile(
      title: Text(
        session.displayTitle,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            _subtitle(session),
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.contentMuted,
            ),
          ),
          // #13：重命名提交后的行内确认，留在本行、不新开 overlay。
          if (_renameNoticeId == session.id && _renameNotice != null)
            Padding(
              padding: const EdgeInsets.only(top: Space.s1),
              child: Text(
                _renameNotice!,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: colors.contentMuted,
                ),
              ),
            ),
          _actionsRow(<Widget>[
            TextButton(
              onPressed: () => _startRename(session),
              child: const Text('改名'),
            ),
            TextButton(
              onPressed: () => setState(() {
                _confirmingDeleteId = session.id;
                _editingId = null;
                _renameNotice = null;
                _renameNoticeId = null;
              }),
              child: const Text('删除'),
            ),
          ]),
        ],
      ),
      selected: isActive,
      onTap: () {
        widget.onSelect(session.id);
        widget.onClose?.call();
      },
    );
  }

  /// 行内动作组：**右对齐、独占一行**（#6：不挤标题、不溢出）。
  Widget _actionsRow(List<Widget> children) => Align(
    alignment: Alignment.centerRight,
    child: Row(mainAxisSize: MainAxisSize.min, children: children),
  );

  /// 浮层顶部的状态提示（#8 裁剪 / #9 落盘失败 / #10 读取损坏）。
  ///
  /// 三条都是「不说用户就不知道」的事实；只显示非空的那几条。
  List<Widget> _noticeLines(ThemeData theme) {
    final AppPalette palette = appPaletteOf(context);
    final List<(String, Color)> lines = <(String, Color)>[
      // 落盘失败最严重：用户以为记录还在，刷新后才会发现没了（#9）。
      if (widget.saveFailed) (kSessionsNotPersistedNote, palette.danger),
      if (widget.loadWarning != null) (widget.loadWarning!, palette.warning),
      if (widget.trimNotice != null) (widget.trimNotice!, palette.warning),
    ];
    return <Widget>[
      for (final (String text, Color color) in lines)
        Padding(
          padding: const EdgeInsets.only(
            left: Space.s4,
            top: Space.s2,
            right: Space.s4,
          ),
          child: Text(
            text,
            style: theme.textTheme.bodySmall?.copyWith(color: color),
          ),
        ),
    ];
  }

  /// 副标题：条数 + 最后活动时间 + **会话 id 短提示**（#7）。
  ///
  /// 为什么带 id：这些会话**只在本机**（浮层顶部那句），而两个会话标题很容易
  /// 一样（都叫「新对话」/ 同一句话开头）。不给一个稳定短号，用户无法把界面上
  /// 这一行与「存储里那条记录」对上；人格面板在未知 id 时回落的「当前会话」
  /// 也就无从核对。只取末尾几位：显示全长会把副标题挤爆。
  String _subtitle(ChatSession session) {
    final String idHint = '$kSessionIdHintLabel ${sessionIdHint(session.id)}';
    final int n = session.messages.length;
    final DateTime? at = session.lastMessageAt;
    if (n == 0 || at == null) return '空会话 · $idHint';
    return '$n 条 · ${relativeTime(at, now: DateTime.now())} · $idHint';
  }

  /// 某会话当前会显示的标题（重命名确认文案用；找不到就回落「新对话」）。
  String _displayTitleOf(String id) {
    for (final ChatSession s in widget.sessions) {
      if (s.id == id) return s.displayTitle;
    }
    return kUntitledSessionLabel;
  }
}
