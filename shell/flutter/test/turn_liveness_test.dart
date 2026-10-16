import 'dart:io';

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
  _userLineTests();
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

  _llmFaultVisibilityTests();
}

/// **LLM 故障可见**（2026-09-23，用户报「LLM 故障时对话无反应」）。
///
/// 这两条判据都住在纯模块里，所以 VM 直接跑得到——它们是「界面不能一直转圈 /
/// 迟到的旧错不能收掉新一轮」的唯一回归。
void _llmFaultVisibilityTests() {
  group('mustSettleTurnOnError：致命错误兜底收口', () {
    test('致命错误 + 本轮在飞 → 立刻收口', () {
      expect(mustSettleTurnOnError(turnInFlight: true, fatal: true), isTrue);
    });

    test('非致命 LLM 失败不收口（已生成的语音还要播完）', () {
      expect(mustSettleTurnOnError(turnInFlight: true, fatal: false), isFalse);
    });

    test('没有在飞的一轮时收到错误 → 无事可收', () {
      expect(mustSettleTurnOnError(turnInFlight: false, fatal: true), isFalse);
    });
  });

  group('frameBelongsToCurrentTurn：迟到的旧轮帧必须丢弃', () {
    test('代次一致 → 属于当前轮', () {
      expect(frameBelongsToCurrentTurn(frameEpoch: 7, currentEpoch: 7), isTrue);
    });

    test('代次不同 → 迟到帧，丢弃', () {
      expect(frameBelongsToCurrentTurn(frameEpoch: 7, currentEpoch: 8), isFalse);
      expect(frameBelongsToCurrentTurn(frameEpoch: 8, currentEpoch: 7), isFalse);
    });

    test('任一侧缺省（旧服务端 / 代次还没回填）→ 按当前轮处理', () {
      expect(frameBelongsToCurrentTurn(frameEpoch: null, currentEpoch: 7), isTrue);
      expect(frameBelongsToCurrentTurn(frameEpoch: 7, currentEpoch: null), isTrue);
      expect(frameBelongsToCurrentTurn(frameEpoch: null, currentEpoch: null), isTrue);
    });
  });

  group('失败兜底：禁止「无字无错」', () {
    test('失败但没有 error 帧 → 需要兜底码', () {
      expect(needsFailureFallbackCode(failed: true, hasError: false), isTrue);
    });

    test('已经有 error 帧 → 不覆盖更全的信息', () {
      expect(needsFailureFallbackCode(failed: true, hasError: true), isFalse);
    });

    test('成功轮不需要任何兜底码', () {
      expect(needsFailureFallbackCode(failed: false, hasError: false), isFalse);
    });

    test('兜底码与文案是稳定契约', () {
      expect(kTurnFailedWithoutDetailCode, 'turn_failed_no_detail');
      expect(kTurnFailedWithoutDetailMessage, contains('诊断日志'));
    });
  });

  group('text_fallback 的落点：同一轮只留一个助手气泡（T1，2026-10-13）', () {
    FallbackTarget target({
      bool current = false,
      bool settled = false,
      bool inSession = true,
      int? frameEpoch = 7,
      int? settledEpoch = 7,
    }) => fallbackTarget(
      hasCurrentAssistant: current,
      hasSettledAssistant: settled,
      settledInSession: inSession,
      frameEpoch: frameEpoch,
      settledEpoch: settledEpoch,
    );

    test('本轮还在飞 → 写回当前气泡（既有行为不变）', () {
      expect(target(current: true), FallbackTarget.current);
      // 即使刚收口过一条，只要有当前气泡就写当前那条。
      expect(target(current: true, settled: true), FallbackTarget.current);
    });

    test('本轮已收口、代次对得上 → 写回那一条，不 append 第二条', () {
      expect(target(settled: true), FallbackTarget.settled);
    });

    test('迟到的旧轮 fallback（代次不符）→ 不写回，另起一条', () {
      expect(
        target(settled: true, frameEpoch: 8, settledEpoch: 7),
        FallbackTarget.fresh,
      );
    });

    test('那条气泡已被换成系统行（不在会话里）→ 另起一条', () {
      expect(target(settled: true, inSession: false), FallbackTarget.fresh);
    });

    test('什么都没有（服务端主动开口 / 帧先于 HTTP 响应）→ 新建', () {
      expect(target(), FallbackTarget.fresh);
    });

    /// 接线（源码扫描）：判据必须真的被 chat_controller 用上——那个文件经
    /// ws_client.dart 依赖 package:web，VM 测试里加载不了（本文件的存在理由）。
    test('chat_controller 用 fallbackTarget 决定落点，并记住收口后的气泡', () {
      final String src = File('lib/chat/chat_controller.dart').readAsStringSync();
      expect(src.contains('fallbackTarget('), isTrue, reason: '落点判据必须来自纯函数');
      expect(
        src.contains('_settledAssistant'),
        isTrue,
        reason: '收口后的那条气泡必须被记住（否则 text_fallback 会再 append 一条）',
      );
      expect(
        src.contains('FallbackTarget.settled'),
        isTrue,
        reason: '收口分支必须写回那一条，而不是新建',
      );
      // 登记 1 处 + send / 注入轮 / abandon / 两个系统行分支各自清 —— 至少要 5 处。
      expect(
        RegExp(r'_settledAssistant = null;').allMatches(src).length,
        greaterThanOrEqualTo(5),
        reason: '新一轮、作废本轮、被换成系统行时都必须清掉写回对象',
      );
    });
  });
}

