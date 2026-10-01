/// A1 资产守护网（A1 动作包下发链 + A2 幅度下发链）。
///
/// 下发链是三段：
///
///     main.dart（导演 cue / 宿主）
///       → Live2DStageState.applyPreset(   [lib/live2d/live2d_stage.dart:418]
///       → Live2DBridge.sendPreset(        [lib/live2d/live2d_bridge.dart:260]
///       → preset 帧                       [lib/live2d/live2d_bridge.dart:293]
///
/// 幅度链只有两段（本文件同时守）：
///
///     Live2DStageState.applyActionScales(   [lib/live2d/live2d_stage.dart:498]
///       → sendSync(actionScales:)           [lib/live2d/live2d_stage.dart:501]
///
/// # 本次新增守的是什么缺口（已存在 vs 新增，逐条对照）
///
/// * **行为 · 新增**：Live2DBridge.sendPreset 的**旧键逐字不变 + 缺省时不泄漏
///   v1 键**（V11「只增不改」）。既有 test/action_cue_test.dart:341 只断言
///   「带了 v1 字段时它们都在」，**没有**断言「没带时一个都不多」；既有
///   live2d_bridge_test.dart 完全没有 preset 帧用例。补这一条。
/// * **结构 · 新增**：main.dart 真的**调用** stage.applyPreset(；
///   applyPreset 的函数体真的把参数转发给 _bridge?.sendPreset(；
///   applyActionScales 的函数体真的转发给 _bridge?.sendSync(actionScales:。
///   这三处**都没有任何既有断言**（action_scales_wiring_test.dart 守的是
///   ActionScalesSyncer 与 main.dart 的构造点，不守这条 stage→bridge 缝）。
///
/// # 为什么 stage→bridge 这一段是结构断言（不是偷懒）
///
/// Live2DStage 没有传输注入参数（构造见 lib/live2d/live2d_stage.dart:90-105）；
/// 桥只由 _attach(transport) 挂上（:330），而 _attach 只从
/// host.buildLive2DHost(onTransport: _attach)（:569-573）回调进来。
/// host 是条件导入：VM 测试解析到 live2d_host_stub.dart，它的占位实现
/// **从不调用 onTransport**（该文件 :14-18）⇒ VM 里挂载的 Live2DStage 的
/// _bridge 恒为 null，applyPreset 会静默返回（一行帧都发不出）。
/// 本文件末尾的「取证」用例在 VM 里把这个事实钉住：若哪天它变红，说明 VM 已能
/// 挂桥，那时应把这里的结构守卫升级为「喂 cue → 断言 preset 帧」的行为断言。
///
/// # 结构断言的做法：函数体级，不是全文件 contains
///
/// 用 [methodBody] 只取目标方法的函数体（先对参数表圆括号配平、再对函数体花括号
/// 配平），于是「同类里别的方法碰了 sendPreset」不会被误算成本方法的转发。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/live2d/live2d_transport.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/source_scan.dart';

/// 只记录发出去的帧（与 live2d_bridge_test.dart 同形；协议级断言用）。
class _FakeTransport implements Live2DTransport {
  final StreamController<String> _controller =
      StreamController<String>.broadcast();
  final List<String> sent = <String>[];

  @override
  Stream<String> get events => _controller.stream;

  @override
  Future<void> send(String json) async => sent.add(json);

  @override
  Future<void> dispose() async {
    await _controller.close();
  }

  void emit(String frame) => _controller.add(frame);
}

/// 剥掉注释与字符串（共享词法器：`support/source_scan.dart`，W3-D3 起唯一定义）。
///
/// 必须先剥字符串再做花括号配平，否则字符串里的花括号会把 [methodBody] 配错。
/// 取 [signature] 起那个方法的**函数体**源码（含最外层花括号）；找不到返回 null。
///
/// 先对参数表的圆括号配平（具名参数的 {} 不影响圆括号深度），再取随后的第一个
/// 花括号做函数体配平。只认这一段，避免「同类里别的方法」冒充本方法的转发。
String? methodBody(String src, String signature) {
  final int start = src.indexOf(signature);
  if (start < 0) return null;
  final int open = src.indexOf('(', start);
  if (open < 0) return null;
  int depth = 0;
  int i = open;
  while (i < src.length) {
    final String c = src[i];
    if (c == '(') {
      depth++;
    } else if (c == ')') {
      depth--;
      if (depth == 0) {
        i++;
        break;
      }
    }
    i++;
  }
  final int brace = src.indexOf('{', i);
  if (brace < 0) return null;
  int braces = 0;
  int j = brace;
  while (j < src.length) {
    final String c = src[j];
    if (c == '{') {
      braces++;
    } else if (c == '}') {
      braces--;
      if (braces == 0) return src.substring(brace, j + 1);
    }
    j++;
  }
  return null;
}

