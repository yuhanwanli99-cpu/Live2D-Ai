part of 'package:live2d_ai_shell/main.dart';

extension _ShellCueVoiceWiring on _ShellRootState {
  void _recordPresetRequest({
    required String id,
    required String source,
    double? intensity,
    String? field,
    double? x,
    double? y,
    double? z,
    bool? hold,
    String? at,
    int? seq,
    int? sentenceSeq,
    int? epoch,
  }) {
    DirectorObserverFeed.instance.pushPresetRequest(
      PresetRequest(
        id: id,
        source: source,
        ts: DateTime.now(),
        intensity: intensity,
        field: field,
        x: x,
        y: y,
        z: z,
        hold: hold,
        at: at,
        seq: seq,
        sentenceSeq: sentenceSeq,
        epoch: epoch,
      ),
    );
  }

  /// 音频开始播放时按 sentence_seq 应用导演 cue（一次性；没 cue 什么都不做）。
  ///
  /// 锚点选**音频开始**（不是句子提交时刻）：慢 TTS 下前者才与声音同拍；
  /// 迟到 / 缺 cue 一律安静丢弃（导演是附加表演能力，不往聊天链路抛错误）。
  ///
  /// **唯一驱动通道 = WS `action_cue`（阶段3 D10–D13）**：不再拉
  /// director 的只读运行态（通道 B 已退役），`state_json.latest`
  /// 仅供 director 面板只读展示。撤销语义已并入 cue 层——`preset_id == 'none'`
  /// 照常下发（渲染面把 `none` 翻成两槽 `Revoke`）；`preset_id` 为空串仍是
  /// 「本轮不动」。按句覆盖 + 取用即移除由 [`DirectorCuePlan`] 负责，同 seq
  /// 重复帧只会应用一次。
  void _applyDirectorCueForSeq(int? seq) {
    unawaited(
      _directorCues.applyForSeq(seq, (ActionCue cue) {
        final Live2DStageState? stage = _stageKey.currentState;
        if (stage == null) return Future<void>.value();
        _recordPresetRequest(
          id: cue.presetId,
          source: 'director',
          intensity: cue.intensity.toDouble(),
          field: cue.field,
          x: cue.x,
          y: cue.y,
          z: cue.z,
          hold: cue.hold,
          at: cue.at,
          seq: cue.seq,
          sentenceSeq: cue.sentenceSeq,
          epoch: _directorEpoch,
        );
        // v1 三族字段原样透传（编排者冻结的集成细节）：消息仍是既有 `preset`，
        // 旧键一个不动、只增可选键；缺 field 时语义与今天逐字相同。
        return stage.applyPreset(
          cue.presetId,
              source: 'director',
              intensity: cue.intensity.toDouble(),
              ttlMs: cue.ttlMs.toDouble(),
              field: cue.field,
              x: cue.x,
              y: cue.y,
              z: cue.z,
              hold: cue.hold,
              at: cue.at,
              seq: cue.seq,
              epoch: _directorEpoch,
          sentenceSeq: cue.sentenceSeq,
        );
      }),
    );
  }

  /// 打断并重发：**先**同步取消编排，再走既有 stop（V7 §6.6 / V10 §9.3）。
  ///
  /// 顺序不能反：等到 stop 的服务端回执（new_epoch）才清动作，就会在没有声音时
  /// 继续把旧 cue 演完——那正是「追着播 / 补帧」。
  Future<void> _stopWithCancellation() async {
    _cancelStageForTurnBoundary('stop');
    await _stop();
  }

  /// 统一「停止」（圆键停止态 / Esc，2026-10-15 执行单 §2）：**永远**先取消
  /// 编排，**仅当这一轮还在飞**时再走 `ChatController.stop()`。
  ///
  /// 与 [_stopWithCancellation]（打断并重发）的差别只在后半段：打断并重发按
  /// 下去时一定有轮在飞；这一条停止在**服务端推流结束之后**也能按——那时若照
  /// 走 `_chat.stop()`，它会凭空收口一轮（落盘 + 一条「已停止本轮」），而用户
  /// 表达的只是「别把尾巴播完」。空闲时也不为了丢队列再打一次
  /// `POST /chat/stop`（它本来幂等，但没必要）。
  ///
  /// 条件与两步的顺序收在 [stopPlayback]（纯编排，VM 可测）。
  Future<void> _stopPlayback() => stopPlayback(
    turnInFlight: _chat.streaming,
    reason: 'stop',
    cancelStage: _cancelStageForTurnBoundary,
    stopTurn: _stop,
  );

  /// 发送（含重试 / 语音注入）：新消息同样先取消上一轮编排。
  Future<void> _sendWithCancellation() async {
    _cancelStageForTurnBoundary('new-message');
    await _send();
  }

