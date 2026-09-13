import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/chat/turn_liveness.dart';

/// **一轮的生命周期判据**回归（纯逻辑，VM 直接跑）。
///
/// 来历（2026-09-11，用户报「第一次对话之后没输出了」）：临时拼起来的
/// `if (bubble.text.isEmpty) { if (!failed) { sessions.remove(bubble); ... } }`
/// 让一个**正常完成、只是没有文字**的回合在界面上彻底消失。而
/// `chat_controller.dart` 在 VM 测试里**加载不了**（经 `ws_client.dart` 依赖
/// `package:web`），所以这段逻辑必须在纯模块里才测得到——这就是本文件存在的
/// 理由，也是它必须覆盖「空气泡 ≠ 删掉」这条回归的理由。
void main() {
  _reasoningOnlyTests();
  group('settleTurn：一轮结束时气泡怎么收口', () {
    test('有文字 → 原样保留', () {
      expect(
        settleTurn(failed: false, text: '好呀'),
        TurnSettlement.keep,
      );
    });

    test('**没失败、但一个字都没有 → 系统提示（不是删掉）**', () {
      // 这条就是 2026-09-11 那个 bug 的钉子。实测复现：发「点点头。」→
      // 零文字、零音频、`turn_state{completed}`、后端无任何错误。
      expect(
        settleTurn(failed: false, text: ''),
        TurnSettlement.wordless,
        reason: '没有文字的回合**必须留下看得见的东西**——'
            '静默消失与「应用坏了」无法区分',
      );
    });

    test('失败且没有文字 → 失败气泡', () {
      expect(settleTurn(failed: true, text: ''), TurnSettlement.failed);
    });

    test('失败但有半句话 → 保留那半句（真实收到的内容不许丢）', () {
      expect(settleTurn(failed: true, text: '说到一半'), TurnSettlement.keep);
    });

    test('空串与纯空白一样按「没有文字」处理', () {
      // 只有 `' '` 的白气泡同样不该被当成回复。
      expect(settleTurn(failed: false, text: ' '), TurnSettlement.wordless);
    });

    test('**用户主动停止 + 没有文字 → 停止（不是「模型没返回文字」）**', () {
      expect(
        settleTurn(failed: false, text: '', stopped: true),
        TurnSettlement.stopped,
        reason: '把用户自己按的停止说成模型的产出，是另一种「伪造」',
      );
    });

    test('停止前已经收到的半句话 → 保留（真实内容不许丢）', () {
      expect(
        settleTurn(failed: false, text: '说到一半', stopped: true),
        TurnSettlement.keep,
      );
    });

    test('失败优先于停止（有码可查比「已停止」有用）', () {
      expect(
        settleTurn(failed: true, text: '', stopped: true),
        TurnSettlement.failed,
      );
    });
  });

  group('mustReleaseTurnOnWsLoss：断连时进行中的一轮必须收口', () {
    test('进行中 + disconnected → 收口', () {
      expect(
        mustReleaseTurnOnWsLoss(
          turnInFlight: true,
          status: WsStatus.disconnected,
        ),
        isTrue,
      );
    });

    test('进行中 + closed → 收口', () {
      expect(
        mustReleaseTurnOnWsLoss(turnInFlight: true, status: WsStatus.closed),
        isTrue,
      );
    });

    test('**进行中 + connecting 不收口**（发送瞬间通道可能还在建连）', () {
      expect(
        mustReleaseTurnOnWsLoss(turnInFlight: true, status: WsStatus.connecting),
        isFalse,
        reason: 'connecting 也不可用，但它是「正在建连」——'
            '此刻把这一轮收掉会把随后到达的回复判成孤儿',
      );
    });

    test('进行中 + connected 不收口', () {
      expect(
        mustReleaseTurnOnWsLoss(turnInFlight: true, status: WsStatus.connected),
        isFalse,
      );
    });

    test('没有进行中的一轮时，断连不动任何状态', () {
      expect(
        mustReleaseTurnOnWsLoss(
          turnInFlight: false,
          status: WsStatus.disconnected,
        ),
        isFalse,
      );
    });
  });

  group('提示文案与错误码是常量（界面与日志拿同一个字符串）', () {
    test('无文字提示只陈述事实，**不提工具/动作**', () {
      expect(kWordlessTurnNotice, contains('没有返回文字'));
      // 2026-09-11：LLM 已裁定为无工具、只做对话，原括号
      // 「（可能只触发了动作）」既解释不了现象，又让用户以为还有个动作系统
      // 在跑，已删除。这两条断言挡住它被加回来。
      expect(kWordlessTurnNotice, isNot(contains('动作')));
      expect(kWordlessTurnNotice, isNot(contains('工具')));
    });

    test('通道中断的码与文案都在', () {
      expect(kWsDroppedMidTurnCode, 'ws_dropped_mid_turn');
      expect(kWsDroppedMidTurnMessage, isNotEmpty);
    });

    test('「已停止」与「模型没返回文字」**必须是两句不同的话**', () {
      expect(kStoppedTurnNotice, isNotEmpty);
      expect(
        kStoppedTurnNotice,
        isNot(kWordlessTurnNotice),
        reason: '两者都是「没有文字的收口」，但原因一个是用户的意志、'
            '一个是模型的产出——写成同一句就是张冠李戴',
      );
    });

    test('「上一轮被新代次作废」的码与文案都在', () {
      expect(kTurnPreemptedCode, 'turn_preempted');
      expect(kTurnPreemptedMessage, isNotEmpty);
    });
  });
}

// ── 只有思考、没有正文（2026-09-13，推理模型实测）────────────────────
//
// 现场：`deepseek-flash` 的思考与正文共用输出预算，512 上限时正文被挤成
// 25 字且以 `\sqrt{a` 收尾——切不出完整句，于是**一个字都不上屏**。
// 这时气泡上其实有思考可看，所以不能按「什么都没有」处理（那会把用户
// 最想看的内容扔掉）。
void _reasoningOnlyTests() {
  group('只有思考的收口', () {
    test('没有正文但有思考 → keepReasoningOnly（保留气泡）', () {
      expect(
        settleTurn(failed: false, text: '', hasReasoning: true),
        TurnSettlement.keepReasoningOnly,
      );
    });

    test('连思考都没有 → 仍然是 wordless（换成系统行）', () {
      expect(
        settleTurn(failed: false, text: ''),
        TurnSettlement.wordless,
      );
    });

    test('有正文时思考不改变结论 → keep', () {
      expect(
        settleTurn(failed: false, text: '你好。', hasReasoning: true),
        TurnSettlement.keep,
      );
    });

    test('失败/用户停止优先于「只有思考」', () {
      expect(
        settleTurn(failed: true, text: '', hasReasoning: true),
        TurnSettlement.failed,
      );
      expect(
        settleTurn(failed: false, text: '', hasReasoning: true, stopped: true),
        TurnSettlement.stopped,
      );
    });

    test('空白正文（只有空格）也按「没有正文」处理', () {
      expect(
        settleTurn(failed: false, text: '   \n ', hasReasoning: true),
        TurnSettlement.keepReasoningOnly,
      );
    });

    test('说明行必须同时给出「发生了什么」与「下一步」', () {
      expect(kReasoningOnlyCaption, contains('思考'));
      expect(kReasoningOnlyCaption, contains('没有正文'));
      // 只说「没有返回文字」会把用户送回原来那个坑：不知道去哪改。
      expect(kReasoningOnlyCaption, contains('输出 token 上限'));
    });
  });
}
