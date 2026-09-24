/// voice-input 的产品面板（2026-09-15 用户裁决：**大幅瘦身**）。
///
/// # 主区只留四件事
///
/// 1. 唤醒词与两把闸的**一行状态**（缺省唤醒词「小可爱」）；
/// 2. 一句白话链路：说「小可爱 ……」→ 后半句进聊天 → LLM → TTS；
/// 3. 「检查配置」自检按钮（结论自带错误码）；
/// 4. 关闭时的**可处置**提示。
///
/// # 「高级」折叠里放什么
///
/// 手动注入一条转写（验证闸门）、官方 sidecar 拉起、失败码 / 退出码长文——
/// 这些是排障与集成用的，不是日常路径。配置字段本身由通用表单渲染，
/// 面板用 [advancedKeys] 把 sidecar / backend / locale / token 也收进「高级」。
///
/// 面板自己**不做网络 / 不起进程**：有副作用动作一律经
/// [ModPanelContext.onCommand] 转给宿主，失败以 [ApiException.code] 呈现。
/// 本文件由 voice-input 轨道独占。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../ui/inline_notice.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';
import 'mod_panel.dart';

/// 浏览器识别的**缺省唤醒词**（与 Rust 侧 gate::DEFAULT_WAKE_PHRASE 逐字一致）。
const String kVoiceDefaultWakePhrase = '小可爱';

/// sidecar 脚本的退出码契约（与 docs/voice-input.md §4.5 同源）。
const List<(String, String)> kVoiceSidecarExitCodes = <(String, String)>[
  ('0', '成功：HTTP 2xx 且 ok:true，转写已注入主链。'),
  ('2', '参数或依赖错：缺 --audio、音频读不到、--transcriber 写法非法、ASR 命令不存在。'),
  ('3', '转写失败：同名 .txt 读不到、ASR 命令非 0 退出或超时、清洗后为空。'),
  ('4', '推送失败：HTTP 非 2xx 或连接被拒 / 超时。按错误码处置后退避重试。'),
  ('5', '服务端 ok:false（目前只有 busy）：主链忙，本条已丢弃。等 2 到 5 秒再发。'),
];

/// 把浏览器 origin 拼成我们的 transcript 端点 URL（**纯函数**，便于单测）。
String voiceTranscriptUrl(Uri? base) {
  const String fallback = 'http://127.0.0.1:18080/api/v1/voice/transcript';
  if (base == null) return fallback;
  final String scheme = base.scheme.toLowerCase();
  if (scheme != 'http' && scheme != 'https') return fallback;
  if (base.host.isEmpty) return fallback;
  final String port = base.hasPort ? ':${base.port}' : '';
  return '$scheme://${base.host}$port/api/v1/voice/transcript';
}

class VoiceInputPanel extends ModPanel {
  const VoiceInputPanel();

  @override
  String get modId => 'voice-input';

  /// 主区之外的字段收进「高级」：sidecar 路径 / ASR 命令 / 令牌 / 后端 / locale。
  @override
  Set<String> get advancedKeys => const <String>{
    'backend',
    'locale',
    'token',
    'sidecar_script',
    'sidecar_url',
    'sidecar_transcriber',
    'sidecar_python',
  };

  @override
  Map<String, String> get stateLabels => const <String, String>{
    'wake_gate_open': '语音总闸',
    'wake_phrase_set': '唤醒词已自定义',
    'manual_enabled': '手动闸',
    'sidecar_status': 'sidecar 运行态',
  };

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) =>
      _VoiceInputPanelBody(ctx: ctx);
}

class _VoiceInputPanelBody extends StatefulWidget {
  const _VoiceInputPanelBody({required this.ctx});

  final ModPanelContext ctx;

  @override
  State<_VoiceInputPanelBody> createState() => _VoiceInputPanelBodyState();
}

class _VoiceInputPanelBodyState extends State<_VoiceInputPanelBody> {
  bool _checking = false;
  String? _checkError;
  Map<String, Object?>? _checkResult;

  final TextEditingController _injectController = TextEditingController();
  bool _injecting = false;
  String? _injectError;
  Map<String, Object?>? _injectResult;

