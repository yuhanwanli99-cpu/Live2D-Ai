/// F-0005-2（审计 45 条 · rc.7 A 组「重建放大链」）的回归。
///
/// # 这条钉子防的是什么
///
/// `text_delta` 是**毫秒级**的：每来一个 delta，`main.dart` 那层
/// `ListenableBuilder` 就会重建整个 `AppShell`。而设置面板是**常驻树**里的
/// 一棵子树（`CollapsiblePanel` 折叠只关指针 / 焦点 / 语义 / 动画四条通路，
/// **不卸载**——那是有意的保活语义，见 `settings_panel_keepalive_test.dart`）。
///
/// 于是旧实现里每个 delta 都会把**当前分区**整棵重建 + 布局一次，**即使设置
/// 面板关着**。本文件把「重建」变成可计数的事实：给外壳传一个**计数用**的
/// `sectionBuilder`，模拟 10 次聊天增量（外层 `ListenableBuilder` 与 `main.dart`
/// 的接线同形），断言它的调用次数增长 ≤ 1。
///
/// # 同时钉住「没有把它一起省掉」
///
/// 「不重建」很容易顺手做成「永久冻结」：聊天自己不再上屏、设置面板打开 /
/// 切分区失效、设置数据回来不刷新。所以本文件里每一条「不重建」都配一条
/// 「该更新的还得更新」。
///
/// # 审计口径提醒
///
/// F-0005-2 的**幅度**（是否肉眼掉帧）账本标的是「未核实」，本文件不碰它——
/// 这里只钉机制（谁在重建），不假装测过真机帧率。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/app/collapsible_panel.dart';
import 'package:live2d_ai_shell/app/nav_host.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

/// 聊天控制器的**替身**：只保留外壳真正订阅的那部分（消息 + 错误）。
class _ChatStub extends ChangeNotifier {
  _ChatStub() {
    messages.add(ChatMessage(role: ChatRole.assistant, text: '第一句'));
  }

  final List<ChatMessage> messages = <ChatMessage>[];
  String? error;

  /// 模拟一个 `text_delta`：追加正文 + 通知（与 `ChatController._appendDelta` 同形）。
  void appendDelta(String text) {
    messages.last.text += text;
    notifyListeners();
  }

  void fail(String message) {
    error = message;
    notifyListeners();
  }
}

/// 与 `main.dart` **同形**的接线：外层 `ListenableBuilder` 订阅聊天控制器，
/// 每个通知都新建一个 `AppShell` —— 审计描述的放大链正是从这里开始。
class _ChatDrivenShell extends StatefulWidget {
  const _ChatDrivenShell({
    required this.chat,
    required this.input,
    required this.sectionBuilder,
    this.settingsChanges = const NeverNotifies(),
    super.key,
  });

  final _ChatStub chat;
  final TextEditingController input;
  final Widget Function(BuildContext context, SettingsSection section)
  sectionBuilder;
  final Listenable settingsChanges;

  @override
  State<_ChatDrivenShell> createState() => _ChatDrivenShellState();
}

class _ChatDrivenShellState extends State<_ChatDrivenShell> {
  SettingsSection section = SettingsSection.theme;

  /// 宿主状态代际：**只在宿主自己重建时**前进（与 `main.dart` 同形）。
  ///
  /// 聊天通知不会走到这里——它只重建那个 `ListenableBuilder` 的子树。
  int revision = 0;

  /// 让测试推一次「宿主状态变了」（真实宿主里就是一次 `setState`）。
  void bumpRevision() => setState(() {});

  @override
  Widget build(BuildContext context) {
    revision++;
    return ListenableBuilder(
      listenable: widget.chat,
      builder: (BuildContext context, Widget? _) => AppShell(
        prefs: const DisplayPrefs(),
        stage: const ColoredBox(color: Color(0xFF101010)),
        phase: UiPhase.idle,
        wsStatus: WsStatus.connected,
        messages: widget.chat.messages,
        input: widget.input,
        error: widget.chat.error,
        onSend: () {},
        onStop: () {},
        onRetryConnection: () {},
        volume: 0.8,
        muted: false,
        onVolumeChanged: (_) {},
        onMutedChanged: (_) {},
        sections: visibleSections(),
        section: section,
        onSectionChanged: (SettingsSection next) =>
            setState(() => section = next),
        settingsChanges: widget.settingsChanges,
        settingsRevision: revision,
        sectionBuilder: widget.sectionBuilder,
        stagePhase: Live2DBridgePhase.ready,
      ),
    );
  }
}

/// 1400×800 = expanded：设置面板走**内联侧板**（常驻树里，折叠不卸载）。
Future<void> _pumpShell(
  WidgetTester tester, {
  required _ChatStub chat,
  required TextEditingController input,
  required Widget Function(BuildContext context, SettingsSection section)
  sectionBuilder,
  Listenable settingsChanges = const NeverNotifies(),
  Key? key,
}) async {
  await tester.binding.setSurfaceSize(const Size(1400, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      theme: buildAppTheme(),
      home: _ChatDrivenShell(
        key: key,
        chat: chat,
        input: input,
        settingsChanges: settingsChanges,
        sectionBuilder: sectionBuilder,
      ),
    ),
  );
  await tester.pumpAndSettle();
}

_ChatStub _newChat() {
  final _ChatStub chat = _ChatStub();
  addTearDown(chat.dispose);
  return chat;
}

TextEditingController _newInput() {
  final TextEditingController input = TextEditingController();
  addTearDown(input.dispose);
  return input;
}

