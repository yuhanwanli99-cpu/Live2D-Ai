import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/actions/action_event.dart';
import 'package:live2d_ai_shell/api/action_source.dart';
import 'package:live2d_ai_shell/api/ws_frame.dart';

/// **真实抓包**的 `action_state` 帧（本机 v0.4.6，2026-09-10），与
/// `action_source_test.dart::kRealActionFrames` 同源。这里从**整帧**出发
/// （那一个文件是从 `data.action` 出发）——投影层的输入就是整帧。
const String kRealPerform = '{"data":{"action":{"action":"surprise",'
    '"source":"rule_fallback","strength":3},"epoch":0,"kind":"perform"},'
    '"seq":566,"ts":"2026-09-10T12:14:53.166Z","type":"action_state"}';

/// `perform{source:llm_tool}`：**P2 的验收条件就是这一支**——LLM 自己触发的
/// 动作必须在历史里出现「AI」徽标。这一条按 `ws/events.rs` 的序列化表手写
/// （实抓那一轮 LLM 走的是规则兜底，没有工具调用）。
const String kLlmToolPerform = '{"data":{"action":{"action":"nod",'
    '"source":"llm_tool","strength":2},"epoch":4,"kind":"perform"},'
    '"seq":700,"ts":"2026-09-10T12:20:00.000Z","type":"action_state"}';

/// `cease`：**没有 `data.action` 对象**（`ws/events.rs` 的 `Cease` 分支）。
const String kCease = '{"data":{"epoch":4,"kind":"cease"},'
    '"seq":701,"ts":"2026-09-10T12:20:03.000Z","type":"action_state"}';

ActionEvent project(String raw, {DateTime? at}) =>
    ActionEvent.fromFrame(
      parseWsFrame(raw)! as ActionStateEvent,
      receivedAt: at ?? DateTime(2026, 9, 10, 20, 30),
    );