  final TextEditingController _audioController = TextEditingController();
  final TextEditingController _transcriberController =
      TextEditingController(text: 'fake');
  late final TextEditingController _urlController;

  bool _spawning = false;
  String? _sidecarError;
  Map<String, Object?>? _sidecarResult;

  @override
  void initState() {
    super.initState();
    _urlController = TextEditingController(text: voiceTranscriptUrl(Uri.base));
  }

  @override
  void dispose() {
    _injectController.dispose();
    _audioController.dispose();
    _transcriberController.dispose();
    _urlController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: appColorsOf(context).contentMuted,
    );
    final Map<String, Object?> config = widget.ctx.mod.config;
    final String phrase = _effectiveWakePhrase(config);
    final bool gateOpen =
        _boolState('wake_gate_open') ?? _configuredWakeOpen(config);
    final bool phraseCustom = _boolState('wake_phrase_set') ?? false;
    final bool manualEnabled =
        _boolState('manual_enabled') ??
            (config['manual_enabled'] is bool
                ? config['manual_enabled']! as bool
                : true);
    final String gateText = gateOpen
        ? '已开（唤醒词「$phrase」${phraseCustom ? '，自定义' : '，缺省'}）'
        : '已关（wake_phrase 显式留空）';

    return Padding(
      padding: const EdgeInsets.only(top: Space.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text('总闸：$gateText；手动闸：${manualEnabled ? '已开' : '已关'}',
              style: theme.textTheme.bodySmall),
          const SizedBox(height: Space.s1),
          Text(
            '对着麦克风说「$phrase 今天天气怎么样」——命中唤醒词后，'
            '后面那句会进聊天 → LLM → 语音合成。',
            style: muted,
          ),
          if (!gateOpen) ...<Widget>[
            const SizedBox(height: Space.s2),
            const InlineNotice(
              severity: NoticeSeverity.warning,
              message: '语音总闸被显式关闭（wake_phrase 留空）：所有转写都会被拒绝（403 voice_gate_closed）。'
                  '在上面配置区填一个唤醒词再保存即可。',
            ),
          ],
          if (!manualEnabled) ...<Widget>[
            const SizedBox(height: Space.s2),
            const InlineNotice(
              severity: NoticeSeverity.warning,
              message: '手动闸已关：所有转写都会被拒绝（403 voice_manual_off）。'
                  '在上面配置区打开 manual_enabled 再保存。',
            ),
          ],
          const SizedBox(height: Space.s3),
          _buildCheckSection(),
          const SizedBox(height: Space.s2),
          _buildAdvanced(theme, muted),
        ],
      ),
    );
  }

  // ------------------------------------------------------------ 高级（折叠）

  Widget _buildAdvanced(ThemeData theme, TextStyle? muted) {
    return ExpansionTile(
      tilePadding: EdgeInsets.zero,
      childrenPadding: const EdgeInsets.only(left: Space.s2),
      expandedCrossAxisAlignment: CrossAxisAlignment.start,
      title: Text('高级', style: theme.textTheme.titleSmall),
      children: <Widget>[
        const SizedBox(height: Space.s2),
        _buildInjectSection(),
        const SizedBox(height: Space.s3),
        _buildSidecarSection(_statusMap(), muted),
        const SizedBox(height: Space.s3),
        Text('常见失败码与 sidecar 退出码（排障用）', style: theme.textTheme.labelLarge),
        const SizedBox(height: Space.s1),
        ..._failureLines(muted),
        const SizedBox(height: Space.s1),
        for (final (String code, String human) in kVoiceSidecarExitCodes)
          Text('退出码 $code：$human', style: muted),
      ],
    );
  }

  // ------------------------------------------------------------ 检查配置

  Widget _buildCheckSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        const SectionHeader(
          title: '配置自检',
          description: '服务端只回结论，不回 token 明文。',
        ),
        const SizedBox(height: Space.s2),
        Align(
          alignment: Alignment.centerLeft,
          child: FilledButton.tonal(
            onPressed: _checking || !widget.ctx.enabled ? null : _runCheck,
            child: Text(_checking ? '检查中…' : '检查配置'),
          ),
        ),
        if (!widget.ctx.enabled)
          const Padding(
            padding: EdgeInsets.only(top: Space.s1),
            child: InlineNotice(
              severity: NoticeSeverity.info,
              dense: true,
              message: 'voice-input Mod 未启用：命令会回 503 command_unavailable，先在上面打开开关。',
            ),
          ),
        if (_checkError != null) ...<Widget>[
          const SizedBox(height: Space.s2),
          InlineNotice(message: _checkError!),
        ],
        if (_checkResult != null) ...<Widget>[
          const SizedBox(height: Space.s2),
          InlineNotice(
            severity: _checkResult!['ok'] == true
                ? NoticeSeverity.info
                : NoticeSeverity.warning,
            message: _formatCheck(_checkResult!),
          ),
        ],
      ],
    );
  }

  // ------------------------------------------------------------ 验证闸门

  Widget _buildInjectSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        const SectionHeader(
          title: '验证闸门（手动注入一条转写）',
          description: '走与端点同一条 gate + 清洗 + say 路径；被拒绝时如实显示 rejected_code。',
        ),
        const SizedBox(height: Space.s2),
        TextField(
          controller: _injectController,
          enabled: widget.ctx.enabled,
          decoration: InputDecoration(
            labelText: '转写文本（总闸开时必须包含「$kVoiceDefaultWakePhrase」或缺省词）',
            border: const OutlineInputBorder(),
            isDense: true,
          ),
        ),
        const SizedBox(height: Space.s2),
        Align(
          alignment: Alignment.centerLeft,
          child: FilledButton.tonal(
            onPressed: widget.ctx.enabled && !_injecting ? _runInject : null,
            child: Text(_injecting ? '注入中…' : '验证闸门'),
          ),
        ),
        if (_injectError != null) ...<Widget>[
          const SizedBox(height: Space.s2),
          InlineNotice(message: _injectError!),
        ],
        if (_injectResult != null) ...<Widget>[
          const SizedBox(height: Space.s2),
          InlineNotice(
            severity: _injectResult!['accepted'] == true
                ? NoticeSeverity.info
                : NoticeSeverity.warning,
            message: _formatInject(_injectResult!),
          ),
        ],
      ],
    );
  }

  // ------------------------------------------------------------ sidecar

  Widget _buildSidecarSection(
    Map<String, Object?> status,
    TextStyle? muted,
  ) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        const SectionHeader(
          title: '用官方 sidecar 识别音频文件',
          description: '宿主 spawn 官方脚本（逐参数，不过 shell），脚本把转写推回主链。',
        ),
        const SizedBox(height: Space.s2),
        TextField(
          controller: _audioController,
          enabled: widget.ctx.enabled,
          decoration: const InputDecoration(
            labelText: '音频文件路径（宿主本机）',
            border: OutlineInputBorder(),
            isDense: true,
          ),
        ),
        const SizedBox(height: Space.s2),
        TextField(
          controller: _transcriberController,
          enabled: widget.ctx.enabled,
          decoration: const InputDecoration(
            labelText: 'transcriber（缺省 fake，读同名 .txt）',
            border: OutlineInputBorder(),
            isDense: true,
          ),
        ),
        const SizedBox(height: Space.s2),
        TextField(
          controller: _urlController,
          enabled: widget.ctx.enabled,
          decoration: const InputDecoration(
            labelText: 'sidecar 要 POST 的 URL',
            border: OutlineInputBorder(),
            isDense: true,
          ),
        ),
        const SizedBox(height: Space.s2),
        Row(
          children: <Widget>[
            FilledButton.tonal(
              onPressed: widget.ctx.enabled && !_spawning ? _runSidecar : null,
              child: Text(_spawning ? '拉起中…' : '拉起 sidecar'),
            ),
            const SizedBox(width: Space.s2),
            TextButton(
              onPressed: widget.ctx.stateLoading
                  ? null
                  : () => widget.ctx.onRefreshState(),
              child: const Text('刷新状态'),
            ),
          ],
        ),
        if (widget.ctx.stateError != null) ...<Widget>[
          const SizedBox(height: Space.s2),
          InlineNotice(message: widget.ctx.stateError!),
        ],
        const SizedBox(height: Space.s2),
        Text(_sidecarStatusLine(status), style: muted),
        if (_sidecarError != null) ...<Widget>[
          const SizedBox(height: Space.s2),
          InlineNotice(message: _sidecarError!),
        ],
        if (_sidecarResult != null) ...<Widget>[
          const SizedBox(height: Space.s2),
          InlineNotice(
            severity: _sidecarResult!['spawned'] == true
                ? NoticeSeverity.info
                : NoticeSeverity.warning,
            message: _formatSidecar(_sidecarResult!),
          ),
        ],
      ],
    );
  }

  // ------------------------------------------------------------ 运行态读取

  bool? _boolState(String key) {
    final Object? raw = widget.ctx.state?[key];
    return raw is bool ? raw : null;
  }

  Map<String, Object?> _statusMap() {
    final Object? raw = widget.ctx.state?['sidecar_status'];
    if (raw is Map) {
      return raw.map<String, Object?>(
        (Object? k, Object? v) => MapEntry<String, Object?>(k.toString(), v),
      );
    }
    return const <String, Object?>{};
  }

  /// config 里**生效**的唤醒词：键缺失 → 缺省「小可爱」；显式空 → 空串。
  String _effectiveWakePhrase(Map<String, Object?> config) {
    final Object? raw = config['wake_phrase'];
    if (raw is String) return raw.trim();
    return kVoiceDefaultWakePhrase;
  }

  bool _configuredWakeOpen(Map<String, Object?> config) =>
      _effectiveWakePhrase(config).isNotEmpty;

  String _sidecarStatusLine(Map<String, Object?> status) {
    if (status.isEmpty) {
      return 'sidecar 状态：还没拉起过。';
    }
    final String state = _value(status['state']);
    final Object? exitCode = status['exit_code'];
    final Object? audio = status['last_audio'];
    final Object? tail = status['stderr_tail'];
    final StringBuffer buffer = StringBuffer('sidecar 状态：$state');
    if (exitCode != null) buffer.write('，退出码 $exitCode');
    if (audio != null) buffer.write('，最近音频 $audio');
    if (tail != null && tail.toString().trim().isNotEmpty) {
      buffer.write('（stderr 尾巴见下）\n$tail');
    }
    return buffer.toString();
  }

  // ------------------------------------------------------------ 动作

  Future<void> _runCheck() async {
    setState(() {
      _checking = true;
      _checkError = null;
      _checkResult = null;
    });
    try {
      final ModCommandResult result = await widget.ctx.onCommand('selftest');
      if (!mounted) return;
      setState(() {
        _checking = false;
        _checkResult = result.result;
      });
    } on ApiException catch (error) {
      if (!mounted) return;
      setState(() {
        _checking = false;
        _checkError = '自检失败：${error.code} —— ${error.message}';
      });
    } catch (error) {
      if (!mounted) return;
      setState(() {
        _checking = false;
        _checkError = '自检失败：error —— $error';
      });
    }
  }

  Future<void> _runInject() async {
    setState(() {
      _injecting = true;
      _injectError = null;
      _injectResult = null;
    });
    try {
      final ModCommandResult result = await widget.ctx.onCommand(
        'inject',
        <String, Object?>{'text': _injectController.text},
      );
      if (!mounted) return;
      setState(() {
        _injecting = false;
        _injectResult = result.result;
      });
    } on ApiException catch (error) {
      if (!mounted) return;
      setState(() {
        _injecting = false;
        _injectError = '验证闸门失败：${error.code} —— ${error.message}';
      });
    } catch (error) {
      if (!mounted) return;
      setState(() {
        _injecting = false;
        _injectError = '验证闸门失败：error —— $error';
      });
    }
  }

  Future<void> _runSidecar() async {
    setState(() {
      _spawning = true;
      _sidecarError = null;
      _sidecarResult = null;
    });
    final String audio = _audioController.text.trim();
    final String transcriber = _transcriberController.text.trim();
    final String url = _urlController.text.trim();
    try {
      final ModCommandResult result = await widget.ctx.onCommand(
        'run_sidecar',
        <String, Object?>{
          'audio_path': audio,
          if (transcriber.isNotEmpty) 'transcriber': transcriber,
          if (url.isNotEmpty) 'url': url,
        },
      );
      if (!mounted) return;
      setState(() {
        _spawning = false;
        _sidecarResult = result.result;
      });
      if (result.result['spawned'] == true) {
        widget.ctx.notifyChanged('已拉起官方 sidecar');
      }
      try {
        await widget.ctx.onRefreshState();
      } catch (_) {
        // 刷新失败已在别处由 stateError 呈现。
      }
    } on ApiException catch (error) {
      if (!mounted) return;
      setState(() {
        _spawning = false;
        _sidecarError = '拉起 sidecar 失败：${error.code} —— ${error.message}';
      });
    } catch (error) {
      if (!mounted) return;
      setState(() {
        _spawning = false;
        _sidecarError = '拉起 sidecar 失败：error —— $error';
      });
    }
  }

  // ------------------------------------------------------------ 文案

  String _formatInject(Map<String, Object?> result) {
    if (result['accepted'] == true) {
      return '已注入主链：${_value(result['text'])}';
    }
    final Object? code = result['rejected_code'];
    final String message = _value(result['message']);
    if (code != null) {
      return '被拒绝（$code）：$message';
    }
    return '未注入：$message';
  }

  String _formatSidecar(Map<String, Object?> result) {
    if (result['spawned'] == true) {
      return '已拉起 sidecar（pid ${_value(result['pid'])}）；'
          '它会把转写推到主链，回聊天区看角色开口。';
    }
    return '没拉起 sidecar：${_value(result['error'])}';
  }

  /// 常见失败 -> 一句人话 + 处置（文字表意，不靠颜色）。
  List<Widget> _failureLines(TextStyle? style) => <Widget>[
        Text(
          '403 voice_gate_closed：总闸被显式留空。在上面配置区填一个唤醒词（缺省 小可爱）再保存。',
          style: style,
        ),
        Text(
          '403 voice_manual_off / 400 wake_phrase_required：手动闸关了，或这句话里没有唤醒词。',
          style: style,
        ),
        Text(
          '403 mod_disabled：voice-input Mod 没启用。到「Mod 管理」打开它。',
          style: style,
        ),
        Text(
          '200 但 ok:false（code=busy）：主链忙，这条已丢弃。等 2 到 5 秒再发。',
          style: style,
        ),
        Text(
          '401 unauthorized / 403 origin_required：对齐 token，或核对发送方 URL/端口。',
          style: style,
        ),
      ];

  /// 自检结果 -> 一段可读摘要（问题 / 提醒逐条列出）。
  String _formatCheck(Map<String, Object?> result) {
    final bool ok = result['ok'] == true;
    final String backend = _value(result['backend']);
    final String locale = _value(result['locale']);
    final Object? profile = result['locale_profile'];
    final Object? route = result['route'];
    final String token = result['token_set'] == true ? '已设置' : '未设置';
    final String network = result['opens_network'] == true ? '是' : '否';
    final String gate = result['wake_gate_open'] == true ? '已开' : '已关';
    final String manual = result['manual_enabled'] == true ? '已开' : '已关';
    final StringBuffer buffer = StringBuffer(ok ? '配置自洽。' : '发现配置问题。');
    buffer.write('生效 backend=$backend，locale=$locale');
    if (profile != null) buffer.write('（归一化档 $profile）');
    buffer.write('，token=$token，总闸=$gate，手动闸=$manual');
    if (route != null) buffer.write('，本地路由 $route');
    buffer.write('，Rust 开网络=$network。');
    for (final Object? problem in _stringList(result['problems'])) {
      buffer.write('\n问题：${_value(problem)}');
    }
    for (final Object? note in _stringList(result['notes'])) {
      buffer.write('\n提醒：${_value(note)}');
    }
    return buffer.toString();
  }

  static String _value(Object? raw) => raw == null ? '未知' : raw.toString();

  static List<Object?> _stringList(Object? raw) =>
      raw is List ? raw : const <Object?>[];
}
