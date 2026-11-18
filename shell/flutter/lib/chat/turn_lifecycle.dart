part of 'chat_controller.dart';

/// ChatController 的**轮次生命周期**：作废 / 收口 / 流式落点 / user_line 路由。
///
/// 单列 part 文件：chat_controller.dart 贴着 Dart lib 的 800 行硬棘轮，而
/// 「这一帧属于哪一轮、该写进哪个会话」正是判据最多的一块。判据本身在纯模块
/// turn_guard.dart（VM 可测）；这里只做接线与落点。
///
/// 一条贯穿全篇的纪律（2026-11-18 审计 #16 / #18）：**用户切走之后，旧轮的
/// 任何帧都不许再写进别的会话**。所以这里的 append / replace / touch 都按
/// 「本轮所属会话」（TurnGuard.activeSession）走，而不是按「现在的活动会话」。
extension _ChatTurnLifecycle on ChatController {
  /// 按 id 找会话；null / 找不到 → null（调用方回落到活动会话）。
  ChatSession? _sessionById(String? id) {
    if (id == null || id.isEmpty) return null;
    for (final ChatSession session in sessions.sessions) {
      if (session.id == id) return session;
    }
    return null;
  }

  /// 往**指定**会话追加一条消息（保持 ChatSessionStore.append 的两条副作用：
  /// 推进 updatedAt、按上限裁掉最旧的）。
  ///
  /// 为什么不直接 sessions.append：那个方法只写活动会话。当这一轮属于 A、
  /// 而用户正在看 B 时（user_line 晚到 / 切走），写 B 就是串会话（#16/#19）。
  void _appendToSession(ChatSession session, ChatMessage message) {
    session.messages.add(message);
    session.updatedAt = DateTime.now();
    final int overflow =
        session.messages.length - ChatSessionStore.kMaxMessagesPerSession;
    if (overflow > 0) session.messages.removeRange(0, overflow);
  }

  /// 在**本轮所属会话**里就地替换一条消息（收口把空气泡换成系统行）。
  bool _replaceInTurnSession(ChatMessage old, ChatMessage replacement) {
    final ChatSession? turn = _sessionById(_guard.activeSession);
    if (turn == null || turn.id == sessions.activeId) {
      return sessions.replace(old, replacement);
    }
    final int index = turn.messages.indexOf(old);
    if (index < 0) return false;
    turn.messages[index] = replacement;
    turn.updatedAt = DateTime.now();
    return true;
  }

  /// 把**本轮所属会话**标记为刚用过（让它在会话列表里浮上来）。
  void _touchTurnSession() {
    final ChatSession? turn = _sessionById(_guard.activeSession);
    if (turn == null || turn.id == sessions.activeId) {
      sessions.touchActive();
    } else {
      turn.updatedAt = DateTime.now();
    }
  }

  /// 拒绝一次「服务端已有别的轮在跑」的发送，并给出**一行原因**（#17）。
  ///
  /// 不伪造用户气泡：那句话并没有发出去，上屏就是另一种撒谎。错误走既有横幅
  /// （组合根读 _ui.errorMessage 或 _chat.error）；sendFailedLocally 置位让
  /// 组合根把相位从「思考中」回落——否则横幅旁边还挂着一个转圈。
  void _refuseSendWhileTurnElsewhere() {
    _error = kTurnElsewhereMessage;
    _errorCode = kTurnElsewhereCode;
    _sendFailedLocally = true;
    _notify();
  }

