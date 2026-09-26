/// W7（2026-09-23）：动作幅度**下发通道整修**的回归。
///
/// # 被修掉的确定性缺陷（RESEARCH-actions-director-audit-2026-09-21.md §2.3/§2.4）
///
/// 1. 通道有两个互不知情的写者、各自去重：产品/草稿值走 `ActionScalesSyncer`，
///    调试面板的临时覆盖直发 `stage.sync(actionScales: …)` 且**不更新** `_sent`；
/// 2. 任何 `DisplayPrefs` 变更（改主题 / 调音量）都会走
///    `_applyPrefs → _syncActionScalesNow(force: true)`，`force` 绕过去重 ⇒
///    临时值被产品值**静默冲掉**（旧文案只说「未保存的草稿」会冲掉它）；
/// 3. `Live2DStage.actionScales` 从未被传过 ⇒ `_attach` / `didUpdateWidget`
///    这两条 iframe 重建后的自愈通路永远拿到 `null`。
///
/// 现在**三层优先级**写死在 syncer 里：**临时覆盖 > 草稿 > 磁盘值**，
/// 且所有下发只经过它一个出口（`active()` / `syncNow()` 发的永远是当前有效值）。
///
/// # 为什么这里同时有「纯逻辑」与「源码扫描」两类断言
///
/// `main.dart` 经 `app/browser_io.dart` 依赖 `package:web`，在 `flutter test`
/// （Dart VM）里**加载不了**；`buildLive2DHost` 又走 `_stub.dart`（不回调
/// `onTransport`，没有 bridge）。所以「构造点真的传了 actionScales」只能按
/// `wiring_test.dart` 的先例**扫源码**钉住，发帧那条路由真机验收。
library;

import 'dart:io';

import 'package:fake_async/fake_async.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/live2d/action_scales_sync.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/live2d/preset_status.dart';
import 'package:live2d_ai_shell/live2d/render_events.dart';
import 'package:live2d_ai_shell/settings/mods/director_panel.dart';
import 'package:live2d_ai_shell/settings/preset_labels.dart';
import 'package:live2d_ai_shell/settings/sections/dev_tools_section.dart';
import 'package:live2d_ai_shell/ui/field_row.dart' show SliderField;
import 'package:live2d_ai_shell/ui/theme.dart';

/// 与 `assets/actions/preset_labels.json` 同形的**假表**：故意与
/// `kExpressionPresetIds` 不对齐，用来证明「表优先、常量只兜底」。
const PresetLabelTable _table = PresetLabelTable(<String, PresetLabel>{
  'smile': PresetLabel(zh: '微笑', en: 'Smile', channel: 'expression'),
  // 在 kExpressionPresetIds 里，但表说它是 motion —— 表必须赢。
  'unhappy': PresetLabel(zh: '不悦', en: 'Unhappy', channel: 'motion'),
  // 不在 kExpressionPresetIds 里，但表说它是 expression —— 表必须赢。
  'wink': PresetLabel(zh: '眨眼', en: 'Wink', channel: 'expression'),
});

/// 读一份 `lib/**` 源码（`flutter test` 的工作目录 = 包根）。
String readLib(String rel) => File(rel).readAsStringSync();

/// 一份「产品值」载荷（与服务端出厂默认同口径）。
const Map<String, double> _product = <String, double>{
  'head': 0.75,
  'body': 0.80,
  'expression': 1.0,
};

/// 一份「临时覆盖」载荷（调试面板三个滑条拖出来的值）。
const Map<String, double> _temp = <String, double>{
  'head': 2.0,
  'body': 0.80,
  'expression': 1.0,
};

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

/// 舞台要占满一屏（`Stack(fit: expand)` 在滚动视图的无界高里会报约束错）。
Widget _wrapStage(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: child),
);

