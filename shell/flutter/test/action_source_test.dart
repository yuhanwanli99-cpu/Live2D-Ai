import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/action_source.dart';

/// **真实抓包**的服务端 `action_state` 帧（本机 v0.4.5，2026-09-10）。
///
/// 取自 `POST /api/v1/commands/live2d_perform_action_nod/invoke`（strength=3）
/// 与 `..._shake_no/invoke`（strength=1）之后 `/ws/state` 上收到的原文。
/// **不要凭印象改写这两个夹具**——它们的作用就是把「服务端到底发什么形状」
/// 钉死；一旦协议真的变了，改夹具前先确认服务端确实变了。
const List<String> kRealActionFrames = <String>[
  '{"data":{"action":{"action":"nod","source":"user_command","strength":3},'
      '"epoch":0,"kind":"perform"},"seq":2,'
      '"ts":"2026-09-10T11:38:23.170Z","type":"action_state"}',
  '{"data":{"action":{"action":"shake_no","source":"user_command","strength":1},'
      '"epoch":0,"kind":"perform"},"seq":3,'
      '"ts":"2026-09-10T11:38:24.680Z","type":"action_state"}',
];

/// 从一帧原文里取出 `data.action` 并解析——即前端真正要做的那一步。
ActionDetail parseFrame(String raw) {
  final Object? frame = jsonDecode(raw);
  final Object? data = (frame! as Map<Object?, Object?>)['data'];
  return ActionDetail.fromData((data! as Map<Object?, Object?>)['action']);
}

void main() {
  group('真实帧必须解析出动作名 / 强度 / 来源（回归钉子）', () {
    test('nod strength=3 source=user_command', () {
      final ActionDetail d = parseFrame(kRealActionFrames[0]);
      // 这一条就是缺陷的钉子：修复前 `_str(data['action'])` 对 Map 返回 null，
      // 于是 action 永远为 null——下面的 expect 会立刻变红。
      expect(d.action, 'nod');
      expect(d.strength, 3);
      expect(d.source, ActionSource.userCommand);
      expect(d.isEmpty, isFalse);
    });

    test('shake_no strength=1 source=user_command', () {
      final ActionDetail d = parseFrame(kRealActionFrames[1]);
      expect(d.action, 'shake_no');
      expect(d.strength, 1);
      expect(d.source, ActionSource.userCommand);
    });

    test('两帧的 source 都是「用户」而非「AI」', () {
      // 这两帧是走 /commands/{id}/invoke 触发的，服务端标 user_command。
      // 若哪天这里变成 llmTool，说明仲裁链路变了下游语义，需要重新确认。
      for (final String raw in kRealActionFrames) {
        final ActionDetail d = parseFrame(raw);
        expect(d.source.isAiInitiated, isFalse);
        expect(d.source.label, '你');
      }
    });
  });

  group('宽容解析：坏帧不许抛，也不许把事件流打断', () {
    test('cease 帧没有 action 字段 → 空明细', () {
      // events.rs 的 `RenderCommand::CeasePerforming` 分支只发 {epoch, kind}。
      final Object? frame = jsonDecode(
        '{"data":{"epoch":0,"kind":"cease"},"type":"action_state"}',
      );
      final Object? data = (frame! as Map<Object?, Object?>)['data'];
      final ActionDetail d = ActionDetail.fromData(
        (data! as Map<Object?, Object?>)['action'],
      );
      expect(d.isEmpty, isTrue);
      expect(d.action, isNull);
      expect(d.source, ActionSource.unknown);
    });

    test('不是对象（字符串 / 列表 / 数字 / null）一律降级为空明细', () {
      // 特别说明：**旧前端正是把对象当字符串**才出的缺陷。这里把「传字符串」
      // 也纳为坏输入，就是为了让那条老路彻底走不通。
      for (final Object? bad in <Object?>[
        'nod', // ← 曾经被误当成正确的形状
        <Object?>['nod'],
        42,
        null,
      ]) {
        final ActionDetail d = ActionDetail.fromData(bad);
        expect(d.isEmpty, isTrue, reason: '输入 $bad 应降级为空明细');
        expect(d.source, ActionSource.unknown);
      }
    });

    test('字段类型不对时逐字段降级，不整体丢弃', () {
      final ActionDetail d = ActionDetail.fromData(<String, Object?>{
        'action': 'tilt',
        'strength': 'strong', // 类型不符 → null
        'source': 99, // 类型不符 → unknown
      });
      expect(d.action, 'tilt', reason: '好字段不该被坏字段连累');
      expect(d.strength, isNull);
      expect(d.source, ActionSource.unknown);
    });

    test('strength 是浮点时取整（JSON 里 2.0 很常见）', () {
      final ActionDetail d = ActionDetail.fromData(<String, Object?>{
        'action': 'nod',
        'strength': 2.0,
      });
      expect(d.strength, 2);
    });

    test('多余的未知字段被忽略', () {
      final ActionDetail d = ActionDetail.fromData(<String, Object?>{
        'action': 'nod',
        'strength': 1,
        'source': 'user_command',
        'future_field': <String, Object?>{'x': 1},
      });
      expect(d.action, 'nod');
      expect(d.strength, 1);
      expect(d.source, ActionSource.userCommand);
    });
  });

  group('ActionSource 映射', () {
    test('三个协议值与 core 侧逐字对应', () {
      expect(ActionSource.fromWire('user_command'), ActionSource.userCommand);
      expect(ActionSource.fromWire('llm_tool'), ActionSource.llmTool);
      expect(ActionSource.fromWire('rule_fallback'), ActionSource.ruleFallback);
    });

    test('认不出的来源 → unknown（协议新增来源时旧前端不该崩）', () {
      for (final String? bad in <String?>[null, '', 'USER_COMMAND', 'new_one']) {
        expect(ActionSource.fromWire(bad), ActionSource.unknown);
      }
    });

    test('unknown 不属于「AI 发起」——不把未知当 AI', () {
      // 若把 unknown 归到 AI 一侧，协议一变就会把用户的动作标成 AI 做的。
      expect(ActionSource.unknown.isAiInitiated, isFalse);
      expect(ActionSource.userCommand.isAiInitiated, isFalse);
      expect(ActionSource.llmTool.isAiInitiated, isTrue);
      expect(ActionSource.ruleFallback.isAiInitiated, isTrue);
    });

    test('每个来源都有非空标签', () {
      for (final ActionSource s in ActionSource.values) {
        expect(s.label, isNotEmpty);
        expect(s.wire, isNotEmpty);
      }
    });
  });
}