  /// user_line 到达：写进**发起这一轮的那个会话**，而不是「现在的活动会话」。
  ///
  /// #19（2026-11-18）：服务端从这一版起在帧上带**可选** session_id。用户完全
  /// 可能在帧到达前已经切走：旧实现把 A 的那句话写进 B（历史串台）。缺字段 /
  /// 未知 id → 回落活动会话（旧服务端仍然可用）。
  void _onUserLine(String? text, int epoch, String? frameSessionId) {
    // **轮次身份守卫**（#16）：本地已作废那一轮的迟到帧静默丢弃；也不许把
    // lastUserLineAction 写成「接受」——壳据此清舞台 cue，重算会算成重复。
    if (!_guard.verdictFor(epoch).accepted) {
      _lastUserLine = UserLineAction.skipDuplicate;
      return;
    }
    final String body = (text ?? '').trim();
    final ChatSession? target = _sessionById(frameSessionId);
    final List<ChatMessage> targetMessages = target?.messages ?? messages;
    // 判据用**目标会话**的「上一条用户句」：比对活动会话的话，A 的重复帧
    // 会被算成 B 的新输入（反过来也会漏掉真正的重复）。
    final UserLineAction action = userLineAction(
      turnInFlight: _turnInFlight,
      text: text,
      lastUserText: lastUserText(targetMessages),
      lastMessageText: targetMessages.isEmpty ? null : targetMessages.last.text,
    );
    _lastUserLine = action;
    switch (action) {
      case UserLineAction.acceptInjected:
        acceptInjectedUserTurn(body, sessionId: target?.id);
      case UserLineAction.appendOnly:
        final ChatMessage line = ChatMessage(role: ChatRole.user, text: body);
        if (target == null || target.id == sessions.activeId) {
          sessions.append(line);
        } else {
          _appendToSession(target, line);
        }
        _persist();
        _notify();
      case UserLineAction.appendSystem:
        final ChatMessage line = ChatMessage(role: ChatRole.system, text: body);
        if (target == null || target.id == sessions.activeId) {
          sessions.append(line);
        } else {
          _appendToSession(target, line);
        }
        _persist();
        _notify();
      case UserLineAction.ignoreEmpty || UserLineAction.skipDuplicate:
        break;
    }
  }

  /// 把「正在生成」的那一轮就地作废（**不**在本地等任何回执）。
  ///
  /// stopServer 为真 = 这次作废是「用户离开了这个会话」（切会话 / 新建 / 删
  /// 拥有这一轮的会话 / 清空）：必须**真的叫停服务端**（#16）。只把本地气泡标
  /// 结束的话，服务端会继续生成，迟到的帧会落进别的会话。fire-and-forget：
  /// 本地收口是确定的，不等 HTTP 回执；回执一到就解除「待停」闸门（#17）。
  ///
  /// stopServer 为假只用于「外部注入的新一轮让位旧轮」：服务端那边新一轮已经
  /// 开始，旧轮由它自己的抢占语义停掉，这里再发 stop 会打错目标。
  void _abandonTurn({bool stopServer = false}) {
    final ChatMessage? bubble = _assistant;
    if (bubble != null) bubble.streaming = false;
    _assistant = null;
    // 作废本轮：这条气泡不再是 text_fallback 的写回对象。
    _settledAssistant = null;
    _streaming = false;
    _highlight.clear();
    _clearSpoken();
    // 记下「哪一轮、属于哪个会话」：之后所有迟到的帧都按这个身份丢弃，
    // 绝不会因为 messages 跟着活动会话走就串到新会话（#16）。
    final bool hadTurn = _guard.abandon();
    if (!hadTurn || !stopServer) return;
    _stopAbandonedTurnOnServer();
  }

  /// 叫停被本地作废的那一轮服务端生成（fire-and-forget）。
  ///
  /// **不设错误横幅**：切会话是本地视图操作，用户没有表达「别说了」；只是不能
  /// 放任服务端继续跑（否则两轮重叠 + 串会话）。失败也不报错——停止端点幂等，
  /// 真正的保险是后续帧会被守卫丢掉。
  void _stopAbandonedTurnOnServer() {
    unawaited(
      api.stopChat().then<void>(
        (_) {
          _guard.onStopAcknowledged();
          // 「上一段会话还在回答」的闸门可能刚解除：通知一次让界面跟上。
          _notify();
        },
        onError: (Object _) {
          // 后端不可达 / 非 2xx：闸门也要有出口，否则输入框永久被挡。
          _guard.onStopAcknowledged();
          _notify();
        },
      ),
    );
  }

  /// **外部原因**导致进行中的一轮必须就地收口（并留一句话说清原因）。
  ///
  /// 与 [_abandonTurn] 的区别：那是「用户自己切走了」，静默是对的；
  /// 这里用户什么都没做却看不到回复——必须留话，否则表现与「切走后的静默」
  /// 完全一样（都是没有输出），而两者的原因完全不同。
  ///
  /// 已经收到的部分文字**保留**（那是真实收到的内容，不许丢）；一个字都没
  /// 收到就不留空气泡（与空气泡的收口规则一致——但要留一条**系统行**，
  /// 由 [_finishTurn] 的 wordless 分支负责；这里删掉的是「还没收口的
  /// 空气泡」，不产生提示）。
  void _releaseTurn({required String message, required String code}) {
    final ChatMessage? bubble = _assistant;
    if (bubble != null) {
      bubble.streaming = false;
      // 一个字都没收到 → 换一条**说清原因**的系统行。用同一句 message：
      // 这里**不能**用「模型没有返回文字」那条——这一轮是被中断/断连掉的，
      // 写成「模型没说话」就是另一种伪造。
      if (bubble.text.trim().isEmpty) {
        _replaceInTurnSession(
          bubble,
          ChatMessage(role: ChatRole.system, text: message),
        );
      }
    }
    _assistant = null;
    _streaming = false;
    _error = message;
    _errorCode = code;
    _highlight.clear();
    _clearSpoken();
    _guard.settle();
    _persist();
  }

