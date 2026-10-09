/// 「模型服务 → 语音合成」组的字段（原「语音合成」分区，规格 §4.1.4）。
///
/// # 四个容易写错的地方（逐条对照规格）
///
/// 1. **「服务端静音」是只读状态**，绝不能做成第二个开关；
/// 2. response_format **不在 view 也不在 patch 里** → UI 上**不存在**这个控件；
/// 3. 测试端点**不合成音频**（只做连通性），所以文案**不能**写成「试听」；
/// 4. **TTS 客户端是核心链路**（2026-09-11 用户裁决）：LLM → TTS → 口型，
///    这里的 base_url 就是**唯一权威来源**。local-tts 两个 Mod（缺省停用）
///    只按 argv **拉起进程**，不写这一段、也不探活改地址。
///    论证见 docs/architecture/tts-is-core.md。
///
/// # 2026-10-09 本轮
///
/// 一级「语音合成」取消，字段搬进「模型服务」页的**语音合成**组
/// （ServiceSection 提供组标题，本 widget 不再画 SectionHeader）。
/// 控件下面的功能介绍全删，**只留一条操作规则**：服务地址旁那句
/// 「扩展 本地tts_MeloTTS 只拉起进程，不会改写这个地址。」——
/// 它不是介绍，是防误解的事实说明。
/// 2026-10-09（0.2.3-rc.1）：出厂出声的那个 Mod 换成了 `local-tts-melo`
/// （CosyVoice3 已封存、未注册），所以这句话跟着改指 `本地tts_MeloTTS`。
///
/// 开发者层只留采样率、声道、服务端静音（不做任何加法）。
library;

import 'package:flutter/material.dart';

import '../../api/env_api.dart';
import '../../api/settings_models.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../settings_controller.dart';
import 'env_key_field.dart';
import 'pane_helpers.dart';

/// 服务地址旁唯一保留的一句（口径：操作规则，不是功能介绍）。
///
/// 2026-10-09（0.2.3-rc.1）：指给出厂那个 Mod —— `本地tts_MeloTTS`
/// （id `local-tts-melo`，缺省启用、`with_app`）。CosyVoice3 已封存。
const String kTtsAddressNotice =
    '扩展 本地tts_MeloTTS 只拉起进程，不会改写这个地址。';

class TtsSection extends StatelessWidget {
  const TtsSection({
    required this.controller,
    required this.view,
    required this.devMode,
    this.serverMuted = false,
    this.envKey,
    this.envFile,
    this.onSaveKey,
    this.onTest,
    this.testResult,
    this.testing = false,
    super.key,
  });

  final SettingsController controller;
  final SettingsView view;
  final bool devMode;

  /// 本段声明的密钥键状态（GET /api/v1/env；null = 没绑定变量名）。
  final EnvKey? envKey;
  final String? envFile;

  /// 保存密钥（PUT /api/v1/env）。
  final Future<void> Function(String key, String value)? onSaveKey;

  /// 服务端静音观测值（WS audio.muted，**只读**）。
  final bool serverMuted;
  final Future<void> Function()? onTest;

  /// 上次自检的结果（**服务端 ok + 文案**，见 FieldTestResult）。成败只认
  /// 服务端的 TestOutcome.ok，不从文案猜。
  final FieldTestResult? testResult;
  final bool testing;

  @override
  Widget build(BuildContext context) {
    final SettingsDraft d = controller.draft;
    final TtsSettingsView tts = view.tts;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextFieldRow(
          label: '服务地址',
          icon: Icons.link,
          value: effString(d.ttsBaseUrl, tts.baseUrl),
          hint: 'http://127.0.0.1:8080/v1',
          // 唯一留下的操作规则（见 kTtsAddressNotice 头注）。
          description: kTtsAddressNotice,
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.ttsBaseUrl = v;
          }),
        ),
        TextFieldRow(
          label: '音色（voice）',
          icon: Icons.record_voice_over_outlined,
          value: effString(d.ttsVoice, tts.voice),
          hint: 'alloy / 已注册的克隆音色 id',
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.ttsVoice = v;
          }),
        ),
        TextFieldRow(
          label: '模型名（可空）',
          icon: Icons.memory,
          value: effNullableString(d.ttsModel, tts.model) ?? '',
          hint: '留空 = 请求体不带 model 字段',
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.ttsModel = v;
          }),
        ),
        // 只读展示（改它后果严重，所以只在开发者层可改采样率/声道）。
        ReadonlyField(
          label: '音频规格',
          icon: Icons.settings_voice,
          text:
              '${tts.sampleRate} Hz · ${tts.channels == 1 ? '单声道' : '${tts.channels} 声道'}',
        ),
        ReadonlyField(
          label: '密钥绑定',
          icon: Icons.link,
          text: envKey == null ? '未绑定密钥（无鉴权）' : '绑定到 ${envKey!.key}',
        ),
        if (envKey != null)
          EnvKeyField(
            sectionLabel: '语音合成',
            status: envKey,
            onSave: onSaveKey,
            debugHint: envFile,
          ),
        if (onTest != null) ...<Widget>[
          const Divider(),
          FieldActionRow(
            label: '连通性自检',
            icon: Icons.wifi_tethering,
            actionLabel: '测试连接',
            busy: testing,
            onPressed: () => onTest!(),
            result: testResult,
          ),
        ],
        // 开发者层：只留这三项（采样率 / 声道 / 服务端静音）。
        if (devMode) ...<Widget>[
          const Divider(),
          const SectionHeader(title: '开发者选项'),
          NumberField(
            label: '采样率（Hz）',
            icon: Icons.graphic_eq,
            value: effInt(d.ttsSampleRate, tts.sampleRate),
            min: 8000,
            max: 192000,
            description: '8000-192000',
            onChanged: (int v) => controller.edit((SettingsDraft draft) {
              draft.ttsSampleRate = v;
            }),
          ),
          NumberField(
            label: '声道数',
            icon: Icons.surround_sound,
            value: effInt(d.ttsChannels, tts.channels),
            min: 1,
            max: 2,
            description: '1-2',
            onChanged: (int v) => controller.edit((SettingsDraft draft) {
              draft.ttsChannels = v;
            }),
          ),
          ReadonlyField(
            label: '服务端静音',
            icon: Icons.campaign_outlined,
            text: serverMuted
                ? '静音中（LIVE2D_AI_MUTE_AUDIO=1）——任何客户端都听不到'
                : '未静音',
          ),
        ],
      ],
    );
  }
}
