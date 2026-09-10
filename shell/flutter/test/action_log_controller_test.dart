import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/actions/action_event.dart';
import 'package:live2d_ai_shell/actions/action_history.dart';
import 'package:live2d_ai_shell/actions/action_log_controller.dart';
import 'package:live2d_ai_shell/api/action_source.dart';
import 'package:live2d_ai_shell/api/commands_api.dart';

/// **真实抓包**的帧原文（本机 v0.4.6，2026-09-10），与 `ws_frame_test.dart` 同源。
const String kPerformRule = '{"data":{"action":{"action":"surprise",'
    '"source":"rule_fallback","strength":3},"epoch":0,"kind":"perform"},'
    '"seq":566,"ts":"2026-09-10T12:14:53.166Z","type":"action_state"}';

const String kPerformUser = '{"data":{"action":{"action":"nod",'
    '"source":"user_command","strength":2},"epoch":0,"kind":"perform"},'
    '"seq":4,"ts":"2026-09-10T12:13:45.727Z","type":"action_state"}';

/// `perform{llm_tool}`：按 `ws/events.rs` 的序列化表手写（实抓那轮 LLM 走的是
/// 规则兜底）。**这一支是 P2 的验收条件**——AI 触发的动作必须显示「AI」。
const String kPerformLlm = '{"data":{"action":{"action":"tilt",'
    '"source":"llm_tool","strength":1},"epoch":2,"kind":"perform"},'
    '"seq":600,"ts":"2026-09-10T12:30:00.000Z","type":"action_state"}';

const String kCease = '{"data":{"epoch":2,"kind":"cease"},"seq":601,'
    '"ts":"2026-09-10T12:30:03.000Z","type":"action_state"}';

/// 真实目录（本机 `GET /api/v1/commands`，只留 P2 用得到的字段）。
const String kCatalogJson = '''
{"commands":[
 {"id":"live2d_perform_action_nod","name":"点头","target_action":"nod",
  "allowed_sources":["user_command"],"kind":"single","step_count":1},
 {"id":"live2d_perform_action_tilt","name":"歪头","target_action":"tilt",
  "allowed_sources":["user_command"],"kind":"single","step_count":1},
 {"id":"live2d_perform_action_surprise","name":"惊讶","target_action":"surprise",
  "allowed_sources":["user_command"],"kind":"single","step_count":1}
]}
''';

/// 一个**非** action_state 的真实帧（用来证明接线层不越界消费）。
const String kTextDelta = '{"data":{"epoch":0,"text":"你好",'
    '"ts_ms":100},"seq":3,"type":"text_delta"}';

DateTime at(int second) => DateTime(2026, 9, 10, 21, 0, second);

CommandCatalog catalog() =>
    CommandCatalog.fromJson(jsonDecode(kCatalogJson) as Map<String, Object?>);