/// 内联侧板里那个折叠容器（断线横幅也是 `CollapsiblePanel`，必须限定范围）。
Finder _dockPanel() => find.descendant(
  of: find.byType(InlineSettingsDock),
  matching: find.byType(CollapsiblePanel),
);

void main() {
  testWidgets('10 次聊天增量最多只让当前设置分区重建 1 次（F-0005-2）', (
    WidgetTester tester,
  ) async {
    final _ChatStub chat = _newChat();
    final TextEditingController input = _newInput();

    int panes = 0;
    await _pumpShell(
      tester,
      chat: chat,
      input: input,
      sectionBuilder: (BuildContext context, SettingsSection s) {
        panes++;
        return const Text('PANE');
      },
    );

    // 前提：面板**关着**的时候，分区内容也在树里（这正是保活语义，也是
    // 「关着也会被重建」这句话成立的原因）。不先确认这一点，下面的计数
    // 可能只是因为压根没建过。
    expect(
      find.text('PANE'),
      findsOneWidget,
      reason: '折叠态的分区内容不在树里 —— 保活语义坏了，下面的计数什么也证明不了',
    );
    final int before = panes;
    expect(before, greaterThan(0), reason: '首帧都没建过分区内容');

    for (int i = 0; i < 10; i++) {
      chat.appendDelta('字');
      await tester.pump();
    }

    expect(
      panes - before,
      lessThanOrEqualTo(1),
      reason: '每个聊天增量都把当前设置分区整棵重建了一遍（F-0005-2 的重建放大链）',
    );
    // 同一场景里顺带钉住「没有拿聊天更新换设置面板」。
    expect(
      find.text('第一句字字字字字字字字字字'),
      findsOneWidget,
      reason: '聊天面板没跟着更新 —— 这不是修复，是把流式更新一起省掉了',
    );
  });

  testWidgets('聊天错误横幅仍然跟着更新', (WidgetTester tester) async {
    final _ChatStub chat = _newChat();
    final TextEditingController input = _newInput();
    await _pumpShell(
      tester,
      chat: chat,
      input: input,
      sectionBuilder: (BuildContext context, SettingsSection s) =>
          Text('PANE:${s.label}'),
    );

    chat.fail('llm_upstream_401：上游非成功状态');
    await tester.pump();

    expect(
      find.textContaining('llm_upstream_401'),
      findsOneWidget,
      reason: '聊天错误没有上屏 —— 外壳那层订阅被改坏了',
    );
  });

  testWidgets('设置面板打开时仍显示当前分区，切分区仍然生效', (WidgetTester tester) async {
    final _ChatStub chat = _newChat();
    final TextEditingController input = _newInput();
    await _pumpShell(
      tester,
      chat: chat,
      input: input,
      sectionBuilder: (BuildContext context, SettingsSection s) =>
          Text('PANE:${s.label}'),
    );
    expect(find.text('PANE:主题'), findsOneWidget, reason: '折叠态也该在树里');

    await tester.tap(find.text('设置'));
    await tester.pumpAndSettle();
    expect(
      tester.getSize(_dockPanel()).width,
      greaterThanOrEqualTo(NavMetrics.paneWidth),
      reason: '点了设置但侧板没展开',
    );

    await tester.tap(find.widgetWithText(ChoiceChip, '模型服务'));
    await tester.pumpAndSettle();
    expect(
      find.text('PANE:模型服务'),
      findsOneWidget,
      reason: '切分区不生效 —— 缓存把面板冻在旧分区上了',
    );
  });

  testWidgets('设置数据通知仍然让分区内容重建（缓存不能把面板冻住）', (WidgetTester tester) async {
    final _ChatStub chat = _newChat();
    final TextEditingController input = _newInput();
    final ValueNotifier<int> changes = ValueNotifier<int>(0);
    addTearDown(changes.dispose);
    bool loaded = false;

    await _pumpShell(
      tester,
      chat: chat,
      input: input,
      settingsChanges: changes,
      sectionBuilder: (BuildContext context, SettingsSection s) => Text(
        loaded ? 'READY:${s.label}' : 'LOADING:${s.label}',
      ),
    );
    expect(find.text('LOADING:主题'), findsOneWidget);

    // 模拟「设置数据回来了」（真实的 SettingsController 每次 notify 都走这里）。
    loaded = true;
    changes.value++;
    await tester.pump();

    expect(
      find.text('READY:主题'),
      findsOneWidget,
      reason: '设置数据回来了，分区内容却没重建 —— 面板会永远停在「加载中」/旧值上',
    );
  });

  testWidgets('宿主状态代际前进时分区内容会跟着刷新（不是永久冻结）', (WidgetTester tester) async {
    final _ChatStub chat = _newChat();
    final TextEditingController input = _newInput();
    final GlobalKey<_ChatDrivenShellState> host =
        GlobalKey<_ChatDrivenShellState>();
    // 宿主那批「外壳看不见」的状态（真实宿主里是模型库 / 自检结果 / 覆盖表…）。
    String label = 'v1';

    await _pumpShell(
      tester,
      key: host,
      chat: chat,
      input: input,
      sectionBuilder: (BuildContext context, SettingsSection s) => Text(label),
    );
    expect(find.text('v1'), findsOneWidget);

    label = 'v2';
    host.currentState!.bumpRevision();
    await tester.pump();

    expect(
      find.text('v2'),
      findsOneWidget,
      reason: '代际前进了却没重建 —— 缓存把设置分区永久冻在上一份内容上了',
    );
  });
}
