part of 'package:live2d_ai_shell/main.dart';

extension _ShellLifecycleWiring on _ShellRootState {
  void _initShell() {
    // 会话 baseline 的第 ① 路交付（D28）：host 随 chat/stop 响应体回。
    // 值不进任何字段——回调里解析成一次性下发清单，立即应用后即丢。
    _api = ApiClient(onSessionBaseline: _onSessionBaselineDelivered);
    _modelsApi = ModelsApi();
    _envApi = EnvApi();
    _modsApi = ModsApi();
    _diagApi = DiagnosticsApi();
    // 常态语音检测：读 voice-input Mod 的 wake_phrase（缺省「小可爱」），
    // 命中后把原文交给 /api/v1/voice/transcript（服务端剥词，一份真相）。
    _voiceApi = VoiceApi();
    _voiceListen = VoiceListenController(
      recognizer: createSpeechRecognizer(),
      // B1（L1，2026-09-16）：服务端受理后**立刻**把用户句上屏并开一轮。
      // 以前这里直接透传 `_voiceApi.sendTranscript`：链路（turn_prompt / 回答）
      // 起来了，聊天区却没有用户那句话——用户以为「识别了但没发出去」。
      send: (String text, {bool ptt = false}) async {
        final VoiceTranscriptResult result = await _voiceApi.sendTranscript(
          text,
          ptt: ptt,
        );
        if (result.ok) {
          // 语音注入也是「新消息」：先同步取消上一轮编排（V10 §9.3），再上屏。
          _cancelStageForTurnBoundary('voice');
          // 上屏用服务端剥词 / 归一化后的正文（与真正喂给 LLM 的**同一份**）。
          _chat.acceptInjectedUserTurn(
            result.text.isNotEmpty ? result.text : text,
          );
        }
        return result;
      },
      loadWakePhrase: _loadWakePhrase,
      // Mod 未启用 → 先给红字（不要等 403 回来）。
      loadModEnabled: _voiceInputModEnabled,
      // 两条路都落到同一段「写进输入框」：
      // - busy 不排队，正文交回输入框让用户改字重发；
      // - 产品听写的定稿（成功与失败都走它）。
      onBusyResult: _onVoiceResult,
      onDictation: _onVoiceResult,
    );
    _settings = SettingsController(api: _api);
    // 动作幅度的**即时预览**（2026-09-16 修）：三滑条改的是设置草稿，
    // 草稿一变就防抖下发渲染面——不必先点「保存」。
    // 监听放在这里（而不是 ListenableBuilder 的 builder 里）的理由：
    // builder 只在「有东西重建」时跑，而拖动滑条恰好**只改草稿**；
    // 监听器与「保存 / 放弃 / 重新加载」共用同一条路（那三条都会 notify）。
    _settings.addListener(_onActionScalesModelContextChanged);
    _ws = WsClient();
    _audio = AudioPlayer();
    // 静音与音量是纯本机输出设置（不经渲染面）：**默认出声**。
    _audio.muted = widget.prefs.muted;
    _audio.volume = widget.prefs.volume;
    // 停止 / 新消息的取消纪律（V7 §6.6 / V10 §9.3）：四个出口都在本 State 上。
    // 顺序由 [StageCancellation] 钉住（纯逻辑、VM 可测），这里只提供实体。
    _stageCancellation = StageCancellation(
      clearCuePlan: (String reason) =>
          _directorCues.replace(const <ActionCue>[]),
      revokeStage: (String reason) {
        final Live2DStageState? stage = _stageKey.currentState;
        if (stage != null) {
          _recordPresetRequest(id: 'none', source: reason);
        }
        unawaited(stage?.applyPreset('none', source: reason));
      },
      dropPendingAudio: (String reason) => _audio.interrupt(),
      returnToBaseline: _requestSessionBaseline,
    );
    // 背景轮播：首次对表（`_applyPrefs` 也会在桥就绪后再对一次）。
    syncSlideshow(widget.prefs);
    // 会话存档：启动时读一次，之后每次变动写回。
    //
    // **落盘时机由 `ChatController` 决定**（一轮结束 / 会话操作），
    // 不是每条消息——`text_delta` 是毫秒级的，逐条写会把主线程拖垮。
    _chat = ChatController(
      api: _api,
      ws: _ws,
      audio: _audio,
      sessions: loadChatSessions(),
      persistSessions: saveChatSessions,
    );

    // 音频 RMS → 舞台口型（30 Hz 高频信号，**不进 Widget 树**，原则 P6）。
    _levelSubscription = _audio.levels.listen((level) {
      _stageKey.currentState?.setMouth(level);
    });

    // 导演 cue（P1-3）：一句音频**开始播放**时按 seq apply（动作与声音同拍）。
    _sentenceCueSubscription = _audio.sentenceStarts.listen(
      _applyDirectorCueForSeq,
    );

    // stage-clock（协议 §6.5 / O13）：**段内播放位置 30ms** 下发渲染面，让编排
    // 锚在音频时钟（V7 §6.1）而不是 performance.now()。节奏由 AudioPlayer 的
    // 30ms ticker 保证；停止后它只发一次 playing:false，不补帧。
    _stageClockSubscription = _audio.stageClock.listen((StageClockSample s) {
      DirectorObserverFeed.instance.pushStageClock(
        seg: s.seg,
        posMs: s.posMs,
        playing: s.playing,
      );
      _stageKey.currentState?.sendStageClock(
        seg: s.seg,
        posMs: s.posMs,
        playing: s.playing,
      );
    });

    // 相位跟踪器自己订阅 WS（只读消费，与 ChatController 互不干扰）。
    _statusSubscription = _ws.statuses.listen(_ui.onWsStatus);
    _eventSubscription = _ws.events.listen((WsEvent event) {
      // 阶段5 W5a：B 栏事件流（text_delta / text_fallback 同时进 D 栏左列）。
      DirectorObserverFeed.instance.pushWsEvent(event);
      final bool wasMuted = _serverMuted;
      _ui.consume(event);
      if (event is AudioEvent && event.muted != wasMuted) {
        _rebuild(() => _serverMuted = event.muted);
      }
      // P0-4：角色播报期间**暂停听**（自己的声音 / 环境人声会误触发），
      // 播报结束自动恢复常驻；PTT 进行中不抢。
      if (event is RuntimeStatusEvent) {
        if (event.event == 'voice_started') {
          unawaited(_voiceListen.suspendForPlayback());
        } else if (event.event == 'voice_ended') {
          unawaited(_voiceListen.resumeAfterPlayback());
        }
      }
      // 导演按句 cue（P1-3，2026-09-16）：整体替换当前计划；缺省忽略 = 兼容。
      // 空 cues = 清空计划 = **本轮不动**（不归零；归零只走 preset_id='none'）。
      // **唯一驱动通道**——阶段3 起不再有 text_delta / text_fallback 触发的
      // 「拉状态面 latest.preset_id」分支（通道 B 退役，见 D10–D13）。
      if (event is ActionCueEvent) {
        // **D31（阶段4f）**：host 的取消信号 = 既有 action_cue 帧 +
        // `baseline:true`（`cues` 是该会话 baseline，不是按句计划）。
        // 第 ② 路交付必须**立即应用**到舞台——不进 [_directorCues]（那是按句
        // 计划，会等到音频开始才生效），也不缓存（不建前端镜像，V8）。
        if (event.baseline) {
          _applyBaselineCues(event.cues, event.reason ?? 'baseline');
          return;
        }
        _directorEpoch = event.epoch;
        _directorCues.replace(event.cues);
      }
    });

    _ws.connect();
    unawaited(_loadAppStatus());
    // 启动就取一次 voice-input 的启停：主界面「听」按钮旁要能**先**给红字
    // （而不是等用户说完才由 403 回来说）。
    unawaited(_refreshVoiceModState());
  }