void main() {
  group('帧原文 → 历史（这一条路径以前完全测不到）', () {
    test('perform{llm_tool} 一路走到「歪头 / AI / Ⅰ」', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      log.useCatalog(catalog());

      expect(log.consumeRaw(kPerformLlm), isTrue);

      expect(log.entries, hasLength(1));
      final ActionHistoryEntry e = log.entries.single;
      expect(e.label, '歪头', reason: '中文名来自服务端目录，不是内建表');
      expect(e.source, ActionSource.llmTool);
      expect(e.source.label, 'AI');
      expect(e.strengthText, 'Ⅰ');
      expect(e.at, at(1));
      expect(e.isPerforming, isTrue);
      expect(log.current, same(e));
    });

    test('perform{user_command} 显示「你」，且**不计未读**', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      log.useCatalog(catalog());

      log.consumeRaw(kPerformUser);

      expect(log.entries.single.label, '点头');
      expect(log.entries.single.source, ActionSource.userCommand);
      expect(
        log.unseenAiActions,
        0,
        reason: '用户自己点的动作不需要再告诉他一遍（规格 §8.3 第 3 处同理）',
      );
    });

    test('perform{rule_fallback} 显示「规则」并计未读（规则兜底也算 AI 侧）', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      log.useCatalog(catalog());

      log.consumeRaw(kPerformRule);

      expect(log.entries.single.label, '惊讶');
      expect(log.entries.single.source.label, '规则');
      expect(log.unseenAiActions, 1);
      expect(log.hasUnseenAiActions, isTrue);
    });

    test('cease 收口：不再有正在演的动作，也不新增行', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      log.useCatalog(catalog());
      log.consumeRaw(kPerformLlm);

      expect(log.consumeRaw(kCease), isTrue);

      expect(log.entries, hasLength(1), reason: 'cease 不是新条目');
      expect(log.entries.single.isPerforming, isFalse);
      expect(log.current, isNull);
    });
  });

  group('只消费 action_state：其它帧一律不碰', () {
    test('text_delta / heartbeat / 坏帧 → false，且不通知', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      int notified = 0;
      log.addListener(() => notified++);

      for (final String raw in <String>[
        kTextDelta,
        '{"type":"heartbeat","seq":9}',
        '{"type":"subscribe_ack","data":{"topics":[]}}',
        '{"type":"audio","data":{"audio":"AAAA"}}',
        'not json at all',
        '',
      ]) {
        expect(log.consumeRaw(raw), isFalse, reason: raw);
      }
      expect(notified, 0);
      expect(log.entries, isEmpty);
    });
  });

  group('未读计数：AI 动作转瞬即逝，得让人看得见', () {
    test('每来一个新的 AI 动作 +1；用户动作不加', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      log.consumeRaw(kPerformRule); // 规则 → +1
      expect(log.unseenAiActions, 1);
      log.consumeRaw(kPerformLlm); // LLM → +1
      expect(log.unseenAiActions, 2);
      log.consumeRaw(kPerformUser); // 用户 → 不变
      expect(log.unseenAiActions, 2);
    });

    test('markSeen 清零，且已读后不再重复通知', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      log.consumeRaw(kPerformRule);
      expect(log.unseenAiActions, 1);
      log.markSeen();
      expect(log.unseenAiActions, 0);
      expect(log.hasUnseenAiActions, isFalse);
      int notified = 0;
      log.addListener(() => notified++);
      log.markSeen();
      expect(notified, 0, reason: '本来就没有未读时不该通知');
    });

    test('同一动作的后续帧不重复计数（只认「刚开始表演」那一次）', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      log.consumeRaw(kPerformLlm);
      expect(log.unseenAiActions, 1);
      // 再来一个同帧（例如重连后重放）→ 是**新条目**，计 1 次；这是正确行为。
      log.consumeRaw(kPerformLlm);
      expect(log.entries, hasLength(2));
      expect(log.unseenAiActions, 2);
      // 但收口本身不该再加。
      log.consumeRaw(kCease);
      expect(log.unseenAiActions, 2);
    });

    test('clear 一并清未读', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      log.consumeRaw(kPerformRule);
      log.clear();
      expect(log.entries, isEmpty);
      expect(log.unseenAiActions, 0);
      expect(log.current, isNull);
    });
  });

  group('目录灌入', () {
    test('空目录不改解析器（继续显示协议名，不撒谎）', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      log.useCatalog(const CommandCatalog.empty());
      log.consumeRaw(kPerformLlm);
      expect(log.entries.single.label, 'tilt');
    });

    test('目录灌入后**新**条目用中文名，旧条目不回溯改名', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      log.consumeRaw(kPerformLlm);
      expect(log.entries.single.label, 'tilt');
      log.useCatalog(catalog());
      expect(log.entries.single.label, 'tilt', reason: '历史是「当时看到了什么」');
      log.consumeRaw(kPerformUser);
      expect(log.entries.last.label, '点头');
    });
  });

  group('通知语义（原则 P6：动作不该拖垮聊天列表）', () {
    test('接线层自己是唯一 listenable，且每个有效事件通知一次', () {
      final ActionLogController log = ActionLogController(now: () => at(1));
      int notified = 0;
      log.addListener(() => notified++);
      log.consumeRaw(kPerformLlm);
      expect(notified, 1);
      log.consumeRaw(kCease);
      expect(notified, 2);
      log.consumeRaw(kCease);
      expect(notified, 2, reason: '空收口是 no-op');
    });

    test('dispose 后不再转发底层 history 的变化（不留悬空监听）', () {
      final ActionHistory history = ActionHistory();
      final ActionLogController log = ActionLogController(history: history);
      log.dispose();
      // dispose 之后直接改底层 history：不该抛（监听已摘）。
      history.consume(
        ActionEvent(
          kind: ActionEventKind.perform,
          epoch: 1,
          action: 'nod',
          source: ActionSource.llmTool,
        ),
      );
      expect(history.entries, hasLength(1));
    });
  });
}
