/// voice-input 的产品面板（L1 产品级，2026-09-15）。
///
/// 职责：
/// 1. 把 backend / locale 的语义说成人话，显示**当前生效值**（读 Mod config）；
/// 2. **能力总闸**一块：总闸 = 唤醒短语（`wake_phrase`，空 = 关），
///    关着时用醒目文案说清「所有转写都会被拒绝（403 voice_gate_closed）」；
/// 3. `selftest` 自检（「检查配置」按钮）；
/// 4. **「验证闸门」**：手动注入一条转写（命令 `inject`），把
///    `accepted` / `rejected_code` 如实摊开——这就是「总闸关时拒绝且可读」的壳内证据；
/// 5. **硬主路径**：「用官方 sidecar 识别音频文件」——命令 `run_sidecar`，
///    宿主 spawn 官方脚本，脚本把转写推回主链。
///
/// 面板自己**不做网络 / 不起进程**：有副作用动作一律经
/// [ModPanelContext.onCommand] 转给宿主，失败以 [ApiException.code] 呈现
/// （项目错误契约：错误必须带码）。本文件由 voice-input 轨道独占。
library;

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../ui/inline_notice.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';
import 'mod_panel.dart';

/// sidecar 脚本的退出码契约（与 docs/voice-input.md §4.5 /
/// docs/examples/voice-sidecar/README.md §5 同源）。
///
/// 放在组件文件里而不是从后端取：这五条是**脚本的退出码**，面板要能在
/// 没起 sidecar、没起服务时就讲清楚，不能依赖任何请求。
const List<(String, String)> kVoiceSidecarExitCodes = <(String, String)>[
  ('0', '成功：HTTP 2xx 且 ok:true，转写已注入主链。'),
  ('2', '参数或依赖错：缺 --audio、音频文件读不到、--transcriber 写法非法、ASR 命令不存在。先修命令再重跑。'),
  ('3', '转写失败：同名 .txt 读不到、ASR 命令非 0 退出或超时、清洗后为空。换一段音频或检查 ASR。'),
  ('4', '推送失败：HTTP 非 2xx（400 / 401 / 403 / 405 / 415 / 503）或连接被拒 / 超时。按下方错误码处置后退避重试。'),
  ('5', '服务端 ok:false：目前只有 busy（主链忙，本条已丢弃）。等 2 到 5 秒再发。'),
];

