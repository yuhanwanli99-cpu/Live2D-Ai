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
  /// 「本轮不动」。整表替换 + 取用即移除由 [`DirectorCuePlan`] 负责，同 seq
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

  /// 停止键：**先**同步取消编排，再走既有 stop（V7 §6.6 / V10 §9.3）。
  ///
  /// 顺序不能反：等到 stop 的服务端回执（new_epoch）才清动作，就会在没有声音时
  /// 继续把旧 cue 演完——那正是「追着播 / 补帧」。
  Future<void> _stopWithCancellation() async {
    _cancelStageForTurnBoundary('stop');
    await _stop();
  }

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

  /// 主链忙时，把识别到的正文**落回输入框**（不排队、不静默丢弃）。
  ///
  /// 不 `setState`：`TextEditingController` 自己会通知输入框重建，这里只改值。
  void _onVoiceBusyResult(String text) {
    if (!mounted || text.trim().isEmpty) return;
    _input.value = TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
    );
  }

  /// voice-input Mod 是否启用（「听」按钮预检；读失败 → `true` 不误拦）。
  ///
  /// 缓存命中直接返回；首次（或用户在设置里改过启停后清缓存）走一次
  /// `GET /api/v1/mods`。**读不到不拦**——真发出去时服务端仍会如实回 403。
  Future<bool> _voiceInputModEnabled() async {
    final bool? cached = _voiceInputEnabled;
    if (cached != null) return cached;
    try {
      final List<ModInfo> mods = await _modsApi.list();
      if (mounted) {
        _mods = mods;
        _voiceInputEnabled = _enabledOf(mods, 'voice-input');
        _refresh();
      }
      return _enabledOf(mods, 'voice-input');
    } catch (_) {
      return true;
    }
  }

  /// 读当前生效的唤醒词：voice-input Mod 的 `config.wake_phrase`。
  ///
  /// 键缺失 / Mod 不在册 / 读失败 → 产品缺省「小可爱」（与 Rust 侧
  /// `gate::DEFAULT_WAKE_PHRASE` 逐字一致）。**显式空**（键存在且为空）
  /// 也回落缺省：服务端此时会拒一切转写，按钮仍能把这条错误如实显示出来；
  /// 本地不需要复刻「空 = 总闸关」的第二套判定。
  Future<String> _loadWakePhrase() async {
    final List<ModInfo> mods = await _modsApi.list();
    for (final ModInfo m in mods) {
      if (m.id != 'voice-input') continue;
      final Object? raw = m.config['wake_phrase'];
      if (raw is String && raw.trim().isNotEmpty) return raw.trim();
    }
    return kDefaultWakePhrase;
  }
}