// ── `user_line` 的判据（2026-10-14 执行单 §2）───────────────────────
//
// 弹幕 sidecar 直接 POST `/api/v1/external/chat`，**浏览器没参加那次发送**，
// 于是主链跑了、历史里却没有那一句。服务端在「say 被取到、投递 TurnPrompt
// 的同一处」补广播 `user_line`，前端据此写进当前会话：**以「弹幕消息：」开头
// 的写系统行**（不是操作者说的那一句），其余仍走原来的用户句三分支。判据住在
// 这里（纯逻辑）：错了不会报错，只会静默漏一句、把弹幕写成「你」、或作废在飞那轮。
void _userLineTests() {
  group('userLineAction：user_line 的判据（弹幕 / 用户句 / 空文本）', () {
    // 非弹幕的 user_line（语音 / 输入框那条回声）——用户句三分支只对它成立。
    const String injected = '操作者说的一句';
    // 弹幕：正文 = 「前缀 + 原文」（2026-10-14 执行单 §0）。
    const String danmaku = '弹幕消息：你好';

    test('① 没有一轮在飞 → acceptInjected（复用 acceptInjectedUserTurn）', () {
      expect(
        userLineAction(
          turnInFlight: false,
          text: injected,
          lastUserText: null,
          lastMessageText: null,
        ),
        UserLineAction.acceptInjected,
      );
      // 上一条用户句是**别的**内容也照旧：没有轮在飞，这一句就是新一轮的输入。
      expect(
        userLineAction(
          turnInFlight: false,
          text: injected,
          lastUserText: '上一句',
          lastMessageText: null,
        ),
        UserLineAction.acceptInjected,
      );
    });

    test('② 在飞 + 上一条用户句就是这句 → skipDuplicate（别写两遍）', () {
      expect(
        userLineAction(
          turnInFlight: true,
          text: injected,
          lastUserText: injected,
          lastMessageText: null,
        ),
        UserLineAction.skipDuplicate,
        reason: '输入框发送与语音已经上屏过这一句；再写一次就是同一句话说两遍',
      );
    });

    test('② 的相等按 trim 判：只差首尾空白仍是同一句', () {
      expect(
        userLineAction(
          turnInFlight: true,
          text: '  $injected  ',
          lastUserText: injected,
          lastMessageText: null,
        ),
        UserLineAction.skipDuplicate,
      );
    });

    test('③ 在飞 + 另一句 → appendOnly（只追加气泡，不动在飞那一轮）', () {
      expect(
        userLineAction(
          turnInFlight: true,
          text: '另一句',
          lastUserText: injected,
          lastMessageText: null,
        ),
        UserLineAction.appendOnly,
      );
      // 会话里还没有任何用户句（例如服务端主动开口那一轮）：仍然只追加，不作废。
      expect(
        userLineAction(
          turnInFlight: true,
          text: injected,
          lastUserText: null,
          lastMessageText: null,
        ),
        UserLineAction.appendOnly,
      );
    });

    test('④ 空文本 / 纯空白 / 缺 text → ignoreEmpty（不看在不在飞）', () {
      for (final String? body in <String?>[null, '', '   ', '\n\t ']) {
        expect(
          userLineAction(
            turnInFlight: false,
            text: body,
            lastUserText: null,
            lastMessageText: null,
          ),
          UserLineAction.ignoreEmpty,
          reason: '空文本不写（body=$body）',
        );
        expect(
          userLineAction(
            turnInFlight: true,
            text: body,
            lastUserText: '别的',
            lastMessageText: null,
          ),
          UserLineAction.ignoreEmpty,
          reason: '在飞也不写空话（body=$body）',
        );
      }
    });

    test('重复判据：在飞 + 与上一条用户句相同 → 跳过（语音 HTTP 回来时不写第二遍）', () {
      expect(
        isDuplicateInjectedUserLine(
          streaming: true,
          text: '操作者说的一句',
          lastUserText: '操作者说的一句',
        ),
        isTrue,
        reason: 'user_line 先到、这句已经上屏；再 append 一次历史里就有两句',
      );
      // 与输入框 / 语音上屏前的裁剪一致：只差首尾空白仍算同一句。
      expect(
        isDuplicateInjectedUserLine(
          streaming: true,
          text: '  同一句  ',
          lastUserText: '同一句',
        ),
        isTrue,
      );
    });

    test('重复判据：不在飞 + 与上一条相同 → 仍接受（用户又把同一句说了一遍）', () {
      expect(
        isDuplicateInjectedUserLine(
          streaming: false,
          text: '同一句',
          lastUserText: '同一句',
        ),
        isFalse,
        reason: '不在飞 = 上一轮已经结束；这是新的一轮，历史里必须有第二句',
      );
      // 在飞但正文不同 / 根本没有上一条 → 都照旧接受。
      expect(
        isDuplicateInjectedUserLine(
          streaming: true,
          text: '新的一句',
          lastUserText: '旧的一句',
        ),
        isFalse,
      );
      expect(
        isDuplicateInjectedUserLine(
          streaming: true,
          text: '新的一句',
          lastUserText: null,
        ),
        isFalse,
      );
    });

    test('弹幕（`弹幕消息：` 前缀）→ appendSystem：系统行，不是操作者说的那一句', () {
      expect(
        userLineAction(
          turnInFlight: false,
          text: danmaku,
          lastUserText: null,
          lastMessageText: null,
        ),
        UserLineAction.appendSystem,
      );
      // 已有一轮在飞也一样：弹幕仍写系统行，不会变成 acceptInjected / appendOnly。
      expect(
        userLineAction(
          turnInFlight: true,
          text: danmaku,
          lastUserText: null,
          lastMessageText: '助手刚回的一句',
        ),
        UserLineAction.appendSystem,
      );
    });

    test('弹幕：同一句已经是最后一条 → skipDuplicate（不写两遍）', () {
      expect(
        userLineAction(
          turnInFlight: true,
          text: danmaku,
          lastUserText: null,
          lastMessageText: danmaku,
        ),
        UserLineAction.skipDuplicate,
      );
      // 最后一条是**别的话**（例如助手回复）→ 这是一条新弹幕，照写。
      expect(
        userLineAction(
          turnInFlight: false,
          text: danmaku,
          lastUserText: null,
          lastMessageText: '助手刚回的一句',
        ),
        UserLineAction.appendSystem,
      );
    });

    test('非弹幕不受影响：lastMessageText 不参与用户句的三分支', () {
      // 最后一条恰好同文，但这不是弹幕 → 仍是「没轮在飞 → acceptInjected」。
      expect(
        userLineAction(
          turnInFlight: false,
          text: injected,
          lastUserText: null,
          lastMessageText: injected,
        ),
        UserLineAction.acceptInjected,
      );
      // 前缀只在**开头**才算：句中出现不算弹幕。
      expect(
        userLineAction(
          turnInFlight: true,
          text: '我说弹幕消息：你好',
          lastUserText: null,
          lastMessageText: null,
        ),
        UserLineAction.appendOnly,
      );
    });

    test('弹幕前缀是常量、全角冒号（服务端 / sidecar 同一串）', () {
      expect(kDanmakuPrefix, '弹幕消息：');
      expect(
        kDanmakuPrefix.endsWith('：'),
        isTrue,
        reason: '全角冒号；写成 ASCII 冒号就对不上服务端',
      );
    });

    /// 接线（源码扫描）：跳过必须发生在 `_abandonTurn` **之前**——顺序反了就是
    /// 先把在飞的那一轮作废、再补一条，正是这次要修的现场。
    test('acceptInjectedUserTurn：重复判据排在 _abandonTurn 之前', () {
      final String src = File('lib/chat/chat_controller.dart').readAsStringSync();
      final int start = src.indexOf('bool acceptInjectedUserTurn(');
      expect(start, greaterThan(0), reason: '注入入口必须还在控制器里');
      final int end = src.indexOf('\n  }\n', start);
      final String body = src.substring(start, end < 0 ? src.length : end);
      expect(
        body.contains('isDuplicateInjectedUserLine('),
        isTrue,
        reason: '判据必须来自 turn_liveness.dart 的纯函数',
      );
      expect(
        body.indexOf('isDuplicateInjectedUserLine(') <
            body.indexOf('_abandonTurn()'),
        isTrue,
        reason: '顺序反了 = 先把在飞的轮作废再补一条（本次要修的现场）',
      );
    });

    /// 接线（源码扫描）：弹幕分支必须写 `ChatRole.system`、**不**调用
    /// `acceptInjectedUserTurn`，且不碰在飞的那一轮（不 _abandonTurn /
    /// 不清助手气泡 / 不关流式态）。
    test('chat_controller：appendSystem = 系统行 + 落盘，不动在飞那一轮', () {
      final String src = File('lib/chat/chat_controller.dart').readAsStringSync();
      expect(
        src.contains('UserLineAction.appendSystem'),
        isTrue,
        reason: '弹幕分支必须在控制器里有落点',
      );
      expect(
        src.contains('lastMessageText:'),
        isTrue,
        reason: '必须把「最后一条正文」交给判据，否则同一句弹幕会写两遍',
      );
      final int start = src.indexOf('case UserLineAction.appendSystem:');
      expect(start, greaterThan(0), reason: '弹幕分支的 case 必须还在');
      final int next = src.indexOf('case UserLineAction.', start + 10);
      final String body = src.substring(start, next < 0 ? src.length : next);
      expect(body.contains('ChatRole.system'), isTrue, reason: '弹幕 = 系统行');
      expect(
        body.contains('acceptInjectedUserTurn'),
        isFalse,
        reason: '弹幕不许写成操作者说的那一句',
      );
      expect(body.contains('_abandonTurn'), isFalse);
      expect(body.contains('_assistant = null'), isFalse);
      expect(body.contains('_streaming = false'), isFalse);
      expect(body.contains('_persist()'), isTrue, reason: '落盘（刷新后还在）');
    });

    /// 接线（源码扫描）：判据必须真的被 `chat_controller` 用上——那个文件经
    /// `ws_client.dart` 依赖 `package:web`，VM 测试里加载不了（本文件的存在理由）。
    test('chat_controller 用 userLineAction，且分支③不动在飞的那一轮', () {
      final String src = File('lib/chat/chat_controller.dart').readAsStringSync();
      expect(src.contains('userLineAction('), isTrue, reason: '判据必须来自纯函数');
      expect(
        src.contains('UserLineAction.acceptInjected'),
        isTrue,
        reason: '分支①必须复用 acceptInjectedUserTurn',
      );
      expect(
        src.contains('UserLineAction.appendOnly'),
        isTrue,
        reason: '分支③必须在控制器里有落点',
      );
      // 分支③的**实现体**只准追加 + 落盘：不得作废在飞的一轮、不得清助手气泡、
      // 不得把 _streaming 弄成 false。
      final int start = src.indexOf('void _onUserLine(');
      expect(start, greaterThan(0), reason: 'user_line 的入口必须还在控制器里');
      final int end = src.indexOf('\n  }\n', start);
      final String body = src.substring(start, end < 0 ? src.length : end);
      expect(body.contains('_abandonTurn'), isFalse, reason: '不许在飞一轮被作废');
      expect(body.contains('_assistant = null'), isFalse, reason: '不许清当前助手气泡');
      expect(body.contains('_streaming = false'), isFalse, reason: '不许把流式态关掉');
      expect(body.contains('sessions.append('), isTrue, reason: '分支③要真的追加气泡');
      expect(body.contains('_persist()'), isTrue, reason: '分支③要落盘（刷新后还在）');
    });
  });
}
