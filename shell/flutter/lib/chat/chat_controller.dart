import 'dart:async';

import 'package:flutter/foundation.dart';

import '../api/api_client.dart';
import 'chat_message.dart';
import 'chat_session.dart';

export 'chat_message.dart';
import '../api/ws_client.dart';
import '../audio/audio_player.dart';
import '../audio/epoch_gate.dart';
import 'turn_liveness.dart';

/// 聊天状态：消息列表 + 流式拼接 + turn 状态 + 错误提示。
///
/// 数据源：`POST /api/v1/chat`（发送）、`/ws/state`（流式与音频）。
class ChatController extends ChangeNotifier {
  ChatController({
    required this.api,
    required this.ws,
    required this.audio,
    ChatSessionStore? sessions,
    this.persistSessions,
  }) : sessions = sessions ?? ChatSessionStore.empty() {
    _subscriptions.addAll(<StreamSubscription<Object?>>[
      ws.events.listen(_onWsEvent),
      ws.statuses.listen((status) {
        _wsStatus = status;
        // **通道掉了就收口**（2026-09-11）：见 `turn_liveness.dart` 的
        // `mustReleaseTurnOnWsLoss`——收口帧在断连期间不会重放，丢了就再也
        // 回不来，输入框会永久卡在「停止本轮」而 `send()` 静默 return。
        if (mustReleaseTurnOnWsLoss(turnInFlight: _streaming, status: status)) {
          _releaseTurn(
            message: kWsDroppedMidTurnMessage,
            code: kWsDroppedMidTurnCode,
          );
        }
        notifyListeners();
      }),
    ]);
  }

  final ApiClient api;
  final WsClient ws;
  final AudioPlayer audio;

  final List<StreamSubscription<Object?>> _subscriptions =
      <StreamSubscription<Object?>>[];

  /// 多会话存储（**本轮只做「本地记录」**，模型记忆见该文件的头注）。
  ///
  /// 由组合根注入：启动时从 localStorage 反序列化，之后每次变动回调
  /// [persistSessions] 写回。**注入而不是自己读存储**，是因为
  /// `package:web` 只能出现在组合根（`chat/` 这一层要能在 VM 上单测）。
  final ChatSessionStore sessions;

  /// 会话变动后的落盘回调（`null` = 不持久化，测试与无存储环境用）。
  final void Function(ChatSessionStore store)? persistSessions;

  /// 界面绑定的消息列表 = **当前会话的 List 实例**（切换会话即换引用）。
  ///
  /// 刻意用 getter 而不是缓存一个字段：缓存就必须在切会话时同步更新，
  /// 而「忘了同步」会表现为「切了会话但列表没变」——一个不会报错的 bug。
  List<ChatMessage> get messages => sessions.messages;

  bool _streaming = false;
  String? _error;

  /// 最近一次错误的**机器码**（`llm_upstream_401` / `tts_transport` …）。
  ///
  /// 与 [error] 的关系：`error` 是给人看的一行（码 + 说明 + 提示），本字段是
  /// 给代码分支用的锚点——错误横幅据此决定「下一步」按钮是「去 LLM 设置」
  /// 还是「去语音合成设置」，**不去解析显示文案**（文案会改，码是契约）。
  String? _errorCode;

  WsStatus _wsStatus = WsStatus.idle;
  ChatMessage? _assistant;
  int? _audioEpoch;

  bool get streaming => _streaming;
  String? get error => _error;

  /// 最近一次错误的机器码（见 [_errorCode]）。
  String? get errorCode => _errorCode;

  WsStatus get wsStatus => _wsStatus;