  /// 停止 / 新消息的统一取消入口：清动作 + 表情 + TTS 待播 + 回 baseline。
  ///
  /// **同步取消、不补帧**：调用 [StageCancellation.cancel] 即刻生效；计划的
  /// 整表替换 + 音频队列清空保证之后不会重放任何旧 cue（V7 §6.6 / V10）。
  void _cancelStageForTurnBoundary(String reason) {
    _stageCancellation.cancel(reason);
  }

  /// 回该会话 baseline（V10 §9.3 / O8 / D28）——[StageCancellation] 的第 ④ 步。
  ///
  /// # 为什么这里先按「缺省 = 待机」下发
  ///
  /// host 的 baseline 随**同一个**请求两路交付（响应体的 `baseline` 字段；
  /// `action_cue{baseline:true}` 帧），但两路都要等 POST 之后才回到前端——
  /// **本地取消这一瞬间还没有可应用的内容**。按 O8「缺省为空 = 待机」，第 ④ 步
  /// 立即下发**缺省 baseline**（= `applyPreset('none')` 回待机），
  /// **不退化成「什么都不做」**；随后到达的交付内容若非空，由
  /// [_applyBaselineCues] 立即覆盖（同一个取消事务的异步后半段）。
  ///
  /// 前端**不缓存 baseline**：没有镜像字段，也没有第二套会话表。
  void _requestSessionBaseline(String reason) {
    _applyBaselineCues(const <ActionCue>[], reason);
  }

  /// 会话 baseline 的**第 ① 路**交付：HTTP 响应体（`api/api_client.dart` 转发）。
  ///
  /// 键缺席不会进来（[ApiClient.onSessionBaseline] 的契约）；显式 `null` /
  /// 空数组 = 待机 → 撤销。**立即应用**，不做任何缓存。
  void _onSessionBaselineDelivered(Object? baseline, String reason) {
    if (!mounted) return;
    _applyBaselineCues(parseBaselineCues(baseline), reason);
  }

  /// baseline 的一次性下发：**立即**逐条 `applyPreset`（不进按句计划）。
  ///
  /// 顺序保证：两路交付都发生在本地取消事务（清 cue 计划 → 撤销两槽 →
  /// 丢待播音频 → 第 ④ 步）**之后**——host 只有在收到 POST 时才广播 / 回包，
  /// 而本地取消在 POST **之前**。空 baseline ⇒ [baselineApplicationFromCues]
  /// 给**恰好一条** `none` 撤销（等价 `applyPreset('none')`）；
  /// 非空 ⇒ 每条 cue 各一次（`source` 用交付原因，便于渲染面 ack 归因）。
  void _applyBaselineCues(List<ActionCue> cues, String reason) {
    final Live2DStageState? stage = _stageKey.currentState;
    for (final BaselinePresetCall call in baselineApplicationFromCues(cues)) {
      if (stage != null) {
        _recordPresetRequest(
          id: call.id,
          source: reason,
          intensity: call.intensity?.toDouble() ?? kDefaultPresetIntensity,
          field: call.field,
          x: call.x,
          y: call.y,
          z: call.z,
          hold: call.hold,
          at: call.at,
          seq: call.seq,
          sentenceSeq: call.sentenceSeq,
        );
      }
      unawaited(
        stage?.applyPreset(
              call.id,
              source: reason,
              intensity: call.intensity?.toDouble() ?? kDefaultPresetIntensity,
              ttlMs: call.ttlMs?.toDouble(),
              field: call.field,
              x: call.x,
              y: call.y,
              z: call.z,
              hold: call.hold,
              at: call.at,
              seq: call.seq,
              sentenceSeq: call.sentenceSeq,
            ) ??
            Future<void>.value(),
      );
    }
  }

  /// 一条语音正文**唯一**的落点：写进输入框（不排队、不静默丢弃）。
  ///
  /// - **产品听写的定稿**（成功与失败都走它；空串 = 没听清 → 什么都不做，
  ///   控制器已经写了「没听清，再点一次说」）。
  ///
  /// 2026-11-16：识别只出文本、**不自动发送**（端点不调 `say`），所以这里也
  /// 只剩「写进输入框」一件事——不再有「主链忙 → 正文交回输入框」那条支路。
  ///
  /// 输入框里已有文字时**接在后面**（中间补一个空格），光标落到末尾；
  /// 不 `setState`——`TextEditingController` 自己会通知输入框重建。
  void _writeVoiceTextToInput(String text) {
    if (!mounted) return;
    // 拼接口径收在纯函数里（`appendVoiceText`，有单测）：空串 = 输入框不动。
    final String next = appendVoiceText(_input.text, text);
    if (next == _input.text) return;
    _input.value = TextEditingValue(
      text: next,
      selection: TextSelection.collapsed(offset: next.length),
    );
  }

  /// [VoiceListenController] 的听写定稿回调（成功与失败共用它）。
  void _onVoiceResult(String text) => _writeVoiceTextToInput(text);
}
