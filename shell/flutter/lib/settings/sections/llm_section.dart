/// 「LLM」分区（规格 §4.1.3）。
///
/// # 密钥红线的严格落地
///
/// 视图侧**永远不返回变量名**，只回 `has_api_key`；密文本体不在
/// `AppSettings` 里，只在进程环境变量里。所以这里**根本没有「输入 API key
/// 明文」这条路**——只能填「存放密钥的变量名」。
///
/// `api_key_env` 属于 **dev** 层（普通用户改它容易把自己弄成未配置），
/// 但 `has_api_key` 徽标在普通层——用户需要知道「有没有配好」。
library;

import 'package:flutter/material.dart';

import '../../api/settings_models.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../settings_controller.dart';
import 'pane_helpers.dart';

class LlmSection extends StatelessWidget {
  const LlmSection({
    required this.controller,
    required this.view,
    required this.devMode,
    this.onTest,
    this.testResult,
    this.testing = false,
    super.key,
  });

  final SettingsController controller;
  final SettingsView view;
  final bool devMode;

  /// 连通性自检（`POST /api/v1/settings/test/llm`）。
  final Future<void> Function()? onTest;

  /// 上次自检的结果行（`ok` / `latency_ms` / `error`）。
  final String? testResult;
  final bool testing;

  @override
  Widget build(BuildContext context) {
    final SettingsDraft d = controller.draft;
    final LlmSettingsView llm = view.llm;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '对话模型',
          description: 'OpenAI 兼容端点。本地 Ollama / vLLM 与云端服务同一套配置。',
        ),
        TextFieldRow(
          label: '服务地址（base_url）',
          icon: Icons.link,
          value: effString(d.llmBaseUrl, llm.baseUrl),
          hint: 'http://127.0.0.1:11434/v1',
          description: '尾部 / 可省略；请求路径为 <base_url>/chat/completions',
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.llmBaseUrl = v;
          }),
        ),
        TextFieldRow(
          label: '模型名',
          icon: Icons.memory,
          value: effString(d.llmModel, llm.model),
          hint: 'qwen2.5:7b',
          description: '原样进入请求体的 model 字段',
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.llmModel = v;
          }),
        ),
        NumberField(
          label: '输出 token 上限',
          icon: Icons.straighten,
          // 服务端回的是**生效值**（0=不限制，省略=512），所以这里不会是 null；
          // 兜底 0 只为类型安全。
          value: effTriInt(d.llmMaxTokens, llm.maxTokens) ?? 0,
          min: 0,
          max: 32768,
          description: '0 = 不限制；省略 = 服务端默认 512。'
              '调太小会把句子截断在半句——那本身就是「断句」',
          onChanged: (int v) => controller.edit((SettingsDraft draft) {
            draft.llmMaxTokens = Tri.set(v);
          }),
        ),
        ReadonlyField(
          label: '密钥状态',
          icon: Icons.key_outlined,
          text: llm.hasApiKey ? '已配置（服务端能从环境变量读到）' : '未配置',
          description: '密钥本身永不下发，界面只能看到「配没配好」',
        ),
        if (devMode) ...<Widget>[
          const Divider(),
          const SectionHeader(
            title: '开发者选项',
            description: '这些字段普通使用不需要碰。',
          ),
          TextFieldRow(
            label: '密钥的环境变量名',
            icon: Icons.badge_outlined,
            value: effString(d.llmApiKeyEnv, ''),
            hint: 'LIVE2D_AI_LLM_API_KEY',
            description: '填**变量名**，不是密钥本身。留空并保存 = 不绑定密钥',
            onChanged: (String v) => controller.edit((SettingsDraft draft) {
              draft.llmApiKeyEnv = v;
            }),
          ),
          ToggleField(
            label: '清除密钥绑定',
            icon: Icons.link_off,
            value: d.clearLlmApiKey,
            description: '保存后服务端不再从环境变量读密钥（下次启动生效）',
            onChanged: (bool v) => controller.edit((SettingsDraft draft) {
              draft.clearLlmApiKey = v;
            }),
          ),
        ],
        if (onTest != null) ...<Widget>[
          const Divider(),
          FieldActionRow(
            label: '连通性自检',
            icon: Icons.wifi_tethering,
            actionLabel: '测试连接',
            description: '只做连通性，不会真的生成回复；结果内联显示',
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