void main() {
  group('W7：三层优先级（临时覆盖 > 草稿 > 磁盘值）', () {
    test('改主题后临时值仍在：DisplayPrefs 变更的 force 补发不得冲掉临时覆盖', () {
      final List<Map<String, double>> sent = <Map<String, double>>[];
      final ActionScalesSyncer syncer = ActionScalesSyncer(sent.add);

      syncer.pin(_temp);
      expect(sent, hasLength(1), reason: '「应用到渲染面（临时）」必须立刻下发');
      expect(sent.single['head'], 2.0);

      // 改主题 / 调音量 / 开关待机 = 任意 DisplayPrefs 变更都会走到
      // `shell_prefs.dart` 的 `_syncActionScalesNow(force: true)`。
      syncer.syncNow(_product, force: true);
      expect(
        sent.any((Map<String, double> m) => m['head'] == 0.75),
        isFalse,
        reason: '改主题后临时值仍在：force 补发只能重发当前有效值（临时覆盖），'
            '不得把产品 0.75 写下去',
      );
      expect(
        sent.last['head'],
        2.0,
        reason: '改主题后临时值仍在：DisplayPrefs 变更走的 force 补发只重发临时覆盖',
      );

      // 非 force 的防抖路（草稿变更 / 设置加载完成）同样发不出产品值。
      syncer.schedule(_product);
      syncer.syncNow(_product);
      expect(sent.last['head'], 2.0, reason: '草稿变化也不得冲掉临时覆盖');

      // 宿主给舞台的自愈快照（iframe 重建用）同样是临时值；产品值还没加载也一样。
      expect(syncer.active(_product)!['head'], 2.0);
      expect(syncer.active(null)!['head'], 2.0, reason: '设置未加载时也不能退回 null');

      // 「唯一出口」：调试面板不再直发 `stage.sync(actionScales: …)`。
      final String settings = readLib('lib/app/shell_settings.dart');
      expect(settings.contains('_actionScalesSyncer.pin('), isTrue);
      expect(
        settings.contains('_stageKey.currentState?.sync('),
        isFalse,
        reason: '临时覆盖必须经 syncer；直发路径已删（RESEARCH §2.4 的缺陷 2）',
      );
      syncer.dispose();
    });

    testWidgets('点「恢复产品设置」→ 产品值一定写下去（不受 _sent 去重挡住）', (
      WidgetTester tester,
    ) async {
      // ① 纯逻辑：临时值恰好等于产品值时，`_sent` 去重会挡住普通下发，
      //    所以「恢复」必须 force（syncer.clearPin）。
      final List<Map<String, double>> sent = <Map<String, double>>[];
      final ActionScalesSyncer syncer = ActionScalesSyncer(sent.add);
      syncer.pin(_product);
      expect(sent, hasLength(1));
      expect(syncer.sentSnapshot, isNotNull);
      syncer.clearPin(_product);
      expect(
        sent,
        hasLength(2),
        reason: '恢复产品设置必须 force——_sent 已等于产品值，普通去重会把它挡掉',
      );
      expect(sent.last, _product);
      expect(syncer.hasPin, isFalse);
      syncer.dispose();

      // ② 接线：面板上的按钮走 `onClearScales`（不再是「把当前临时值再钉一次」）。
      bool cleared = false;
      await tester.pumpWidget(
        _wrap(
          DebugPanels(
            onApplyPreset: (String _, double _) {},
            productScales: const ActionSettingsView(
              headScale: 0.75,
              bodyScale: 0.80,
              expressionScale: 1.0,
            ),
            onApplyScales: (double _, double _, double _) {},
            onClearScales: () => cleared = true,
            labels: _table,
          ),
        ),
      );
      final Finder reset = find.text('恢复产品设置');
      await tester.ensureVisible(reset);
      await tester.pumpAndSettle();
      await tester.tap(reset);
      await tester.pumpAndSettle();
      expect(cleared, isTrue, reason: '「恢复产品设置」必须让宿主清掉临时覆盖');
    });

    testWidgets('iframe 重建（重挂）后幅度不会退回渲染面出厂默认', (
      WidgetTester tester,
    ) async {
      // ① 构造点真的传了（RESEARCH §2.3 的死参数由此接上）。
      final String main = readLib('lib/main.dart');
      expect(
        main.contains('actionScales: _stageActionScales'),
        isTrue,
        reason: 'main.dart 的 Live2DStage(...) 必须传当前有效幅度；'
            '不传 = _attach / didUpdateWidget 两条自愈通路退回 null（= 出厂默认）',
      );
      final String prefs = readLib('lib/app/shell_prefs.dart');
      expect(
        prefs.contains('_actionScalesSyncer.active('),
        isTrue,
        reason: '自愈快照必须经 syncer 的 active()（临时覆盖 > 草稿 > 磁盘值）',
      );

      // ② 值本身：设置还没加载（product=null）时，有临时覆盖就不能是 null。
      final ActionScalesSyncer syncer = ActionScalesSyncer((Map<String, double> _) {});
      syncer.pin(_temp);
      final Map<String, double>? snapshot = syncer.active(null);
      expect(snapshot, isNotNull, reason: '有临时覆盖时快照不得为 null');

      // 挂载 → 换 key 重挂（新 State，等价 retry / iframe 重建）。
      await tester.pumpWidget(
        _wrapStage(
          Live2DStage(key: const ValueKey<String>('g1'), actionScales: snapshot),
        ),
      );
      expect(
        tester.widget<Live2DStage>(find.byType(Live2DStage)).actionScales,
        _temp,
      );
      await tester.pumpWidget(
        _wrapStage(
          Live2DStage(key: const ValueKey<String>('g2'), actionScales: snapshot),
        ),
      );
      expect(
        tester.widget<Live2DStage>(find.byType(Live2DStage)).actionScales,
        _temp,
        reason: '重挂后舞台拿到的仍是临时快照，不是 null（渲染面出厂默认）',
      );
      syncer.dispose();
    });

    test('值没变不重复下发；拖动只发一帧（临时覆盖期间同样成立）', () {
      fakeAsync((FakeAsync async) {
        final List<Map<String, double>> sent = <Map<String, double>>[];
        final ActionScalesSyncer syncer = ActionScalesSyncer(sent.add);

        // 拖动三滑条：20 次 onChanged 只落一帧。
        for (int i = 1; i <= 20; i++) {
          syncer.schedule(<String, double>{
            'head': i / 10,
            'body': 0.80,
            'expression': 1.0,
          });
          async.elapse(const Duration(milliseconds: 5));
        }
        expect(sent, isEmpty, reason: '防抖窗口没到点不该发');
        async.elapse(kActionScalesDebounce);
        expect(sent, hasLength(1), reason: '一次拖动只下发一帧');
        expect(sent.single['head'], 2.0);

        // 值没变不重发。
        syncer.syncNow(sent.single);
        expect(sent, hasLength(1), reason: '同一份值不该重复下发');

        // 临时覆盖期间：草稿 / 磁盘值再怎么变都不打扰渲染面。
        syncer.pin(_temp);
        expect(sent, hasLength(2));
        for (int i = 1; i <= 5; i++) {
          syncer.schedule(<String, double>{..._product, 'head': i / 10});
          async.elapse(const Duration(milliseconds: 5));
        }
        async.elapse(kActionScalesDebounce);
        expect(
          sent,
          hasLength(2),
          reason: '临时覆盖期间产品/草稿值不得重复下发（否则临时值会被冲掉）',
        );
        syncer.dispose();
      });
    });
  });

  group('W7：点名的文案 / 通道口径守卫（B②/C③/A①）', () {
    test('B②：通道判据优先查标签表，取不到表才回落 kExpressionPresetIds', () {
      // 表里 'unhappy'=motion、'wink'=expression，都与常量集合相反。
      expect(directorPresetText('unhappy', labels: _table), '短动作（unhappy）');
      expect(directorPresetText('wink', labels: _table), '表情（wink）');
      expect(isExpressionChannel('unhappy', labels: _table), isFalse);
      expect(isExpressionChannel('wink', labels: _table), isTrue);

      // 取不到表（空表）→ 回落常量：旧行为一字不改。
      expect(directorPresetText('unhappy'), '表情（unhappy）');
      expect(directorPresetText('wink'), '短动作（wink）');
      expect(directorPresetText('bogus'), '短动作（bogus）');
      expect(isExpressionChannel('unhappy'), isTrue);
      expect(isExpressionChannel('wink'), isFalse);

    });

    test('B②：面板到点时长来自渲染面 ack（O3 默认），前端不再跑本地计时器', () {
      // 阶段4d：v0 的「按标签表猜固定 ms + Timer.periodic 推演剩余」已删。
      // 现在快照只由 ack 生成：preset-applied → set（时长取渲染面给的
      // ttl_ms，缺省用 O3 默认）；expired / replaced / dropped → clear。
      final DateTime now = DateTime(2026, 9, 26, 12);

      PresetStatusUpdate applied(String field, {String? id, int? ttlMs}) {
        final RenderEvent e = parseRenderEvent(
          'preset-applied',
          <String, Object?>{
            'field': field,
            'id': ?id,
            'ttl_ms': ?ttlMs,
          },
        )!;
        return presetStatusUpdateFor(e, now);
      }

      expect(applied('expression', id: 'wink').action, PresetStatusAction.set);
      expect(
        applied('expression', id: 'wink').status!.ttl,
        const Duration(milliseconds: 2600),
        reason: 'expression 缺 ttl_ms → O3 默认 2600ms',
      );
      expect(
        applied('body', ttlMs: 1200).status!.ttl,
        const Duration(milliseconds: 1200),
        reason: '渲染面给了 ttl_ms 就用它（真源在渲染面）',
      );
      expect(
        applied('head').status!.ttl,
        const Duration(milliseconds: 900),
        reason: 'head 缺 ttl_ms → O3 默认 900ms',
      );
      // 到点 / 被顶掉 / 缺参数 → 清空（不再有「本地到点」）。
      for (final String type in <String>[
        'preset-expired',
        'preset-replaced',
        'preset-dropped',
      ]) {
        final RenderEvent e = parseRenderEvent(type, <String, Object?>{})!;
        expect(
          presetStatusUpdateFor(e, now).action,
          PresetStatusAction.clear,
          reason: '$type 必须结束这条显示快照',
        );
      }
      // 段结束是音频事件，不动预设显示。
      final RenderEvent ended = parseRenderEvent(
        'segment-ended',
        <String, Object?>{'seg': 2},
      )!;
      expect(
        presetStatusUpdateFor(ended, now).action,
        PresetStatusAction.ignore,
      );

      // 舞台不再有「本地计时器」：源码级扫描（同一文件同时含定时器推演 +
      // applyPreset 即红）由 no_client_side_preset_ttl_prediction_test.dart 守。
    });

    testWidgets('A①：叠加基础表情开关写明会「重新起算（2.6s 重新计时）」', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _wrap(
          DeveloperSection(
            devMode: true,
            onDevModeChanged: (bool _) {},
            forcedByLaunchFlag: false,
            presetLabels: _table,
          ),
        ),
      );
      expect(
        find.textContaining('每次点手势都会把当前基础表情重新起算（2.6s 重新计时）'),
        findsOneWidget,
        reason: '开关的实际行为是重新下发 Face 槽 = 表情重新起算，文案必须写实',
      );
      expect(find.textContaining('缺省关'), findsOneWidget);
    });

    test('C③：director 面板写明「表演层主路由 / staging 回退」', () {
      expect(kDirectorLegacyNotice, contains('表演层是主路由'));
      expect(kDirectorLegacyNotice, contains('staging_* 是回退'));
      // 两句必须保留的锚点（既有 copy 门禁也钉着它们）。
      expect(kDirectorLegacyNotice, contains('此处 staging_* 仅兼容旧配置'));
      expect(kDirectorLegacyNotice, contains('设置 → LLM → 表演层'));
      expect(kDirectorLegacyNotice, contains('SentenceReady'));
      expect(
        readLib('lib/settings/mods/director_panel.dart').contains('遗留回退旁路'),
        isFalse,
        reason: '旧口径「遗留回退旁路 / 不是产品主路径」必须改掉',
      );
    });
  });

  group('W7b/D2：Developer 面板与「临时幅度覆盖」状态同步', () {
    // 三个值都与产品值不同：能同时钉住「head 也不回产品值」与三项整体播种。
    const Map<String, double> pinned = <String, double>{
      'head': 2.0,
      'body': 1.5,
      'expression': 0.6,
    };
    const ActionSettingsView product = ActionSettingsView(
      headScale: 0.75,
      bodyScale: 0.80,
      expressionScale: 1.0,
    );

    DebugPanels panel({
      Map<String, double>? pin,
      Key? key,
      VoidCallback? onClear,
    }) => DebugPanels(
      key: key,
      onApplyPreset: (String _, double _) {},
      productScales: product,
      pinnedScales: pin,
      onApplyScales: (double _, double _, double _) {},
      onClearScales: onClear ?? () {},
      labels: _table,
    );

    double sliderValue(WidgetTester tester, String label) => tester
        .widget<SliderField>(find.widgetWithText(SliderField, label))
        .value;

    testWidgets('有 pin 时面板初值 = pin（不是产品值），并明示「生效中」', (
      WidgetTester tester,
    ) async {
      // 场景：先在面板里「应用到渲染面（临时）」，宿主把 syncer 的 pin 落进
      // `pinnedScales`；离开 Developer 分区再回来就是一个**新 State**。
      await tester.pumpWidget(_wrap(panel(pin: pinned)));
      expect(
        sliderValue(tester, '临时 head'),
        2.0,
        reason: '有 pin 时滑条初值必须是 pin，不是产品值 0.75',
      );
      expect(sliderValue(tester, '临时 body'), 1.5);
      expect(sliderValue(tester, '临时 expression'), 0.6);
      // 滑条右侧读数区也必须与渲染面有效值一致（不是只有 value 对）。
      expect(find.text('200%'), findsOneWidget);
      expect(find.text('60%'), findsOneWidget);
      expect(
        find.textContaining('临时覆盖生效中'),
        findsOneWidget,
        reason: '有 pin 时 UI 必须明示「临时覆盖生效中（不落盘）」',
      );

      // 「离开后重新进入」= 同一个 DebugPanels 类型换 key 重挂（新 State）。
      await tester.pumpWidget(
        _wrap(panel(pin: pinned, key: const ValueKey<String>('re-enter'))),
      );
      expect(
        sliderValue(tester, '临时 head'),
        2.0,
        reason: '重新进入 Developer 分区后滑条仍是渲染面正在用的 pin（T4 症状）',
      );
      expect(find.textContaining('临时覆盖生效中'), findsOneWidget);
    });

    testWidgets('清 pin 后面板回产品值（didUpdateWidget 重新播种）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(_wrap(panel(pin: pinned)));
      expect(sliderValue(tester, '临时 head'), 2.0);
      expect(find.textContaining('临时覆盖生效中'), findsOneWidget);

      // 「恢复产品设置」→ 宿主 clearPin → pinnedScales 变 null → 同一 State
      // 走 didUpdateWidget 重新播种。
      await tester.pumpWidget(_wrap(panel(pin: null)));
      expect(
        sliderValue(tester, '临时 head'),
        0.75,
        reason: '清 pin 后滑条必须回产品值（不能停在上一组临时值）',
      );
      expect(sliderValue(tester, '临时 body'), 0.80);
      expect(sliderValue(tester, '临时 expression'), 1.0);
      expect(
        find.textContaining('临时覆盖生效中'),
        findsNothing,
        reason: '没有 pin 就不能再声称生效中（无 pin 维持现状）',
      );
    });

    testWidgets('点「恢复产品设置」：宿主这一帧没重建，滑条也必须回产品值', (
      WidgetTester tester,
    ) async {
      // 浮层宿主（medium 底部浮层）不会因外壳 setState 重跑 builder：
      // 面板必须**立刻**自己把滑条播回产品值，不能等宿主下一次重建。
      bool cleared = false;
      await tester.pumpWidget(
        _wrap(panel(pin: pinned, onClear: () => cleared = true)),
      );
      expect(sliderValue(tester, '临时 head'), 2.0);
      final Finder reset = find.text('恢复产品设置');
      await tester.ensureVisible(reset);
      await tester.pumpAndSettle();
      await tester.tap(reset);
      await tester.pumpAndSettle();
      expect(cleared, isTrue, reason: '「恢复产品设置」必须让宿主清掉 pin');
      expect(sliderValue(tester, '临时 head'), 0.75);
      expect(sliderValue(tester, '临时 body'), 0.80);
      expect(sliderValue(tester, '临时 expression'), 1.0);
    });

    test('接线：宿主把 syncer 的显式 pin 只读传进面板（源码扫描）', () {
      final String settings = readLib('lib/app/shell_settings.dart');
      expect(
        settings.contains('pinnedScales: _actionScalesSyncer.pinned'),
        isTrue,
        reason: '面板的初值真源必须是 syncer 的显式 pin；另存一份会分叉',
      );
      final String sync = readLib('lib/live2d/action_scales_sync.dart');
      expect(
        sync.contains('Map<String, double>? get pinned'),
        isTrue,
        reason: 'syncer 已暴露只读 pinned；不得为面板另加写口',
      );
    });
  });
}