  /// 发送用户文本；成功则建流式 assistant 气泡，失败写入可见错误。
  Future<void> send(String raw) async {
    final text = raw.trim();
    if (text.isEmpty || _streaming) return;
    audio.unlock();
    // **发送即保活**：服务端重启后旧连接会失效（POST 能成功但回复走 WS），
    // 这里主动确认连接，避免「发出去没回应」。（2026-09-10）
    ws.ensureConnected();
    sessions.append(ChatMessage(role: ChatRole.user, text: text));
    _persist();
    _streaming = true;
    _error = null;
    _errorCode = null;
    notifyListeners();
    try {
      final accepted = await api.sendChat(text);
      assert(accepted.accepted);
      final bubble = ChatMessage(
        role: ChatRole.assistant,
        epoch: accepted.epoch,
        streaming: true,
      );
      _assistant = bubble;
      sessions.append(bubble);
      notifyListeners();
    } on ApiException catch (error) {
      _error = error.toString();
      // 本地（HTTP）失败也有码——`ApiClient._errorFrom` 从响应体里取的
      // `error.code`（`busy` / `no_supervisor` / `invalid_payload` …）。
      _errorCode = error.code;
      _streaming = false;
      sessions.append(
        ChatMessage(role: ChatRole.assistant, text: '⚠ ${error.message}', failed: true),
      );
      _persist();
      notifyListeners();
    } catch (error) {
      _error = '发送失败：$error';
      _streaming = false;
      sessions.append(
        ChatMessage(role: ChatRole.assistant, text: '⚠ 发送失败：$error', failed: true),
      );
      _persist();
      notifyListeners();
    }
  }

  /// `POST /api/v1/chat/stop` 并收口当前气泡。
  ///
  /// **先本地收口，再发请求**（顺序有讲究）：
  /// ① 停止路径在后端走 `RootEffect::TurnAborted`，而它**不投影任何 WS 帧**
  ///    （`supervisor/handlers.rs`）——也就是说这一轮的收口**只能由这里做**，
  ///    服务端不会补一个 `turn_state`；
  /// ② 先收口还能避免下一行那个误判：服务端收到 stop 后会推进 epoch 并广播
  ///    `new_epoch`，而 `new_epoch` 在本文件的语义是「上一轮被**外部**中断」
  ///    （见 `_onWsEvent`）——若不先收口，用户自己按的停止会被当成外来抢占。
  Future<void> stop() async {
    _finishTurn(stopped: true);
    try {
      await api.stopChat();
    } on ApiException catch (error) {
      _error = error.toString();
      notifyListeners();
    } catch (error) {
      _error = '停止失败：$error';
      notifyListeners();
    }
  }

  void clearError() {
    _error = null;
    _errorCode = null;
    notifyListeners();
  }

  void _onWsEvent(WsEvent event) {
    switch (event) {
      case SubscribeAckEvent():
      case HeartbeatEvent():
      case UnknownWsEvent():
        break;
      case TextDeltaEvent(:final text, :final completed, :final epoch):
        if (text != null && text.isNotEmpty) _appendDelta(text, epoch);
        if (completed != null) _finishTurn(failed: completed == false);
      case ReasoningDeltaEvent(:final text, :final epoch):
        // 思考**只累积、不落盘**（见 `ChatMessage.reasoning` 的取舍说明），
        // 也**不**触发 `_streaming`——「正在思考」不等于「正文已开始」，
        // 混用会让输入框的停止态与文字气泡的流式态对不上。
        if (text != null && text.isNotEmpty) _appendReasoning(text, epoch);
      case TextFallbackEvent(:final text, :final epoch):
        // rc.3 N0 正文兜底：失败轮把**整轮正文**交过来，**覆盖式**设置并标注
        // 「未收尾」（见 [_applyTextFallback]）。
        if (text != null && text.isNotEmpty) _applyTextFallback(text, epoch);
      case TurnStateEvent(:final status):
        _finishTurn(failed: status == 'failed');
      case final RuntimeStatusEvent status:
        final String name = status.event;
        if (name == 'voice_started') {
          _streaming = true;
          notifyListeners();
        }
        // 服务端推进 epoch = 上一轮作废（停止按钮 / 抢占）。判据抽在
        // `audio/epoch_gate.dart`（纯函数，**可单测**）——那是钉子 13，
        // 而本文件必须 `import 'package:web'`，在 `flutter test` 里加载不了。
        if (mustInterruptAudio(event)) {
          _audioEpoch = null;
          audio.interrupt();
          // **同一件事对「文字」也成立**（2026-09-11 追加）：`new_epoch` 的
          // 语义是「上一轮作废」，而停止**不发 `turn_state`**——所以当停止是
          // **别的客户端**发起的（本机开了两个页面时很常见），本客户端那一轮
          // 就永远等不到收口帧，输入框锁死在「停止本轮」。
          // 用户自己按的停止已经在上面的 `stop()` 里先收口了，走到这里说明
          // 这一轮是**外部**中断的。
          if (_streaming) {
            _releaseTurn(
              message: kTurnPreemptedMessage,
              code: kTurnPreemptedCode,
            );
          }
        }
      case AudioEvent(
          :final pcm,
          :final sampleRate,
          :final epoch,
          :final start,
          :final end,
          :final volume,
          :final muted,
          :final sentenceSeq,
          :final wav,
        ):
        _onAudio(
          pcm,
          sampleRate,
          epoch,
          start,
          end,
          volume,
          muted,
          sentenceSeq,
          wav,
        );
      case WsErrorEvent(:final code, :final message, :final hint):
        // **错误码上屏**（2026-09-11）：`code` 是契约锚点，也是后端日志里的
        // 字段名——只有 message 时用户根本无法定位（「上游非成功状态 401」
        // 查不到任何东西）。`hint` 由后端给，前端只显示。
        _error = formatWsError(code: code, message: message, hint: hint);
        _errorCode = code;
        notifyListeners();
    }
  }