  /// 追加流式正文。
  ///
  /// 调用方已经用 TurnGuard.verdictFor 确认：这一帧属于当前轮，或由它**开启**
  /// 新的一轮（服务端主动开口 / 外部注入）。这里不再重复 epoch 判据。
  void _appendDelta(String text, int? epoch, int? sentenceSeq) {
    var bubble = _assistant;
    if (bubble == null) {
      // 服务端主动开口 = 新一轮：旧气泡的朗读标记同样立即作废（见 send）。
      _clearSpoken();
      _highlight.clear();
      bubble = ChatMessage(
        role: ChatRole.assistant,
        epoch: epoch,
        streaming: true,
      );
      // 先追加到活动会话（可能因此新建会话），再登记本轮身份——顺序反了
      // 会记下一个当时还不存在的会话 id。
      sessions.append(bubble);
      _guard.start(sessionId: sessions.activeId, epoch: epoch);
    } else {
      // 旧服务端的帧没有 epoch：当前轮还不知道代次时从帧上补记。
      _guard.noteFrameEpoch(epoch);
    }
    bubble.text += text;
    if (sentenceSeq != null) {
      bubble.spoken.add(SpokenSpan(seq: sentenceSeq, text: text));
    }
    _assistant = bubble;
    _streaming = true;
    _notify();
  }

  /// 累积思考片段到当前（或新建的）助手气泡。
  ///
  /// 与 [_appendDelta] 共用「气泡引用」语义：思考先到而正文还没到时，
  /// 这里**建**一个气泡并挂住它，等正文（SentenceVoiced）到了再往里写正文——
  /// 这样界面不会先出现一个空气泡，也不会出现两条气泡。
  void _appendReasoning(String text, int? epoch) {
    var bubble = _assistant;
    if (bubble == null) {
      bubble = ChatMessage(
        role: ChatRole.assistant,
        epoch: epoch,
        streaming: true,
      );
      sessions.append(bubble);
      _guard.start(sessionId: sessions.activeId, epoch: epoch);
      _assistant = bubble;
    } else {
      _guard.noteFrameEpoch(epoch);
    }
    bubble.reasoning += text;
    _notify();
  }

  /// 失败轮的正文兜底（text_fallback，rc.3 N0，2026-09-13）。
  ///
  /// **整段设置**（不是追加）并打上 unfinished：服务端发的是**整轮正文**，
  /// 已上屏的句子只是它（大致）的前缀——追加会让正文出现两遍，覆盖还天然幂等
  /// （不必算「前缀到哪结束」，那受切句器 trim 影响、算不准）。
  /// 要能自己建气泡：「服务端主动开口」/ 帧先于 HTTP 响应到达时 _assistant
  /// 还可能是 null，那时丢了帧就等于又把正文丢了。
  void _applyTextFallback(String text, int? epoch) {
    // **同一轮只留一个助手气泡**（T1，2026-10-13 执行单）：本轮已收口时写回
    // 刚落定的那一条（并标「未收尾」），不 append 第二条。判据是纯函数
    // [fallbackTarget]（VM 可测），三种落点各有各的语义。
    final FallbackTarget target = fallbackTarget(
      hasCurrentAssistant: _assistant != null,
      hasSettledAssistant: _settledAssistant != null,
      // _settledAssistant 只在气泡**仍留在会话里**时才被登记
      // （见 _finishTurn 的两个系统行分支），所以这里恒为 true。
      settledInSession: true,
      frameEpoch: epoch,
      settledEpoch: _settledAssistant?.epoch,
    );
    if (target == FallbackTarget.settled) {
      final ChatMessage bubble = _settledAssistant!;
      bubble.text = text;
      bubble.unfinished = true;
      // 真正文到了：占位文案（「（生成失败）」）必须让位，否则真实内容顶着
      // 失败底色显示。
      bubble.failed = false;
      // 只写回一次；下一帧要是还有（同轮重发）就走新建那条老路。
      _settledAssistant = null;
      // 收口时已经落过一次盘，这里是新增的文本，必须再落一次。
      _touchTurnSession();
      _persist();
      _notify();
      return;
    }
    var bubble = _assistant;
    if (bubble == null) {
      bubble = ChatMessage(
        role: ChatRole.assistant,
        epoch: epoch,
        streaming: true,
      );
      sessions.append(bubble);
      _guard.start(sessionId: sessions.activeId, epoch: epoch);
    } else {
      _guard.noteFrameEpoch(epoch);
    }
    bubble.text = text;
    bubble.unfinished = true;
    bubble.failed = false;
    _assistant = bubble;
    _streaming = true;
    _notify();
  }