void main() {
  group('真实帧 → 动作事件（projection）', () {
    test('perform + rule_fallback：字段一个不少', () {
      final ActionEvent e = project(kRealPerform);
      expect(e.kind, ActionEventKind.perform);
      expect(e.action, 'surprise');
      expect(e.strength, 3);
      expect(e.source, ActionSource.ruleFallback);
      expect(e.epoch, 0);
      expect(e.seq, 566);
      expect(e.ts, '2026-09-10T12:14:53.166Z');
      expect(e.receivedAt, DateTime(2026, 9, 10, 20, 30));
      expect(e.hasAction, isTrue);
      expect(e.isDisplayable, isTrue);
    });

    test('perform + llm_tool：这一支就是「AI 触发」的验收依据', () {
      final ActionEvent e = project(kLlmToolPerform);
      expect(e.source, ActionSource.llmTool);
      expect(e.source.isAiInitiated, isTrue);
      expect(e.source.label, 'AI');
      expect(e.strength, 2);
      expect(e.epoch, 4);
    });

    test('cease：没有 action 对象，但**不是**「空事件」', () {
      final ActionEvent e = project(kCease);
      expect(e.kind, ActionEventKind.cease);
      expect(e.isCease, isTrue);
      expect(e.action, isNull, reason: 'cease 帧本来就不带 action');
      expect(e.hasAction, isFalse);
      expect(
        e.isDisplayable,
        isTrue,
        reason: 'cease 必须能进管线去收口——把它当「不可展示」会让当前动作永不结束',
      );
    });
  });

  group('kind 解析：未知取值绝不当成 perform', () {
    test('perform / cease / 未知', () {
      expect(ActionEventKind.fromWire('perform'), ActionEventKind.perform);
      expect(ActionEventKind.fromWire('cease'), ActionEventKind.cease);
      expect(ActionEventKind.fromWire('dropped'), ActionEventKind.unknown);
      expect(ActionEventKind.fromWire(null), ActionEventKind.unknown);
      expect(ActionEventKind.fromWire(''), ActionEventKind.unknown);
    });

    test('未知 kind → 不可展示（不许显示一个没发生的动作）', () {
      final ActionEvent e = ActionEvent.fromFrame(
        parseWsFrame(
              '{"type":"action_state","data":{"epoch":1,"kind":"paused",'
              '"action":{"action":"nod","source":"llm_tool","strength":1}}}',
            )!
            as ActionStateEvent,
      );
      expect(e.kind, ActionEventKind.unknown);
      expect(e.isDisplayable, isFalse);
      expect(e.hasAction, isFalse, reason: 'kind 未知时 hasAction 也必须为假');
    });

    test('perform 但 data.action 是坏的（字符串/缺失）→ 不可展示', () {
      for (final String raw in <String>[
        '{"type":"action_state","data":{"epoch":1,"kind":"perform","action":"nod"}}',
        '{"type":"action_state","data":{"epoch":1,"kind":"perform"}}',
        '{"type":"action_state","data":{"epoch":1,"kind":"perform","action":null}}',
      ]) {
        final ActionEvent e = ActionEvent.fromFrame(
          parseWsFrame(raw)! as ActionStateEvent,
        );
        expect(e.hasAction, isFalse, reason: raw);
        expect(e.isDisplayable, isFalse, reason: raw);
      }
    });

    test('缺 epoch → 0（宽容），不抛', () {
      final ActionEvent e = ActionEvent.fromFrame(
        parseWsFrame(
              '{"type":"action_state","data":{"kind":"perform",'
              '"action":{"action":"tilt"}}}',
            )!
            as ActionStateEvent,
      );
      expect(e.epoch, 0);
      expect(e.action, 'tilt');
      expect(e.strength, isNull);
      expect(e.source, ActionSource.unknown);
    });
  });

  group('强度标签：档位不是数量', () {
    test('1/2/3 → Ⅰ/Ⅱ/Ⅲ', () {
      expect(strengthLabel(1), 'Ⅰ');
      expect(strengthLabel(2), 'Ⅱ');
      expect(strengthLabel(3), 'Ⅲ');
    });

    test('越界/缺失 → null，**不 clamp**（把 7 显示成 Ⅲ 是编造信息）', () {
      expect(strengthLabel(0), isNull);
      expect(strengthLabel(4), isNull);
      expect(strengthLabel(7), isNull);
      expect(strengthLabel(-1), isNull);
      expect(strengthLabel(null), isNull);
    });

    test('无障碍读法说人话（不能只念罗马数字）', () {
      expect(strengthSemanticLabel(1), '强度 轻');
      expect(strengthSemanticLabel(2), '强度 中');
      expect(strengthSemanticLabel(3), '强度 强');
      expect(strengthSemanticLabel(5), isNull);
    });
  });

  group('fromFrame 是纯投影：不改帧、可重复调用', () {
    test('两次投影结果一致，且原帧字段未被改动', () {
      final ActionStateEvent frame =
          parseWsFrame(kRealPerform)! as ActionStateEvent;
      final ActionEvent a = ActionEvent.fromFrame(frame, receivedAt: null);
      final ActionEvent b = ActionEvent.fromFrame(frame, receivedAt: null);
      expect(a.kind, b.kind);
      expect(a.action, b.action);
      expect(a.source, b.source);
      expect(frame.action, 'surprise', reason: '投影不得消费/清空原帧');
      expect(frame.source, ActionSource.ruleFallback);
    });
  });

  group('目录 JSON 形状（夹具取自本机 GET /api/v1/commands）', () {
    test('一条指令的字段都能读出来', () {
      final Map<String, Object?> one =
          jsonDecode(kRealCommandJson) as Map<String, Object?>;
      expect(one['id'], 'live2d_perform_action_nod');
      expect(one['name'], '点头');
      expect(one['target_action'], 'nod');
      expect(one['allowed_sources'], <Object?>['user_command']);
    });
  });
}

/// **真实抓包**的一条 `GET /api/v1/commands` 条目（本机 v0.4.6，2026-09-10）。
const String kRealCommandJson = '''
{"id":"live2d_perform_action_nod","name":"点头",
"description":"表演点头动作（同意 / 应和）",
"params_schema":{"additionalProperties":false,"properties":{"strength":
{"default":2,"description":"动作强度 1..=3（默认 2）","maximum":3,
"minimum":1,"type":"integer"}},"type":"object"},
"allowed_sources":["user_command"],"kind":"single","target_action":"nod",
"step_count":1}
''';
