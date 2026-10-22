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
      // 停止 / 新消息：按句计划整份作废（0.3.14 起 `clear`，不再是「整表替换
      // 成空」——那时空列表与「这一帧没有 cue」长得一样，语义相反）。
      clearCuePlan: (String reason) => _directorCues.clear(),
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

    // 播放忙闲的静默上报 + 「播完且不在 streaming」清按句计划（0.3.14）。
    // 放在这里而不是更早：处理函数要读 `_chat.streaming`（此时已构造好）。
    _initPlaybackWiring();

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
        // **`new_epoch` = 上一轮作废**（0.3.14）：还没开播的按句 cue 一并作废。
        // 判据用既有纯函数 [mustInterruptAudio]，不在这里重写一遍
        // （`audio/epoch_gate.dart` 同时服务音频打断，两处必须是同一个判据）。
        if (mustInterruptAudio(event)) _directorCues.clear();
      }
      // 0.3.14：被接受的新 `user_line` = 又一句话进来了 ⇒ 上一轮还没播的动作
      // 作废（重复帧不清）。判据的**结论**由 ChatController 在决策点留下，
      // 见 [_onUserLineCueBoundary] 为什么不能在这里重算。
      if (event is UserLineEvent) _onUserLineCueBoundary();
      // 导演按句 cue（P1-3，2026-09-16）：**按 sentence_seq 覆盖**（0.3.14）。
      // 空 cues = 这一帧没有 cue，**不是**清空（清空只走下面那几个触发点；
      // 归零只走 preset_id='none'）。
      // **唯一驱动通道**——阶段3 起不再有 text_delta / text_fallback 触发的
      // 「拉状态面 latest.preset_id」分支（通道 B 退役，见 D10–D13）。
      if (event is ActionCueEvent) {
        // **D31（阶段4f）**：host 的取消信号 = 既有 action_cue 帧 +
        // `baseline:true`（`cues` 是该会话 baseline，不是按句计划）。
        // 第 ② 路交付必须**立即应用**到舞台——不进 [_directorCues]（那是按句
        // 计划，会等到音频开始才生效），也不缓存（不建前端镜像，V8）。
        // 它同时是**取消信号**：还没开播的按句 cue 整份作废（0.3.14）。
        if (event.baseline) {
          _directorCues.clear();
          _applyBaselineCues(event.cues, event.reason ?? 'baseline');
          return;
        }
        _directorEpoch = event.epoch;
        // 0.3.14：**逐句覆盖**。以前整表替换，后一帧会把没带上的、还没开播的
        // 句子整片丢掉（同一轮的动作是逐句产出的）。
        _directorCues.upsert(event.cues);
      }
    });

    _ws.connect();
    unawaited(_loadAppStatus());
    // 启动就取一次 voice-input 的启停：主界面「听」按钮旁要能**先**给红字
    // （而不是等用户说完才由 403 回来说）。
    unawaited(_refreshVoiceModState());
  }

  /// 被接受的新 `user_line` 清掉还没播的上一轮动作（0.3.14）。
  ///
  /// 判据**不在这里重算**：`ChatController` 在本 State **之前**订阅
  /// `ws.events`（同一广播流按订阅顺序派发），壳的监听跑起来时消息表已经被它
  /// 改过——此时再调 `userLineAction`，刚上屏的那句会被算成「上一条就是这个」
  /// （`skipDuplicate`），于是「重复帧是唯一不清的」这条契约刚好反了过来。
  /// 所以只读控制器在**决策点**存下的结论（`ChatController.lastUserLineAction`）。
  ///
  /// 三种「被接受」都清：`acceptInjected`（没有轮在飞 → 这一句开新一轮）、
  /// `appendOnly`（同一轮里的**另一句**）、`appendSystem`（弹幕）——都意味着
  /// 又一句话进来了，上一轮剩下的 cue 不该再演。`skipDuplicate` / `ignoreEmpty`
  /// **不清**（同一句重放不是新输入）。
  void _onUserLineCueBoundary() {
    switch (_chat.lastUserLineAction) {
      case UserLineAction.acceptInjected:
      case UserLineAction.appendOnly:
      case UserLineAction.appendSystem:
        _directorCues.clear();
      case UserLineAction.skipDuplicate:
      case UserLineAction.ignoreEmpty:
      case null:
        break;
    }
  }

  /// 渲染面每采纳一次 loaded（含首次装载，0.3.6 T2）：模型身份进 dev 可观测面。
  ///
  /// **只记内存**：不新增 WS 帧、不落盘（导演可观测栏的红线）。B 栏因此能
  /// 看到 model_id / model_epoch / field_map 来源与降级原因——换模到底换了
  /// 哪个模型、哪个 epoch，开发者面有唯一证据。
  void _onStageModelLoaded(ModelLoadedInfo info) {
    final FieldMapInfo? map = info.fieldMap;
    DirectorObserverFeed.instance.pushModelLoaded(
      url: info.url,
      modelId: info.modelId,
      modelEpoch: info.modelEpoch,
      fieldMapSource: map?.source,
      fieldMapDegraded: map?.degraded ?? false,
      fieldMapReason: map?.reason,
    );
  }

  /// 换模**成功**（匹配 URL 的 loaded，0.3.6 T2）：清计划 + 重绑动作幅度。
  ///
  /// 只由 [Live2DStage.onModelSwapped] 调用——失败路径不回调，所以这里无需
  /// 再判「是否成功」：失败时旧模型、旧 cue、旧 epoch 一概不动。
  void _onStageModelSwapped(ModelLoadedInfo info) {
    if (!mounted) return;
    // ① **旧 cue 不得落到新模型**：换模前到达的按句计划整份作废（0.3.14 起
    //    用 `clear`；这条触发点是执行单 §行为 列的六个之一）。
    _directorCues.clear();
    // ② 按**渲染面确认的** model_id 重设 D40「每模型覆盖」上下文，并补发一次
    //    有效幅度。force 绕过同值去重（模型已换，旧快照不算数）；syncer 内部
    //    仍是临时覆盖（dev pin）优先——force 冲不掉调试面板钉住的值。
    final ActionSettingsView? action = _settings.remote?.action;
    _actionScalesSyncer.setModelContext(
      activeModelId: (info.modelId ?? '').isEmpty ? null : info.modelId,
      overrides: action == null
          ? const <String, Map<String, double>>{}
          : toModelOverrideMap(action.models),
    );
    _syncActionScalesNow(force: true);
    // ③ C 栏记下这次「两槽放回」的请求（真帧由舞台紧接着下发，source=swap）。
    _recordPresetRequest(id: 'none', source: 'swap');
  }

  void _didUpdateShell(ShellRoot oldWidget) {
    if (oldWidget.prefs == widget.prefs) return;
    // 已保存那份变了（保存成功 / 水合回填背景库）：它才是舞台与音频的真源，
    // 重新下发一次；轮播也重新对表（库长度 / 随机 / 间隔可能都变了）。
    //
    // 刻意**不**在这里清 `_prefsDraft`：保存成功由 `_saveAll` 自己清；水合回填
    // 只动背景域，用户在水合窗口里攒下的偏好草稿必须保留（与 F-0013-1 同一条
    // 教训：窗口期的改动不能被整份旧值回滚）。
    _applyPrefs();
    syncSlideshow(widget.prefs);
  }

  void _disposeShell() {
    unawaited(_levelSubscription?.cancel());
    unawaited(_statusSubscription?.cancel());
    unawaited(_eventSubscription?.cancel());
    unawaited(_sentenceCueSubscription?.cancel());
    unawaited(_stageClockSubscription?.cancel());
    // 先摘播放忙闲的监听（`_audio.dispose` 会销毁那个 ValueNotifier）。
    _disposePlaybackWiring();
    _actionScalesSyncer.dispose();
    _settings.removeListener(_onActionScalesModelContextChanged);
    _ui.dispose();
    _slideshow.dispose();
    _live.dispose();
    _settings.dispose();
    _prefsRevision.dispose();
    // T2（2026-10-13）：Mod 表单草稿登记处（卡片在 dispose 里注销，这里兜底）。
    _modDrafts.dispose();
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
