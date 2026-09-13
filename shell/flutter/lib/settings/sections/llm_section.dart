/// 「LLM」分区（规格 §4.1.3）。
///
/// # 密钥（rc.2 2026-09-12 改了这里的说法）
///
/// - **变量名**（`api_key_env`）属于 **dev** 层，改它容易把自己弄成未配置；
/// - **变量值**现在有地方填了：[`EnvKeyField`] 经 `PUT /api/v1/env` 写进 `.env`，
///   写完热重载、立即生效。以前只能在启动脚本/进程环境里设，界面上改不了——
///   于是「保存了还是 401」且无处可改。
///
/// 红线没变：值**只往上走**，`GET /api/v1/env` 只回键名与「是否已设置」。
/// 所以 [LlmSettingsView.hasApiKey]（来自 `GET /settings`）表达的仍然是
/// **「配置里声明了键名」**，不是「值已经拿到」——两者文案必须分开说，
/// 否则用户看到「已配置」却还在 401。
library;

import 'package:flutter/material.dart';

import '../../api/env_api.dart';
import '../../api/settings_models.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../settings_controller.dart';
import 'env_key_field.dart';
import 'pane_helpers.dart';

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
  final bool devMode;

  /// 本段声明的密钥键状态（`GET /api/v1/env`；`null` = 没绑定变量名）。
  final EnvKey? envKey;

  /// `.env` 路径（只在提示里出现，用于排障）。
  final String? envFile;

  /// 保存密钥（`PUT /api/v1/env`）；`null` = 不可写（未加载/无键名）。
  final Future<void> Function(String key, String value)? onSaveKey;

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
          label: '密钥绑定',
          icon: Icons.link,
          // 这里说的是**变量名有没有声明**（`GET /settings` 只知道这个）。
          // 值有没有拿到看下面那一行的「已设置/未设置」——两句分开说，
          // 否则「已配置」会与 401 同时出现。
          text: llm.hasApiKey ? '已声明密钥的环境变量名' : '未绑定密钥（无鉴权）',
          description: '密钥本体永不下发；值请在下面填写（写进 .env）',
        ),
        if (envKey != null)
          EnvKeyField(
            sectionLabel: '对话模型',
            status: envKey,
            onSave: onSaveKey,
            debugHint: envFile,
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
