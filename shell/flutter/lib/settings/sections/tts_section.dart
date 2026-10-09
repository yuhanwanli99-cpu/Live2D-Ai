/// 「模型服务 → 语音合成」组的字段（原「语音合成」分区，规格 §4.1.4）。
///
/// # 四个容易写错的地方（逐条对照规格）
///
/// 1. **「服务端静音」是只读状态**，绝不能做成第二个开关；
/// 2. response_format **不在 view 也不在 patch 里** → UI 上**不存在**这个控件；
/// 3. 测试端点**不合成音频**（只做连通性），所以文案**不能**写成「试听」；
/// 4. **TTS 客户端是核心链路**（2026-09-11 用户裁决）：LLM → TTS → 口型，
///    这里的 base_url 就是**唯一权威来源**。local-tts 两个 Mod 只按 argv
///    **拉起进程**，不写这一段、也不探活改地址。论证见 docs/architecture/tts-is-core.md。
///
/// # 2026-10-09：云端 / 本地二选一（缺省本地）
///
/// - **本地**（缺省）：地址固定 `127.0.0.1:8091`，音色 `ZH`，`pcm` /
///   44100 / 单声道、**无密钥**。普通层**不给改端口**（这里连输入框都不画，
///   只有只读展示）；保存时由**设置保存**把这一组值写进 `[tts]`，
///   同时只启用 `local-tts-melo` 这一个本地引擎（已有子进程不再起一个）。
/// - **云端**：地址 / 音色 / 模型名 / 密钥都由用户填；**不预填厂商、不带 Key**。
///   自检仍只打 `/models`。
/// - 切换走同一次「保存并应用」；失败就让这一轮没生效（错误码照旧），
///   **不自动改回另一个**。
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

/// 本地模式下服务地址旁的一句（口径：操作规则，不是功能介绍）。
const String kTtsLocalNotice =
    '本地：地址与规格固定，由扩展 本地tts_MeloTTS 拉起进程；已有子进程不会再起一个。';

/// 云端模式下的说明（唯一一句，紧跟在地址输入框下）。
const String kTtsCloudNotice =
    '云端：地址、音色、模型名与密钥都由你填；自检仍只打 /models，不合成。';

/// 本地模式的只读规格串（与 Rust 侧 `apply_local_tts_defaults` 逐字对应）。
const String kTtsLocalSpecText = 'pcm · 44100 Hz · 单声道';

/// 云端模式、还没绑定密钥时的那一行（保存后才出现输入框）。
const String kTtsCloudKeyPending = '保存后这里出现密钥输入（绑定 LIVE2D_AI_TTS_API_KEY）';

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

  /// 当前该显示的模式：草稿优先，再回落服务端；两边都没有 = 本地（缺省）。
  static String modeOf(SettingsDraft draft, TtsSettingsView tts) {
    final String? drafted = draft.ttsMode;
    if (drafted != null && drafted.isNotEmpty) return drafted;
    return tts.mode.isEmpty ? kTtsModeLocal : tts.mode;
  }

  /// 切换来源：**先把对面那一套值从草稿里撤掉**，免得保存时把本地端口当
  /// 「用户填的云端地址」发上去（反之亦然）。
  void _switchMode(String next) {
    controller.edit((SettingsDraft draft) {
      draft.ttsMode = next;
      if (next == kTtsModeLocal) {
        draft.ttsBaseUrl = null;
        draft.ttsVoice = null;
        draft.ttsModel = null;
        draft.ttsSampleRate = null;
        draft.ttsChannels = null;
      } else {
        // 不预填厂商：三个框都从空开始，用户自己填。
        draft.ttsBaseUrl = '';
        draft.ttsVoice = '';
        draft.ttsModel = '';
        // 云端要有一个密钥**变量名**才填得进 Key（值仍由用户自己写）。
        // 本机已经绑过别的变量名时不覆盖——那是用户自己的选择。
        if (envKey == null) draft.ttsApiKeyEnv = Tri.set(kTtsCloudKeyEnv);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final SettingsDraft d = controller.draft;
    final TtsSettingsView tts = view.tts;
    final String mode = modeOf(d, tts);
    final bool isLocal = mode != kTtsModeCloud;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SegmentedField<String>(
          label: '语音来源',
          icon: Icons.cloud_outlined,
          value: mode,
          options: const <FieldOption<String>>[
            FieldOption<String>(value: kTtsModeLocal, label: '本地'),
            FieldOption<String>(value: kTtsModeCloud, label: '云端'),
          ],
          onChanged: _switchMode,
        ),
        if (isLocal) ...<Widget>[
          // 本地：端口固定，普通层不给改 ⇒ 只读展示，连输入框都不画。
          const ReadonlyField(
            label: '服务地址',
            icon: Icons.link,
            text: kTtsLocalBaseUrl,
          ),
          const ReadonlyField(
            label: '音色（voice）',
            icon: Icons.record_voice_over_outlined,
            text: 'ZH',
          ),
          const ReadonlyField(
            label: '音频规格',
            icon: Icons.settings_voice,
            text: kTtsLocalSpecText,
            description: kTtsLocalNotice,
          ),
        ] else ...<Widget>[
          TextFieldRow(
            label: '服务地址',
            icon: Icons.link,
            value: effString(d.ttsBaseUrl, tts.baseUrl),
            hint: 'https://你的端点/v1',
            description: kTtsCloudNotice,
            onChanged: (String v) => controller.edit((SettingsDraft draft) {
              draft.ttsBaseUrl = v;
            }),
          ),
          TextFieldRow(
            label: '音色（voice）',
            icon: Icons.record_voice_over_outlined,
            value: effString(d.ttsVoice, tts.voice),
            hint: '服务端支持的音色',
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
        ],
        ReadonlyField(
          label: '密钥绑定',
          icon: Icons.link,
          text: isLocal
              ? '本地固定：不带密钥'
              : (envKey == null ? kTtsCloudKeyPending : '绑定到 ${envKey!.key}'),
        ),
        if (!isLocal && envKey != null)
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