/// 把浏览器 origin 拼成我们的 transcript 端点 URL（**纯函数**，便于单测）。
///
/// - `base` 为空 / 非 http(s) / 没有 host（例如 Flutter VM 下的 `file:`）→
///   回退到 `http://127.0.0.1:18080/api/v1/voice/transcript`（ignite.sh 默认端口）；
/// - 否则 `<scheme>://<host>[:port]/api/v1/voice/transcript`。
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

  /// 运行态字段的中文标签（合并进统一的运行态渲染；未知 key 不隐藏）。
  @override
  Map<String, String> get stateLabels => const <String, String>{
    'wake_gate_open': '能力总闸（唤醒短语）',
    'wake_phrase_set': '唤醒短语已设置',
    'manual_enabled': '手动闸',
    'sidecar_script': 'sidecar 脚本路径',
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

    final Object? rawBackend = config['backend'];
    final bool backendKnown =
        rawBackend == null || rawBackend == 'mock' || rawBackend == 'sidecar';
    final String backend = rawBackend == 'sidecar' ? 'sidecar' : 'mock';
    final Object? rawLocale = config['locale'];
    final String trimmedLocale = rawLocale is String ? rawLocale.trim() : '';
    final bool localeDefaulted = trimmedLocale.isEmpty;
    final String locale = localeDefaulted ? 'zh-CN' : trimmedLocale;
    final bool localeValid = _localeLooksValid(locale);

    final bool configuredWake = _configuredWakeOpen(config);
    final bool gateOpen = _boolState('wake_gate_open') ?? configuredWake;
    final bool manualEnabled =
        _boolState('manual_enabled') ??
            (config['manual_enabled'] is bool
                ? config['manual_enabled']! as bool
                : true);
    final Map<String, Object?> sidecarStatus = _statusMap();
    final String gateText =
        gateOpen ? '已开（wake_phrase 已设置）' : '未开（wake_phrase 为空）';
    final String manualText = manualEnabled ? '已开' : '已关（manual_enabled=false）';

    return Padding(
      padding: const EdgeInsets.only(top: Space.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          const SectionHeader(
            title: '能力总闸',
            description: '总闸就是「唤醒短语」（wake_phrase）：留空 = 总闸关，'
                '所有转写都会被拒绝（403 voice_gate_closed）。',
          ),
          const SizedBox(height: Space.s2),
          Text(
            '总闸：$gateText；手动闸：$manualText。',
            style: theme.textTheme.bodySmall,
          ),
          Text(
            '总闸与手动闸缺任一，转写都进不了主链；总闸开时文本还必须包含唤醒短语，'
            '命中后短语会从正文里被剥掉。',
            style: muted,
          ),
          if (!gateOpen) ...<Widget>[
            const SizedBox(height: Space.s2),
            const InlineNotice(
              severity: NoticeSeverity.warning,
              message: '总闸未开：所有转写都会被拒绝（403 voice_gate_closed）。'
                  '去上面的配置区填「唤醒短语」再保存。',
            ),
          ],
          if (!manualEnabled) ...<Widget>[
            const SizedBox(height: Space.s2),
            const InlineNotice(
              severity: NoticeSeverity.warning,
              message: '手动闸已关：所有转写都会被拒绝（403 voice_manual_off）。'
                  '去上面的配置区打开 manual_enabled 再保存。',
            ),
          ],
          const SizedBox(height: Space.s3),
          const SectionHeader(
            title: '这个开关管什么（白话）',
            description: 'backend 决定「谁把转写文本交给角色」；locale 只决定文本怎么归一化，不是识别语言开关。',
          ),
          const SizedBox(height: Space.s2),
          Text(
            'mock：没有 ASR。由集成方或测试直接把转写文本喂进来，不需要任何识别进程。',
            style: muted,
          ),
          Text(
            'sidecar：外部 ASR 进程自己识别，再把文本推到 POST /api/v1/voice/transcript。Rust 侧从不开 socket、也不会主动请求 sidecar。',
            style: muted,
          ),
          Text(
            'locale 只影响转写文本的归一化（CJK 词间空格、中英边界），不是「识别语言开关」；识别语言由 sidecar 的 --transcriber 决定。',
            style: muted,
          ),
          const SizedBox(height: Space.s3),
          const SectionHeader(
            title: '当前生效值',
            description: '读的是这个 Mod 已保存的配置；缺省 mock / zh-CN。',
          ),
          const SizedBox(height: Space.s2),
          Text(_backendLine(backend, backendKnown, rawBackend),
              style: theme.textTheme.bodySmall),
          Text(_localeLine(locale, localeDefaulted),
              style: theme.textTheme.bodySmall),
          if (rawBackend != null && !backendKnown)
            const Padding(
              padding: EdgeInsets.only(top: Space.s1),
              child: InlineNotice(
                severity: NoticeSeverity.warning,
                dense: true,
                message: 'backend 写了 mock / sidecar 之外的值，服务端已按 mock 处理（没有 ASR）。'
                    '要等外部 ASR 推文本，请把它改成 sidecar。',
              ),
            ),
          if (!localeValid)
            const Padding(
              padding: EdgeInsets.only(top: Space.s1),
              child: InlineNotice(
                severity: NoticeSeverity.warning,
                dense: true,
                message: 'locale 不是合法 BCP-47（如 zh-CN）：归一化会落到「拉丁」档，'
                    '中文词间空格不会被删。',
              ),
            ),
          const SizedBox(height: Space.s3),
          _buildCheckSection(),
          const SizedBox(height: Space.s3),
          _buildInjectSection(),
          const SizedBox(height: Space.s3),
          _buildSidecarSection(sidecarStatus, muted),
          const SizedBox(height: Space.s3),
          const SectionHeader(
            title: '出错怎么办',
            description: '前几条是面板 / 服务端能直接告诉你的；最后五条是 sidecar 脚本的退出码。',
          ),
          const SizedBox(height: Space.s2),
          ..._failureLines(muted),
          const SizedBox(height: Space.s2),
          for (final (String code, String human) in kVoiceSidecarExitCodes)
            Text(_exitCodeLine(code, human), style: muted),
        ],
      ),
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
          decoration: const InputDecoration(
            labelText: '转写文本（总闸开时必须包含唤醒短语）',
            border: OutlineInputBorder(),
            isDense: true,
          ),
        ),
        const SizedBox(height: Space.s2),
        Align(
          alignment: Alignment.centerLeft,
          child: FilledButton.tonal(
            onPressed:
                widget.ctx.enabled && !_injecting ? _runInject : null,
            child: Text(_injecting ? '注入中…' : '验证闸门'),
          ),
        ),
        if (!widget.ctx.enabled)
          const Padding(
            padding: EdgeInsets.only(top: Space.s1),
            child: InlineNotice(
              severity: NoticeSeverity.info,
              dense: true,
              message: 'voice-input Mod 未启用：按钮禁用（命令会回 503 command_unavailable）。',
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

  // ------------------------------------------------------------ 硬主路径：sidecar

  Widget _buildSidecarSection(
    Map<String, Object?> status,
    TextStyle? muted,
  ) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        const SectionHeader(
          title: '用官方 sidecar 识别音频文件',
          description: '硬主路径：宿主 spawn 官方脚本（逐参数，不过 shell），'
              '脚本把转写推回主链，回聊天区看角色开口。',
        ),
        const SizedBox(height: Space.s2),
        TextField(
          controller: _audioController,
          enabled: widget.ctx.enabled,
          decoration: const InputDecoration(
            labelText: '音频文件路径（宿主本机）',
            hintText: '例如 docs/examples/voice-sidecar/fixtures/fake_zh.wav',
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
            hintText: 'fake 或 cmd:"whisper --model small --language zh"',
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
              onPressed:
                  widget.ctx.enabled && !_spawning ? _runSidecar : null,
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
        if (!widget.ctx.enabled)
          const Padding(
            padding: EdgeInsets.only(top: Space.s1),
            child: InlineNotice(
              severity: NoticeSeverity.info,
              dense: true,
              message: 'voice-input Mod 未启用：按钮禁用（命令会回 503 command_unavailable）。',
            ),
          ),
        if (widget.ctx.stateError != null) ...<Widget>[
          const SizedBox(height: Space.s2),
          InlineNotice(message: widget.ctx.stateError!),
        ],
        const SizedBox(height: Space.s2),
        Text(
          '官方 sidecar 会按目标 URL 自动带上同源 loopback Origin 头，'
          '所以标准 ./scripts/ignite.sh 点火即可，不需要放松服务端的 Origin 校验。',
          style: muted,
        ),
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

  /// config 里「唤醒短语是否非空」（state 还没取到时的回退判据）。
  bool _configuredWakeOpen(Map<String, Object?> config) {
    final Object? raw = config['wake_phrase'];
    return raw is String && raw.trim().isNotEmpty;
  }

  String _sidecarStatusLine(Map<String, Object?> status) {
    if (status.isEmpty) {
      return 'sidecar 状态：还没拉起过（点上面的「拉起 sidecar」，或「刷新状态」）。';
    }
    final String state = _value(status['state']);
    final Object? exitCode = status['exit_code'];
    final Object? audio = status['last_audio'];
    final Object? tail = status['stderr_tail'];
    final StringBuffer buffer = StringBuffer('sidecar 状态：$state');
    if (exitCode != null) buffer.write('，退出码 $exitCode');
    if (audio != null) buffer.write('，最近音频 $audio');
    if (tail != null && tail.toString().trim().isNotEmpty) {
      buffer.write('\nstderr 尾巴：$tail');
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
      // 顺带刷一次运行态，让 sidecar_status 立刻可见（失败不影响上面的结论）。
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
          '它会把转写推到主链，回聊天区看角色开口。'
          '目标 URL：${_value(result['url'])}';
    }
    return '没拉起 sidecar：${_value(result['error'])}';
  }

  /// backend 生效值那一行（把「缺省」与「配错回落」分开说）。
  String _backendLine(String backend, bool known, Object? raw) {
    if (!known) {
      return 'backend：$backend（配置值 $raw 不认识，已回落 mock）';
    }
    final String suffix = raw == null ? '（缺省，未显式设置）' : '（已设置）';
    return 'backend：$backend$suffix';
  }

  /// locale 生效值那一行。
  String _localeLine(String locale, bool defaulted) {
    final String suffix = defaulted ? '（缺省，未显式设置）' : '（已设置）';
    return 'locale：$locale$suffix';
  }

  /// sidecar 退出码那一行（码 -> 一句话人话）。
  String _exitCodeLine(String code, String human) => '退出码 $code：$human';

  /// 常见失败 -> 一句人话 + 处置（文字表意，不靠颜色）。
  List<Widget> _failureLines(TextStyle? style) => <Widget>[
        Text(
          '403 voice_gate_closed：能力总闸未开（没配唤醒短语）。到上面配置区填 wake_phrase 再保存。',
          style: style,
        ),
        Text(
          '403 voice_manual_off：手动闸关了。到上面配置区打开 manual_enabled 再保存。',
          style: style,
        ),
        Text(
          '400 wake_phrase_required：转写里没有唤醒短语。先说唤醒词再说正文；短语会被从正文里剥掉。',
          style: style,
        ),
        Text(
          '403 mod_disabled：voice-input Mod 没启用。到「Mod 管理」打开它，或 POST /api/v1/mods/voice-input/enable。',
          style: style,
        ),
        Text(
          '200 但 ok:false（code=busy）：主链忙，这条转写已被丢弃。等 2 到 5 秒再发，不要立即重试。',
          style: style,
        ),
        Text(
          '连接被拒 / 超时（sidecar 没起，或服务没点火）：先跑 ./scripts/ignite.sh，再退避 1 到 2 秒重试。',
          style: style,
        ),
        Text(
          '403 origin_required：发送方没带同源 Origin。官方 sidecar 会自动带；'
          '出现这条通常是 URL/端口填错（或你在用别的客户端）——核对上面的目标 URL。',
          style: style,
        ),
        Text(
          'backend 配错：写了 mock / sidecar 之外的值时服务端按 mock 处理（没有 ASR）；用上面的「检查配置」确认生效值。',
          style: style,
        ),
        Text(
          '401 unauthorized：服务端设了 token 而发送方没带对。对齐 sidecar 的 --token / VOICE_INPUT_TOKEN 与服务端配置。',
          style: style,
        ),
      ];

  /// BCP-47-lite 合法性（与 Mod 侧 selftest 的 locale_is_wellformed 同口径）。
  bool _localeLooksValid(String locale) {
    final List<String> parts = locale.split(RegExp('[-_]'));
    if (!RegExp(r'^[A-Za-z]{2,3}$').hasMatch(parts.first)) return false;
    for (final String part in parts.skip(1)) {
      if (!RegExp(r'^[A-Za-z0-9]{1,8}$').hasMatch(part)) return false;
    }
    return true;
  }

  /// 自检结果 -> 一段可读摘要（问题 / 提醒逐条列出）。
  String _formatCheck(Map<String, Object?> result) {
    final bool ok = result['ok'] == true;
    final String backend = _value(result['backend']);
    final String locale = _value(result['locale']);
    final Object? profile = result['locale_profile'];
    final Object? route = result['route'];
    final String token = result['token_set'] == true ? '已设置' : '未设置';
    final String network = result['opens_network'] == true ? '是' : '否';
    final String gate = result['wake_gate_open'] == true ? '已开' : '未开';
    final String manual = result['manual_enabled'] == true ? '已开' : '已关';
    final StringBuffer buffer = StringBuffer(ok ? '配置自洽。' : '发现配置问题。');
    buffer.write('生效 backend=$backend，locale=$locale');
    if (profile != null) buffer.write('（归一化档 $profile）');
    buffer.write('，token=$token，总闸=$gate，手动闸=$manual');
    if (route != null) buffer.write('，本地路由 $route');
    buffer.write('，Rust 开网络=$network。');
    for (final Object? problem in _stringList(result['problems'])) {
      final String text = _value(problem);
      buffer.write('\n问题：$text');
    }
    for (final Object? note in _stringList(result['notes'])) {
      final String text = _value(note);
      buffer.write('\n提醒：$text');
    }
    return buffer.toString();
  }

  static String _value(Object? raw) => raw == null ? '未知' : raw.toString();

  static List<Object?> _stringList(Object? raw) =>
      raw is List ? raw : const <Object?>[];
}
