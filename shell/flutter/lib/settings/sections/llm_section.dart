/// 「模型服务 → 语言模型」组的字段（原「LLM / 对话」分区，规格 §4.1.3）。
///
/// # 2026-10-18（0.3.7 T1）：云端 / 本地二选一
///
/// - **云端**（缺省）：地址 / 模型名 / 密钥由用户填。
/// - **本地**：端点在**用户自己起的**本机 / 局域网 OpenAI-compatible 服务上
///   （Ollama / llama.cpp / vLLM 等）。**transport 不变**（仍是一条
///   `/chat/completions`）——本项目不下载模型、不装推理运行时、不预填厂商或 Key；
///   这里只给 Ollama 地址做示例文案。
/// - 切模式**只是草稿**（保存并重载才生效）；服务端把 `mode` 当普通可选值：
///   切模式不动地址 / 模型 / token / 思考，也不清密钥绑定。
/// - 2026-10-19（0.3.9 rc.2）：厂商下拉与快照脏标记已删除——「云端 / 本地」
///   二选一就是唯一切换手段，二级字段直接挂在这一档下面。
///   与 `tts_section.dart` 的 `_switchMode` 同一条纪律。
/// - 连通性自检与 TTS 同一条纪律：`GET {base_url}/models`，404 算可达 + note，
///   **不真跑一次对话**；本地没配置 / 连不上时服务端回 `llm_local_unreachable`
///   （明确状态，不显示 ready），且**不会自动切回云端**。
///
/// # 密钥（rc.2 / P2 / P6 的既有边界，未变）
///
/// - **变量名**（api_key_env）界面上没有编辑入口（P6）——所以切模式**不清**它，
///   否则切回云端就再也绑不回来；
/// - **变量值**仍在这里填：EnvKeyField 经 PUT /api/v1/env 写进 .env，写完热重载；
/// - 仓库不提供、不提交任何 Key；变量名由 EnvKeyField 自己显示（GET /api/v1/env）。
library;

import 'package:flutter/material.dart';

import '../../api/env_api.dart';
import '../../api/settings_models.dart';
import '../../ui/field_row.dart';
import '../settings_controller.dart';
import 'env_key_field.dart';
import 'pane_helpers.dart';

/// 云端模式的说明（一句，挂在服务地址下；与 tts_section 的写法同款）。
const String kLlmCloudNotice =
    '云端：地址、模型名与密钥变量名都由配置决定；自检只打 /models，不跑对话。';

/// 本地模式的说明：**示例文案**（不硬编码下拉、不预填）。
const String kLlmLocalNotice =
    '本地：你自己起的 OpenAI-compatible 端点（Ollama / llama.cpp / vLLM 等）；'
    '本项目不下载模型、不预填厂商与 Key。示例 $kLlmLocalExampleBaseUrl；自检只打 /models。';

/// 云端模式的密钥行（2026-10-09 原句，逐字保留）。
const String kLlmCloudKeyHint = '到 DeepSeek 申请自己的 Key，填到下面。';

/// 本地模式的密钥行：本地端点通常不鉴权，但**不清理**已绑定的变量名。
const String kLlmLocalKeyHint = '本地通常不需要密钥；要鉴权时仍填在下面（变量名不变）。';

/// 当前该显示的模式：草稿优先，再回落服务端；两边都没有 = 云端（缺省）。
String llmModeOf(SettingsDraft draft, LlmSettingsView llm) {
  final String? drafted = draft.llmMode;
  if (drafted != null && drafted.isNotEmpty) return drafted;
  return llm.mode.isEmpty ? kLlmModeCloud : llm.mode;
}

class LlmSection extends StatelessWidget {
  const LlmSection({
    required this.controller,
    required this.view,
    required this.devMode,
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

  /// 开发模式开关。**语言模型段没有 dev-only 字段**（P6 删掉密钥变量名编辑、
  /// 2026-10-08 撤掉表演层开关之后就没有了）。参数保留是为了与调用签名一致。
  final bool devMode;

  /// 本段声明的密钥键状态（GET /api/v1/env；null = 没绑定变量名）。
  final EnvKey? envKey;

  /// .env 路径（只在提示里出现，用于排障）。
  final String? envFile;

  /// 保存密钥（PUT /api/v1/env）；null = 不可写（未加载/无键名）。
  final Future<EnvWriteResult> Function(String key, String value)? onSaveKey;

  /// 连通性自检（POST /api/v1/settings/test/llm）。
  final Future<void> Function()? onTest;

  /// 上次自检的结果（**服务端 ok + 文案**，见 FieldTestResult）。
  final FieldTestResult? testResult;
  final bool testing;

  /// 切换端点形态（0.3.7 T1）：**只记草稿**，保存并重载才生效。
  ///
  /// **不预填、也不清空**地址 / 模型 / token / 思考：LLM 的本地形态没有
  /// 冻结地址（不像 TTS 的 melo 固定端口），这些字段仍归用户自己填；
  /// 清掉它们反而会让「切回云端」丢配置。
  void switchMode(String next) {
    controller.edit((SettingsDraft draft) {
      draft.llmMode = next;
    });
  }

  @override
  Widget build(BuildContext context) {
    final SettingsDraft d = controller.draft;
    final LlmSettingsView llm = view.llm;
    final String mode = llmModeOf(d, llm);
    final bool isLocal = mode == kLlmModeLocal;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SegmentedField<String>(
          label: '模型来源',
          icon: Icons.cloud_outlined,
          value: mode,
          options: const <FieldOption<String>>[
            FieldOption<String>(value: kLlmModeCloud, label: '云端'),
            FieldOption<String>(value: kLlmModeLocal, label: '本地'),
          ],
          onChanged: switchMode,
        ),
        TextFieldRow(
          label: '服务地址',
          icon: Icons.link,
          value: effString(d.llmBaseUrl, llm.baseUrl),
          hint: isLocal ? kLlmLocalExampleBaseUrl : 'https://api.deepseek.com/v1',
          description: isLocal ? kLlmLocalNotice : kLlmCloudNotice,
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.llmBaseUrl = v;
          }),
        ),
        TextFieldRow(
          label: '模型名',
          icon: Icons.memory,
          value: effString(d.llmModel, llm.model),
          hint: isLocal ? 'qwen2.5:7b' : 'deepseek-flash',
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.llmModel = v;
          }),
        ),
        NumberField(
          label: '输出 token 上限',
          icon: Icons.straighten,
          value: effTriInt(d.llmMaxTokens, llm.maxTokens) ?? 0,
          min: 0,
          max: 32768,
          // 数字项只留一行范围（数从控件上抄，不改夹持）。
          description: '0-32768',
          onChanged: (int v) => controller.edit((SettingsDraft draft) {
            draft.llmMaxTokens = Tri.set(v);
          }),
        ),
        ToggleField(
          label: '思考',
          icon: Icons.psychology_outlined,
          value: effTriBool(d.llmShowReasoning, llm.showReasoning) ?? false,
          onChanged: (bool v) => controller.edit((SettingsDraft draft) {
            draft.llmShowReasoning = Tri.set(v);
          }),
        ),
        ReadonlyField(
          label: '密钥',
          icon: Icons.key_outlined,
          // 云端保留 2026-10-09 的原句；本地只换这一句说明——**变量名与绑定都不动**
          //（界面上没有变量名编辑入口，清了就再也绑不回去）。
          text: isLocal ? kLlmLocalKeyHint : kLlmCloudKeyHint,
        ),
        if (envKey != null)
          EnvKeyField(
            sectionLabel: '语言模型',
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
      ],
    );
  }
}