  void _didUpdateShell(ShellRoot oldWidget) {
    // 启动时字节回来，可能摘掉「字节已经不在」的孤儿项 ⇒ 库长度变了。
    // 轮播的对表只在偏好**提交**时做，而那次提交发生在 `ShellRoot` 之外
    // （应用根的 `setState`），所以这里必须补一次，否则定时器会拿着
    // 旧长度数到不存在的第 N 项。
    if (oldWidget.prefs.backgrounds.length !=
        widget.prefs.backgrounds.length) {
      syncSlideshow(widget.prefs);
    }
  }

  void _disposeShell() {
    unawaited(_levelSubscription?.cancel());
    unawaited(_statusSubscription?.cancel());
    unawaited(_eventSubscription?.cancel());
    unawaited(_sentenceCueSubscription?.cancel());
    unawaited(_stageClockSubscription?.cancel());
    _actionScalesSyncer.dispose();
    // 待发的覆盖编辑直接丢弃（dispose 后 API 客户端也会关，flush 反而会
    // 在请求发出后立刻被关掉连接）。窗口只有 250ms，代价可忽略。
    _modelOverrideCoalescer.dispose();
    _settings.removeListener(_onActionScalesModelContextChanged);
    _ui.dispose();
    _slideshow.dispose();
    _live.dispose();
    _settings.dispose();
    _modelsApi.dispose();
    _envApi.dispose();
    _modsApi.dispose();
    _diagApi.dispose();
    _voiceListen.dispose();
    _voiceApi.dispose();
    _chat.dispose();
    _ws.dispose();
    _audio.dispose();
    _api.dispose();
    _input.dispose();
  }
}