  void _finishTurn({bool failed = false, bool stopped = false}) {
    // ── 攒句器的收尾信号（2026-09-11）────────────────────────────────────
    // 收口是「不会再有本句分片」的**唯一确定时点**：引擎允许某句
    // final_chunk 样本数为 0，那一句就永远等不到 end（end=true 才是
    // 句尾，见 audio/sentence_assembler.dart），尾巴卡在累积里播不出来。
    // 失败 / 停止的轮次**不**收尾——那半截没合成完，播出来就是断句。
    if (!failed && !stopped) audio.flush();
    // 失败 / 停止的轮次没有「后续音频」，高亮就地灭。**正常收口不清**：最后
    // 几句可能还在播，提前灭了就是「话没说完高亮先没了」。
    if (failed || stopped) _highlight.clear();
    final bubble = _assistant;
    if (bubble != null) {
      bubble.streaming = false;
      // ── 空气泡的收口方式：判据在 turn_liveness.dart（纯逻辑，可单测）──
      // 历史弯路（别再走回去）：替模型编一句台词（用户以为那是回复）、或直接
      // sessions.remove(bubble)（「模型没返回文字」与「应用坏了」无法区分，
      // 用户就是来报「第一次对话之后没输出了」的）——两条都被否了。
      // 现在的路：**换一条系统行**（ChatRole.system），事实照说但一眼可见
      // 不是角色说的；用户主动停止同路、另一句话（[kStoppedTurnNotice]）。
      // T1（2026-10-13）：先把这条气泡登记成 text_fallback 的写回对象，
      // 下面真把它换成系统行的两个分支再清掉。
      _settledAssistant = bubble;
      switch (settleTurn(
        failed: failed,
        text: bubble.text,
        stopped: stopped,
        hasReasoning: bubble.reasoning.trim().isNotEmpty,
      )) {
        case TurnSettlement.keep:
        // 只有思考：**保留气泡**（思考是用户要看的内容），由气泡自己标一句
        // 「只有思考、没有正文」（见 kReasoningOnlyCaption）。
        case TurnSettlement.keepReasoningOnly:
          break;
        case TurnSettlement.failed:
          bubble.failed = true;
          bubble.text = '（生成失败）';
        case TurnSettlement.wordless:
          _replaceInTurnSession(
            bubble,
            ChatMessage(role: ChatRole.system, text: kWordlessTurnNotice),
          );
          _settledAssistant = null;
        case TurnSettlement.stopped:
          _replaceInTurnSession(
            bubble,
            ChatMessage(role: ChatRole.system, text: kStoppedTurnNotice),
          );
          _settledAssistant = null;
      }
    }
    _assistant = null;
    _streaming = false;
    // **失败但没收到 error 帧**：老实说「不知道为什么」，并指向诊断日志。
    // 以前这里什么都不设——界面上只有一个灰掉的「（生成失败）」气泡，用户
    // 既没有码也没有去处可查（「前端无法知道错误信息」）。既然现在后端会发
    // error 帧（带码），收不到就说明是「旧服务端 / 帧被截断 / 真的静默失败」，
    // 这三种都该明说，而不是假装知道原因。
    if (needsFailureFallbackCode(failed: failed, hasError: _error != null)) {
      _error = kTurnFailedWithoutDetailMessage;
      _errorCode = kTurnFailedWithoutDetailCode;
    }
    // **一轮结束才落盘**：text_delta 是毫秒级的，逐条写 localStorage
    // 会把主线程拖垮（序列化整份会话 × 每秒几十次）。
    _touchTurnSession();
    _persist();
    _guard.settle();
    _notify();
  }
}