void main() {
  final String stageSource = stripCommentsAndStrings(
    File('lib/live2d/live2d_stage.dart').readAsStringSync(),
  );
  final String mainSource = stripCommentsAndStrings(
    File('lib/main.dart').readAsStringSync(),
  );

  group('结构守卫：preset 下发链每一环都在（行为在 VM 下不可达，见文件头注）', () {
    test('main.dart 仍然**调用** stage.applyPreset(（不是只定义）', () {
      expect(
        RegExp(r'stage\s*\??\.\s*applyPreset\s*\(').hasMatch(mainSource),
        isTrue,
        reason:
            '导演 cue 取用后必须真的落到舞台上（main.dart:612）。'
            '只留 Live2DStageState.applyPreset 的定义、删掉 main.dart 的调用，'
            '既有断言（action_cue_test:290 只查名字）发现不了。',
      );
    });

    test('applyPreset 的函数体把参数转发给 _bridge?.sendPreset(', () {
      final String? body = methodBody(stageSource, 'Future<void> applyPreset(');
      expect(body, isNotNull, reason: 'applyPreset 方法本身不许消失');
      expect(
        RegExp(r'_bridge\s*\?\.\s*sendPreset\s*\(').hasMatch(body!),
        isTrue,
        reason:
            '直播审计 §3.3 点名：live2d_stage.dart 的 applyPreset 一旦不再转发，'
            '动作包（preset 帧）会静默丢失。',
      );
    });

    test('applyPreset 保留「没有 field 且 id 为空 = 本轮不动」的早退', () {
      final String body = methodBody(stageSource, 'Future<void> applyPreset(')!;
      expect(
        RegExp(r'!hasField\s*&&\s*id\.isEmpty').hasMatch(body),
        isTrue,
        reason: '旧路径语义（preset_id 空串 = 本轮不动）不得在重放里被改掉',
      );
    });

    test('applyActionScales 的函数体转发给 _bridge?.sendSync(actionScales:', () {
      final String? body = methodBody(stageSource, 'void applyActionScales(');
      expect(body, isNotNull, reason: 'applyActionScales 方法本身不许消失');
      expect(
        RegExp(r'_bridge\s*\?\.\s*sendSync\s*\(').hasMatch(body!),
        isTrue,
        reason: 'A2 每皮套幅度只许走 sync.actionScales 出口',
      );
      expect(
        RegExp(r'actionScales\s*:').hasMatch(body),
        isTrue,
        reason: '必须作为 sync 的 actionScales 字段下发（不是自造帧）',
      );
    });

    test('applyActionScales 保留「三键 + 全部有限」的静默不发判据', () {
      final String body = methodBody(stageSource, 'void applyActionScales(')!;
      expect(
        RegExp(r'scales\.length\s*!=\s*3').hasMatch(body),
        isTrue,
        reason: '不是三个键就不发（宁可不发，也不污染舞台快照）',
      );
      expect(
        RegExp(r'isFinite').hasMatch(body),
        isTrue,
        reason: '非有限值不发',
      );
    });
  });

  group('行为：preset 帧的旧键逐字不变（V11 只增不改）', () {
    test('缺 field 时只发旧四键，一个字都不多', () async {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      await bridge.sendPreset(
        'nod',
        source: 'director',
        ttlMs: 1800,
        intensity: 2.0,
      );

      expect(transport.sent, hasLength(1));
      final Map<String, Object?> frame =
          jsonDecode(transport.sent.single) as Map<String, Object?>;
      expect(frame['version'], 1);
      expect(frame['type'], 'preset', reason: '消息类型不得改名（V11）');
      final Map<String, Object?> payload =
          frame['payload']! as Map<String, Object?>;
      expect(payload['id'], 'nod');
      expect(payload['source'], 'director');
      expect(payload['ttl_ms'], 1800.0);
      expect(payload['intensity'], 2.0);
      for (final String v1Key in <String>[
        'field',
        'x',
        'y',
        'z',
        'hold',
        'at',
        'seq',
        'epoch',
        'sentence_seq',
      ]) {
        expect(
          payload.containsKey(v1Key),
          isFalse,
          reason: '缺省时不得多发 v1 键（「缺 field 时语义与今天逐字相同」）',
        );
      }
      await bridge.destroy();
    });
  });

  testWidgets('取证：VM 测试里舞台走 host stub ⇒ _bridge 恒 null（结构守卫的理由）', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: buildAppTheme(),
        home: const Scaffold(body: Live2DStage()),
      ),
    );
    await tester.pump();
    expect(
      find.text('Live2D 舞台仅在 Web 平台可用'),
      findsOneWidget,
      reason:
          'VM 下 buildLive2DHost 解析到 live2d_host_stub.dart（不回调 onTransport）'
          '⇒ 没有桥、applyPreset 发不出帧。这条若变红 = VM 已能挂桥，'
          '请把上面的结构守卫升级为「喂 cue → 断言 preset 帧」的行为断言。',
    );
  });
}
