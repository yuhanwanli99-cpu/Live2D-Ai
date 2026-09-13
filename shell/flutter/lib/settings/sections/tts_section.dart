/// 「语音合成」分区（规格 §4.1.4）。
///
/// # 四个容易写错的地方（逐条对照规格）
///
/// 1. **「服务端静音」是只读状态**，绝不能做成第二个开关——
///    否则「我能听到」与「有没有声音被生产出来」两个轴会被同一个词命名。
/// 2. `response_format` **不在 view 也不在 patch 里** → UI 上**不存在**这个控件。
///    不要「顺手加一个」。
/// 3. 测试端点**不合成音频**（只做连通性），所以文案**不能**写成「试听」。
/// 4. **TTS 不在 Mod 里**（2026-09-11 用户裁决）：它是核心链路
///    （LLM → TTS → 口型），这里的 `base_url` 就是**唯一权威来源**。
///    界面上**不要**再出现「本地 TTS Mod / 启用本地语音」这类说法——
///    那会让人以为还有第二个开关能决定会不会出声。
///    论证见 `docs/architecture/tts-is-core.md`。
library;

import 'package:flutter/material.dart';

import '../../api/env_api.dart';
import '../../api/settings_models.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../settings_controller.dart';
import 'env_key_field.dart';
import 'pane_helpers.dart';

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

  /// 本段声明的密钥键状态（`GET /api/v1/env`；`null` = 没绑定变量名）。
  final EnvKey? envKey;
  final String? envFile;

  /// 保存密钥（`PUT /api/v1/env`）。见 `llm_section.dart` 头注的密钥说明。
  final Future<void> Function(String key, String value)? onSaveKey;

  /// 服务端静音观测值（WS `audio.muted`，**只读**）。
  final bool serverMuted;
  final Future<void> Function()? onTest;
  final String? testResult;
  final bool testing;

  @override
  Widget build(BuildContext context) {
    final SettingsDraft d = controller.draft;
    final TtsSettingsView tts = view.tts;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '语音合成',
          description: 'TTS 上游与音色。合成按**整句**进行，不做句中切分。'
              '这里是语音链路的**唯一权威配置**——TTS 是核心链路，不是 Mod。',
        ),
        TextFieldRow(
          label: '服务地址（base_url）',
          icon: Icons.link,
          value: effString(d.ttsBaseUrl, tts.baseUrl),
          hint: 'http://127.0.0.1:8080/v1',
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.ttsBaseUrl = v;
          }),
        ),
        TextFieldRow(
          label: '音色（voice）',
          icon: Icons.record_voice_over_outlined,
          value: effString(d.ttsVoice, tts.voice),
          hint: 'alloy / 已注册的克隆音色 id',
          description: '本服务没有音色清单端点，所以这里用文本框而不是下拉',
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
        ReadonlyField(
          label: '音频规格',
          icon: Icons.settings_voice,
          text: '${tts.sampleRate} Hz · ${tts.channels == 1 ? '单声道' : '${tts.channels} 声道'}',
          description: '当前值；改它后果严重（改错 = 全是噪声），所以只在开发者模式里可改',
        ),
        ReadonlyField(
          label: '密钥绑定',
          icon: Icons.link,
          // 同 LLM：这里说的是「变量名有没有声明」，值在下面那一行填。
          text: tts.hasApiKey ? '已声明密钥的环境变量名' : '未绑定密钥（本地 TTS 通常不需要）',
        ),
        if (envKey != null)
          EnvKeyField(
            sectionLabel: '语音合成',
            status: envKey,
            onSave: onSaveKey,
            debugHint: envFile,
          ),
        // **只读徽标**：服务端静音不是开关。
        ReadonlyField(
          label: '服务端静音',
          icon: Icons.campaign_outlined,
          text: serverMuted
              ? '静音中（LIVE2D_AI_MUTE_AUDIO=1）——任何客户端都听不到'
              : '未静音',
          description: '这是**观测值**，不是开关：它由服务端启动参数决定',
        ),
        if (devMode) ...<Widget>[
          const Divider(),
          const SectionHeader(
            title: '开发者选项',
            description: 'PCM 规格与密钥绑定。改错会导致全是噪声或没有声音。',
          ),
          TextFieldRow(
            label: '密钥的环境变量名',
            icon: Icons.badge_outlined,
            value: effString(d.ttsApiKeyEnv, ''),
            description: '填**变量名**，不是密钥本身',
            onChanged: (String v) => controller.edit((SettingsDraft draft) {
              draft.ttsApiKeyEnv = v;
            }),
          ),
          ToggleField(
            label: '清除密钥绑定',
            icon: Icons.link_off,
            value: d.clearTtsApiKey,
            onChanged: (bool v) => controller.edit((SettingsDraft draft) {
              draft.clearTtsApiKey = v;
            }),
          ),
          NumberField(
            label: '采样率（Hz）',
            icon: Icons.graphic_eq,
            value: effInt(d.ttsSampleRate, tts.sampleRate),
            min: 8000,
            max: 192000,
            description: '必须与服务端实际输出一致，否则听到的是噪声',
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
            onChanged: (int v) => controller.edit((SettingsDraft draft) {
              draft.ttsChannels = v;
            }),
          ),
        ],
        if (onTest != null) ...<Widget>[
          const Divider(),
          FieldActionRow(
            label: '连通性自检',
            icon: Icons.wifi_tethering,
            actionLabel: '测试连接',
            // 规格明确：**不能**宣传成「试听」。2026-09-13 再收紧：
            // 自检**不再合成**（只探 `GET /models`，毫秒级）——原实现会真合成一次，
            // 既慢（本机 2.4s）又与真实链路抢上游，还因此在链路繁忙时报假失败
            //（用户原话：「自检没通，但可以正常播放声音」）。
            description: '只检查端点**可达与鉴权**，不合成、不播放音频。'
                '要确认「真的能出声」，发一条消息即可听见',
            busy: testing,
            onPressed: () => onTest!(),
            result: testResult,
            resultIsError:
                testResult != null &&
                !testResult!.toLowerCase().contains('ok') &&
                !testResult!.contains('毫秒') &&
                !testResult!.contains('ms'),
          ),
        ],
      ],
    );
  }
}
