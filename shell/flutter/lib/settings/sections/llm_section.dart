/// 「模型服务 → 语言模型」组的字段（原「LLM / 对话」分区，规格 §4.1.3）。
///
/// # 2026-10-09 本轮
///
/// 一级「对话」取消，字段**语义一字不改**地搬进「模型服务」页的
/// **语言模型**组（ServiceSection 提供组标题，本 widget 不再画 SectionHeader）。
/// 控件下面的功能介绍全删；数字项只留一行 min-max。
///
/// # 密钥（rc.2 / P2 / P6 的既有边界，未变）
///
/// - **变量名**（api_key_env）界面上没有编辑入口（P6）；
/// - **变量值**仍在这里填：EnvKeyField 经 PUT /api/v1/env 写进 .env，写完热重载；
/// - 密钥那一行**只留一句指引**（2026-10-09，0.2.3-rc.1）：
///   「到 DeepSeek 申请自己的 Key，填到下面。」——仓库不提供、不提交任何 Key；
///   变量名由 EnvKeyField 自己显示（GET /api/v1/env），值只往上走。
library;

import 'package:flutter/material.dart';

import '../../api/env_api.dart';
import '../../api/settings_models.dart';
import '../../ui/field_row.dart';
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

  /// 开发模式开关。**语言模型段没有 dev-only 字段**（P6 删掉密钥变量名编辑、
  /// 2026-10-08 撤掉表演层开关之后就没有了）。参数保留是为了与调用签名一致。
  final bool devMode;

  /// 本段声明的密钥键状态（GET /api/v1/env；null = 没绑定变量名）。
  final EnvKey? envKey;

  /// .env 路径（只在提示里出现，用于排障）。
  final String? envFile;

  /// 保存密钥（PUT /api/v1/env）；null = 不可写（未加载/无键名）。
  final Future<void> Function(String key, String value)? onSaveKey;

  /// 连通性自检（POST /api/v1/settings/test/llm）。
  final Future<void> Function()? onTest;

  /// 上次自检的结果（**服务端 ok + 文案**，见 FieldTestResult）。
  final FieldTestResult? testResult;
  final bool testing;

  @override
  Widget build(BuildContext context) {
    final SettingsDraft d = controller.draft;
    final LlmSettingsView llm = view.llm;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextFieldRow(
          label: '服务地址',
          icon: Icons.link,
          value: effString(d.llmBaseUrl, llm.baseUrl),
          hint: 'https://api.deepseek.com/v1',
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.llmBaseUrl = v;
          }),
        ),
        TextFieldRow(
          label: '模型名',
          icon: Icons.memory,
          value: effString(d.llmModel, llm.model),
          hint: 'deepseek-flash',
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
          // 2026-10-09（0.2.3-rc.1）：这里只留**这一句**。出厂不带任何 Key，
          // 用户自己去申请；键名与「已设置/未设置」由下面的 EnvKeyField 如实显示。
          text: '到 DeepSeek 申请自己的 Key，填到下面。',
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
