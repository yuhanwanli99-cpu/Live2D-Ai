import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/actions/action_event.dart';
import 'package:live2d_ai_shell/actions/action_history.dart';
import 'package:live2d_ai_shell/api/action_source.dart';

/// 固定时刻的工厂（时间戳断言必须确定，否则只能断言「不为 null」= 没测到）。
DateTime at(int second) => DateTime(2026, 9, 10, 20, 0, second);

ActionEvent perform({
  required String action,
  ActionSource source = ActionSource.llmTool,
  int? strength,
  int epoch = 1,
  int second = 0,
  int? seq,
}) => ActionEvent(
  kind: ActionEventKind.perform,
  epoch: epoch,
  action: action,
  strength: strength,
  source: source,
  seq: seq,
  receivedAt: at(second),
);

ActionEvent cease({int epoch = 1, int second = 0}) => ActionEvent(
  kind: ActionEventKind.cease,
  epoch: epoch,
  receivedAt: at(second),
);

void main() {
  group('perform / cease 的基本语义', () {
    test('perform 建一条，并成为 current', () {
      final ActionHistory h = ActionHistory();
      expect(h.consume(perform(action: 'nod', strength: 2, second: 1)), isTrue);
      expect(h.entries, hasLength(1));
      expect(h.entries.single.action, 'nod');
      expect(h.entries.single.label, 'nod', reason: '无目录时显示协议名，不编造');
      expect(h.entries.single.source, ActionSource.llmTool);
      expect(h.entries.single.strength, 2);
      expect(h.entries.single.isPerforming, isTrue);
      expect(h.current, same(h.entries.single));
    });

    test('cease **收口当前条目，不新建条目**', () {
      final ActionHistory h = ActionHistory();
      h.consume(perform(action: 'nod', second: 1));
      expect(h.consume(cease(second: 3)), isTrue);
      expect(h.entries, hasLength(1), reason: 'cease 不是「又发生了一件事」');
      expect(h.entries.single.isPerforming, isFalse);
      expect(h.entries.single.endedAt, at(3));
      expect(h.entries.single.superseded, isFalse, reason: '正常收口不是抢占');
      expect(h.current, isNull);
    });

    test('没有未收口动作时的 cease 是 no-op（不建条目、不报错）', () {
      final ActionHistory h = ActionHistory();
      expect(h.consume(cease()), isFalse);
      expect(h.entries, isEmpty);
      expect(h.current, isNull);
    });

    test('cease 幂等：重复收口不改变已有结束时刻', () {
      final ActionHistory h = ActionHistory();
      h.consume(perform(action: 'nod', second: 1));
      h.consume(cease(second: 3));
      expect(h.consume(cease(second: 9)), isFalse, reason: '已收口 → 无变化');
      expect(h.entries.single.endedAt, at(3));
    });

    test('epoch 不匹配的 cease 不收口（那是另一轮的信号）', () {
      final ActionHistory h = ActionHistory();
      h.consume(perform(action: 'nod', epoch: 4, second: 1));
      expect(h.consume(cease(epoch: 3, second: 2)), isFalse);
      expect(h.current, isNotNull);
      expect(h.entries.single.isPerforming, isTrue);
    });
  });

  group('抢占：core 不会先发 cease（这是最容易漏的一条）', () {
    test('新 perform 到来时，上一条被标为 superseded 并收口', () {
      final ActionHistory h = ActionHistory();
      h.consume(perform(action: 'nod', source: ActionSource.llmTool, second: 1));
      h.consume(perform(action: 'shake_no', source: ActionSource.userCommand, second: 2));

      expect(h.entries, hasLength(2));
      final ActionHistoryEntry first = h.entries[0];
      expect(first.action, 'nod');
      expect(first.isPerforming, isFalse, reason: '被顶掉后不能再显示「正在演」');
      expect(first.superseded, isTrue);
      expect(first.endedAt, at(2), reason: '结束时刻 = 新动作到达时刻');
      expect(h.current!.action, 'shake_no', reason: '游标必须指向新动作');
    });

    test('连续抢占不留下僵尸游标（current 永远在 entries 里）', () {
      final ActionHistory h = ActionHistory();
      for (int i = 0; i < 20; i++) {
        h.consume(perform(action: 'a$i', second: i));
        expect(h.current, isNotNull);
        expect(h.entries.contains(h.current), isTrue, reason: '第 $i 次');
      }
      expect(h.entries.where((ActionHistoryEntry e) => e.isPerforming), hasLength(1));
    });
  });

  group('不可展示的事件不进列表', () {
    test('kind == unknown', () {
      final ActionHistory h = ActionHistory();
      expect(
        h.consume(
          ActionEvent(kind: ActionEventKind.unknown, epoch: 1, receivedAt: at(0)),
        ),
        isFalse,
      );
      expect(h.entries, isEmpty);
    });

    test('perform 但没有动作名', () {
      final ActionHistory h = ActionHistory();
      expect(
        h.consume(
          ActionEvent(
            kind: ActionEventKind.perform,
            epoch: 1,
            action: null,
            source: ActionSource.llmTool,
            receivedAt: at(0),
          ),
        ),
        isFalse,
      );
      expect(h.entries, isEmpty);
    });
  });

  group('中文名解析器：注入，不内建', () {
    test('注入目录后 label 是中文', () {
      final ActionHistory h = ActionHistory(
        labelOf: (String a) => const <String, String>{'nod': '点头'}[a] ?? a,
      );
      h.consume(perform(action: 'nod'));
      h.consume(perform(action: 'custom_fancy'));
      expect(h.entries[0].label, '点头');
      expect(h.entries[1].label, 'custom_fancy', reason: '目录里没有就回协议名');
    });

    test('useLabels 之后**不回溯改名**（历史是「当时看到了什么」）', () {
      final ActionHistory h = ActionHistory();
      h.consume(perform(action: 'nod', second: 1));
      expect(h.entries.single.label, 'nod');
      h.useLabels((String a) => '点头');
      expect(h.entries.single.label, 'nod', reason: '旧行不改名');
      h.consume(perform(action: 'nod', second: 2));
      expect(h.entries.last.label, '点头', reason: '新行走新目录');
    });
  });

  group('有界列表：长期运行不许无限增长', () {
    test('超过 maxEntries 丢最旧的，并计数（不静默）', () {
      final ActionHistory h = ActionHistory(maxEntries: 3);
      for (int i = 0; i < 5; i++) {
        h.consume(perform(action: 'a$i', second: i));
      }
      expect(h.entries, hasLength(3));
      expect(h.entries.map((ActionHistoryEntry e) => e.action), <String>['a2', 'a3', 'a4']);
      expect(h.droppedCount, 2);
      expect(h.current!.action, 'a4');
    });

    test('被淘汰的正好是 current 时，游标一并清掉（不留悬空引用）', () {
      // maxEntries=1：每来一条，上一条（也就是 current）立刻被淘汰。
      final ActionHistory h = ActionHistory(maxEntries: 1);
      h.consume(perform(action: 'a0', second: 1));
      // 先收口，让 current 不为「正在演」；再灌入新条目把旧的挤掉。
      h.consume(perform(action: 'a1', second: 2));
      expect(h.entries, hasLength(1));
      expect(h.current, same(h.entries.single));
      expect(h.entries.single.action, 'a1');
    });

    test('默认上限是个具体数字（防止有人改成无限）', () {
      expect(ActionHistory.defaultMaxEntries, 500);
      final ActionHistory h = ActionHistory();
      expect(h.maxEntries, 500);
    });
  });

  group('通知与清空', () {
    test('有效事件触发一次通知；无效事件不通知', () {
      final ActionHistory h = ActionHistory();
      int notified = 0;
      h.addListener(() => notified++);

      h.consume(perform(action: 'nod'));
      expect(notified, 1);
      // `perform` 但没有动作名 → 不可展示，不该引起重建。
      h.consume(
        ActionEvent(
          kind: ActionEventKind.perform,
          epoch: 1,
          source: ActionSource.llmTool,
          receivedAt: at(1),
        ),
      );
      expect(notified, 1, reason: '不可展示的事件不该引起重建');
      h.consume(cease());
      expect(notified, 2);
      h.consume(cease());
      expect(notified, 2, reason: '空收口是 no-op');
    });

    test('clear 清空一切并通知；空列表上 clear 不通知', () {
      final ActionHistory h = ActionHistory();
      h.consume(perform(action: 'nod'));
      int notified = 0;
      h.addListener(() => notified++);
      h.clear();
      expect(h.entries, isEmpty);
      expect(h.current, isNull);
      expect(h.droppedCount, 0);
      expect(notified, 1);
      h.clear();
      expect(notified, 1, reason: '已经空了就不该再通知');
    });
  });

  group('条目本身', () {
    test('end 幂等，且不丢字段', () {
      final ActionHistoryEntry e = ActionHistoryEntry(
        action: 'nod',
        label: '点头',
        source: ActionSource.userCommand,
        epoch: 7,
        strength: 3,
        at: at(1),
        seq: 42,
      );
      final ActionHistoryEntry ended = e.end(at(2));
      expect(ended.endedAt, at(2));
      expect(ended.action, 'nod');
      expect(ended.label, '点头');
      expect(ended.epoch, 7);
      expect(ended.strength, 3);
      expect(ended.seq, 42);
      expect(ended.isPerforming, isFalse);
      expect(ended.end(at(9)).endedAt, at(2), reason: '重复 end 不覆盖');
    });

    test('end 传 null → 回落 at（不写 null 结束时刻）', () {
      final ActionHistoryEntry e = ActionHistoryEntry(
        action: 'nod',
        label: '点头',
        source: ActionSource.userCommand,
        epoch: 1,
        at: at(1),
      );
      expect(e.end(null).endedAt, at(1));
    });

    test('semanticLabel 是一句完整的话（读屏不能念碎片）', () {
      final ActionHistoryEntry e = ActionHistoryEntry(
        action: 'nod',
        label: '点头',
        source: ActionSource.llmTool,
        epoch: 1,
        strength: 2,
      );
      expect(e.semanticLabel, '点头，来源AI，强度 中，正在表演');
      expect(e.end(at(9)).semanticLabel, '点头，来源AI，强度 中');
    });

    test('强度标签：越界为 null（不显示假的档位）', () {
      ActionHistoryEntry withStrength(int? s) => ActionHistoryEntry(
        action: 'nod',
        label: '点头',
        source: ActionSource.llmTool,
        epoch: 1,
        strength: s,
      );
      expect(withStrength(2).strengthText, 'Ⅱ');
      expect(withStrength(9).strengthText, isNull);
      expect(withStrength(null).strengthText, isNull);
    });
  });

  group('真实帧端到端：WS 原文 → 历史列表', () {
    test('perform(llm_tool) → 历史里出现「AI」徽标的数据（P2 验收）', () {
      final ActionHistory h = ActionHistory(
        labelOf: (String a) => const <String, String>{'nod': '点头'}[a] ?? a,
      );
      // 这一帧就是 P2 要兑现的东西：core 一直在发 source，前端曾经把它丢掉。
      h.consume(_llmToolPerform(second: 5));
      expect(h.entries, hasLength(1));
      final ActionHistoryEntry e = h.entries.single;
      expect(e.label, '点头');
      expect(e.source, ActionSource.llmTool);
      expect(e.source.label, 'AI');
      expect(e.source.isAiInitiated, isTrue);
      expect(e.strengthText, 'Ⅱ');
      expect(e.epoch, 4);
    });

    test('perform(rule_fallback) → 「规则」而不是「AI」', () {
      final ActionHistory h = ActionHistory();
      h.consume(
        ActionEvent(
          kind: ActionEventKind.perform,
          epoch: 0,
          action: 'surprise',
          strength: 3,
          source: ActionSource.ruleFallback,
          receivedAt: at(6),
        ),
      );
      expect(h.entries.single.source.label, '规则');
    });
  });
}

ActionEvent _llmToolPerform({required int second}) => ActionEvent(
  kind: ActionEventKind.perform,
  epoch: 4,
  action: 'nod',
  strength: 2,
  source: ActionSource.llmTool,
  receivedAt: at(second),
);