  void _appendDelta(String text, int? epoch) {
    var bubble = _assistant;
    if (bubble == null) {
      bubble = ChatMessage(role: ChatRole.assistant, epoch: epoch, streaming: true);
      // 走到这里说明「没经过 send() 就收到了 delta」（例如服务端主动开口）。
      // 仍然追加到**当时**的活动会话；之后用户切走也不会把它带走——
      // 因为下面用的是 `bubble` 这个引用，而不是 `messages`。
      sessions.append(bubble);
    }
    bubble.text += text;
    _assistant = bubble;
    _streaming = true;
    notifyListeners();
  }

  /// 累积思考片段到当前（或新建的）助手气泡。
  ///
  /// 与 [`_appendDelta`] 共用「气泡引用」语义：思考先到而正文还没到时，
  /// 这里**建**一个气泡并挂住它，等正文（`SentenceVoiced`）到了再往里写正文——
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
      _assistant = bubble;
    }
    bubble.reasoning += text;
    notifyListeners();
  }

  /// 失败轮的正文兜底（`text_fallback`，rc.3 N0，2026-09-13）。
  ///
  /// **整段设置**（不是追加）并打上 `unfinished`：服务端发的是**整轮正文**，
  /// 而已上屏的句子只是它的（大致）前缀——追加会让正文出现两遍。覆盖天然幂等，
  /// 也不必去算「已上屏的前缀到哪结束」（那受切句器 trim 影响，算不准）。
  ///
  /// 为什么要能建气泡：理论上 `send()` 已经建好了；但「服务端主动开口」或
  /// 「帧先于 HTTP 响应到达」时 `_assistant` 还可能是 null，那时照样要收下
  /// —— 丢帧就等于又把正文丢了。
  void _applyTextFallback(String text, int? epoch) {
    var bubble = _assistant;
    if (bubble == null) {
      bubble = ChatMessage(
        role: ChatRole.assistant,
        epoch: epoch,
        streaming: true,
      );
      sessions.append(bubble);
    }
    bubble.text = text;
    bubble.unfinished = true;
    _assistant = bubble;
    _streaming = true;
    notifyListeners();
  }

  void _onAudio(
    Uint8List pcm,
    int sampleRate,
    int epoch,
    bool start,
    bool end,
    double? volume,
    bool muted,
    int? sentenceSeq,
    Uint8List? wav,
  ) {
    final AudioGateDecision decision = decideAudioFrame(
      currentEpoch: _audioEpoch,
      incomingEpoch: epoch,
    );
    if (decision.interrupt) audio.interrupt();
    _audioEpoch = decision.epoch;
    audio.feed(
      PcmFrame(
        pcm: pcm,
        sampleRate: sampleRate,
        start: start,
        end: end,
        volume: volume,
        // 服务端静音**观测值**（PCM 已被置零）：播放侧不据此改行为，
        // 口型照常用 volume 驱动——见 `PcmFrame.muted`。
        muted: muted,
        // 2026-09-11 新契约的可选字段：句内分片归组；整句 WAV 直通（当前
        // 服务端不发，缺省 null）。两者都只是**透传**——攒句规则住在
        // `audio/sentence_assembler.dart`（纯逻辑，可单测）。
        sentenceSeq: sentenceSeq,
        wav: wav,
      ),
    );
  }

  void _finishTurn({bool failed = false, bool stopped = false}) {
    // ── 攒句器的收尾信号（2026-09-11）────────────────────────────────────
    // 一轮收口是「不会再有本句分片」的**唯一确定时点**，所以在这里给音频侧
    // 一个收尾闸门。常规路径用不到它（`end=true` 才是句尾，见
    // `audio/sentence_assembler.dart` 头注）；它防的是一个已确认的服务端边界：
    // 引擎允许某句的 `final_chunk` **样本数为 0**，而 `build_audio_frames`
    // 对空 PCM 直接返回空列表——那一句就永远等不到 `end`，尾巴会卡在累积里
    // 播不出来（「话说完了但结尾没了」）。
    //
    // 失败/停止的轮次**不**收尾：那半截本来就没合成完，播出来就是断句，而
    // 契约是「宁可晚开口，不可中途断」。
    if (!failed && !stopped) audio.flush();
    final bubble = _assistant;
    if (bubble != null) {
      bubble.streaming = false;
      // ── 空气泡的收口方式：判据在 `turn_liveness.dart`（纯逻辑，可单测）──
      //
      // 历史（两天的弯路，别再走回去）：
      // - 曾经写成 `bubble.text = '（本轮没有文字输出——通常是模型只调用了
      //   动作工具）'`：**那不是模型的输出**，是前端凭上下文猜的一句台词，
      //   却长得和真回复一模一样 → 用户问「为何会有空提示词」。
      //   （2026-09-11 补：LLM 已无工具，那句「只调用了动作工具」的猜测
      //   连前提都不成立了——现在只陈述「本轮没有返回文字」这个事实。）
      // - 2026-09-11 改成 `sessions.remove(bubble)`：不再伪造了，但对
      //   「模型整轮没返回文字」这个**正常完成**的回合变成了**什么都不显示**
      //   → 用户报「第一次对话之后没输出了」（2026-09-11 实测复现：
      //   `点点头。` → 零文字、零音频、`turn_state{completed}`）。
      //
      // 现在的第三条路：**换一条系统行**（`ChatRole.system`）。事实照说，
      // 但一眼就能看出那不是角色说的话。**用户主动停止**也走这条路，
      // 但用另一句话（[kStoppedTurnNotice]）——别把用户的意志说成模型的产出。
      switch (settleTurn(
        failed: failed,
        text: bubble.text,
        stopped: stopped,
        hasReasoning: bubble.reasoning.trim().isNotEmpty,
      )) {
        case TurnSettlement.keep:
        // 只有思考：**保留气泡**（思考是用户要看的内容），由气泡自己标一句
        // 「只有思考、没有正文」（见 `kReasoningOnlyCaption`）。
        case TurnSettlement.keepReasoningOnly:
          break;
        case TurnSettlement.failed:
          bubble.failed = true;
          bubble.text = '（生成失败）';
        case TurnSettlement.wordless:
          sessions.replace(
            bubble,
            ChatMessage(role: ChatRole.system, text: kWordlessTurnNotice),
          );
        case TurnSettlement.stopped:
          sessions.replace(
            bubble,
            ChatMessage(role: ChatRole.system, text: kStoppedTurnNotice),
          );
      }
    }
    _assistant = null;
    _streaming = false;
    // **失败但没收到 `error` 帧**：老实说「不知道为什么」，并指向诊断日志。
    // 以前这里什么都不设——界面上只有一个灰掉的「（生成失败）」气泡，用户
    // 既没有码也没有去处可查（「前端无法知道错误信息」）。既然现在后端会发
    // `error` 帧（带码），收不到就说明是「旧服务端 / 帧被截断 / 真的静默失败」，
    // 这三种都该明说，而不是假装知道原因。
    if (failed && _error == null) {
      _error = '本轮失败，但服务端未给出错误详情'
          '（可打开「设置 → 开发模式 → 诊断日志」查看后端日志）';
      _errorCode = 'turn_failed_no_detail';
    }
    // **一轮结束才落盘**：`text_delta` 是毫秒级的，逐条写 localStorage
    // 会把主线程拖垮（序列化整份会话 × 每秒几十次）。
    sessions.touchActive();
    _persist();
    notifyListeners();
  }

  // ─────────────────────────────────────────────────────────────────────
  // 多会话：新建 / 切换 / 重命名 / 删除 / 清空
  //
  // 全部在这里做，UI 只调这些方法。**切换会话前先收口当前轮**：
  // 否则一条迟到的 delta 会落进新会话（`_assistant` 还指着旧气泡，
  // 见 `_appendDelta` 的引用语义——这里显式收口是为了让「切走时那一轮
  // 就算结束」这件事在代码里看得见，而不是依赖一个巧合）。
  // ─────────────────────────────────────────────────────────────────────

  /// 新建会话并切过去。
  void newSession() {
    _abandonTurn();
    sessions.pruneEmpty();
    sessions.create();
    _persist();
    notifyListeners();
  }

  /// 切到一个已有会话。
  void selectSession(String id) {
    if (sessions.active?.id == id) return;
    _abandonTurn();
    if (!sessions.select(id)) return;
    _persist();
    notifyListeners();
  }

  /// 重命名；传空串 = 恢复自动标题。
  void renameSession(String id, String title) {
    if (!sessions.rename(id, title)) return;
    _persist();
    notifyListeners();
  }

  /// 删除一个会话。
  void deleteSession(String id) {
    if (sessions.active?.id == id) _abandonTurn();
    if (!sessions.delete(id)) return;
    _persist();
    notifyListeners();
  }

  /// 清空当前会话的消息（保留会话）。
  void clearActiveSession() {
    _abandonTurn();
    sessions.clearActive();
    _persist();
    notifyListeners();
  }

  /// 把「正在生成」的那一轮就地作废（**不**调用服务端 /stop）。
  ///
  /// 为什么不顺带 `POST /chat/stop`：切会话是**纯本地视图操作**，用户没有
  /// 表达「别说了」。真让服务端停下是「停止」按钮的职责。这一轮剩下的
  /// delta 会继续落到**旧会话**（`_assistant` 还指着那个气泡），
  /// 而 `_streaming` 归位让新会话可以正常发消息。
  void _abandonTurn() {
    final ChatMessage? bubble = _assistant;
    if (bubble != null) bubble.streaming = false;
    _assistant = null;
    _streaming = false;
  }

  /// **外部原因**导致进行中的一轮必须就地收口（并留一句话说清原因）。
  ///
  /// 与 [_abandonTurn] 的区别：那是「用户自己切走了」，静默是对的；
  /// 这里用户什么都没做却看不到回复——必须留话，否则表现与「切走后的静默」
  /// 完全一样（都是没有输出），而两者的原因完全不同。
  ///
  /// 已经收到的部分文字**保留**（那是真实收到的内容，不许丢）；一个字都没
  /// 收到就不留空气泡（与空气泡的收口规则一致——但要留一条**系统行**，
  /// 由 [_finishTurn] 的 `wordless` 分支负责；这里删掉的是「还没收口的
  /// 空气泡」，不产生提示）。
  void _releaseTurn({required String message, required String code}) {
    final ChatMessage? bubble = _assistant;
    if (bubble != null) {
      bubble.streaming = false;
      // 一个字都没收到 → 换一条**说清原因**的系统行。用同一句 `message`：
      // 这里**不能**用「模型没有返回文字」那条——这一轮是被中断/断连掉的，
      // 写成「模型没说话」就是另一种伪造。
      if (bubble.text.trim().isEmpty) {
        sessions.replace(
          bubble,
          ChatMessage(role: ChatRole.system, text: message),
        );
      }
    }
    _assistant = null;
    _streaming = false;
    _error = message;
    _errorCode = code;
    _persist();
  }

  void _persist() {
    final void Function(ChatSessionStore store)? write = persistSessions;
    if (write != null) write(sessions);
  }

  @override
  void dispose() {
    for (final subscription in _subscriptions) {
      unawaited(subscription.cancel());
    }
    super.dispose();
  }
}
