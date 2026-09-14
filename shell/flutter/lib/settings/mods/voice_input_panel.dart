/// voice-input 的产品面板（产品级加强波次）。
///
/// 职责：把 backend / locale 的语义说成人话，显示**当前生效值**（读 Mod config，
/// 缺省 mock / zh-CN），给一个「检查配置」自检按钮（走基座的一次性命令通道），
/// 并把 sidecar 的退出码翻成用户能执行的处置。本文件由 voice-input 轨道独占，
/// 其他轨道不要改。
///
/// 面板自己**不做网络**：自检经 ModPanelContext.onCommand 转给宿主，
/// 失败以 ApiException.code 呈现（项目错误契约：错误必须带码）。
library;

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../ui/inline_notice.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';
import 'mod_panel.dart';

/// sidecar 脚本的退出码契约（与 docs/voice-input.md 4.5 /
/// docs/examples/voice-sidecar/README.md 5 同源）。
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

class VoiceInputPanel extends ModPanel {
  const VoiceInputPanel();

  @override
  String get modId => 'voice-input';

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

    return Padding(
      padding: const EdgeInsets.only(top: Space.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
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
          const SectionHeader(
            title: '配置自检',
            description: '服务端只回结论，不回 token 明文。',
          ),
          const SizedBox(height: Space.s2),
          Align(
            alignment: Alignment.centerLeft,
            child: FilledButton.tonal(
              onPressed: _checking ? null : _runCheck,
              child: Text(_checking ? '检查中…' : '检查配置'),
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
          const SizedBox(height: Space.s3),
          const SectionHeader(
            title: '出错怎么办',
            description: '前五条是面板 / 服务端能直接告诉你的；最后五条是 sidecar 脚本的退出码。',
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
      final String code = error.code;
      final String message = error.message;
      setState(() {
        _checking = false;
        _checkError = '自检失败：$code —— $message';
      });
    } catch (error) {
      if (!mounted) return;
      final String text = error.toString();
      setState(() {
        _checking = false;
        _checkError = '自检失败：error —— $text';
      });
    }
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
    final StringBuffer buffer = StringBuffer(ok ? '配置自洽。' : '发现配置问题。');
    buffer.write('生效 backend=$backend，locale=$locale');
    if (profile != null) buffer.write('（归一化档 $profile）');
    buffer.write('，token=$token');
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
