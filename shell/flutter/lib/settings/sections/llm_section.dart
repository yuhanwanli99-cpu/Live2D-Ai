/// 「LLM」分区（规格 §4.1.3）。
///
/// # 密钥（rc.2 2026-09-12；P2 收敛 hasApiKey 语义；P6 收掉变量名编辑）
///
/// - **变量名**（`api_key_env`）**界面上不再提供编辑入口**（P6）：改它容易把自己
///   弄成未配置，而收益为零。要改绑定就改 `live2d-ai.toml` 的 `api_key_env`
///   （`curl PATCH /api/v1/settings` 也仍然接受这个字段——服务端那条路没删）。
/// - **变量值**仍有地方填：[`EnvKeyField`] 经 `PUT /api/v1/env` 写进 `.env`，
///   写完热重载、立即生效。以前只能在启动脚本/进程环境里设，界面上改不了——
///   于是「保存了还是 401」且无处可改。
/// - 「密钥绑定」一行显示的是 `GET /api/v1/env` 回的**变量名**（绑了哪个）；
///   值本身仍然只往上走、永不下发。
///
/// 红线没变：值**只往上走**，`GET /api/v1/env` 只回键名与「是否已设置」。
/// P2 起 [LlmSettingsView.hasApiKey]（`GET`/`PATCH /settings` 同口径）收敛为
/// **单一语义**——「配置里声明了键名」**且**「服务端真读得到非空值」。
library;

import 'package:flutter/material.dart';

import '../../api/env_api.dart';
import '../../api/settings_models.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../settings_controller.dart';
import 'env_key_field.dart';
import 'pane_helpers.dart';

/// 表演层契约的**界面原文**（2026-09-22 用户敲定）。
///
/// 抽成顶层常量不是为了复用，而是为了让回归能直接断言这几句——
/// 「主模型不负责表演」与三态（只说 / 只动 / noop）是用户理解分工的锚点，
/// 改含糊了就会有人以为两套大脑在抢 cue。
const String kLlmPerformanceText = '主模型不负责表演；表演层每轮 JSON';

/// 表演层一行说明：三态 + 配置位置（都在 [performance] 段）。
const String kLlmPerformanceDescription =
    '主模型只写剧情正文（人设与记忆注入，不暴露工具）。'
    '本轮说什么、做什么由表演层每轮交回一份 JSON 决定，三种输出都合法：'
    '**只说**（speak 有值、cues 空）、**只动**（speak 空、cues 有值）、'
    '**noop**（speak 与 cues 都是空字段 = 本轮不说也不动）。'
    '配置只在 live2d-ai.toml 的 [performance] 段'
    '（enabled + 独立 base_url/model）；表演层默认关。';

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

  /// 开发模式开关。**LLM 段目前没有 dev-only 字段**：P6 删掉最后一行
  /// 「密钥的环境变量名」后，`if (devMode)` 块只剩一个空标题，所以整块撤掉。
  /// 参数保留是为了与 `shell_settings.dart` 的调用签名一致（不在本次改动范围内）。
  final bool devMode;

  /// 本段声明的密钥键状态（`GET /api/v1/env`；`null` = 没绑定变量名）。
  final EnvKey? envKey;

  /// `.env` 路径（只在提示里出现，用于排障）。
  final String? envFile;

  /// 保存密钥（`PUT /api/v1/env`）；`null` = 不可写（未加载/无键名）。
  final Future<void> Function(String key, String value)? onSaveKey;

  /// 连通性自检（`POST /api/v1/settings/test/llm`）。
  final Future<void> Function()? onTest;

  /// 上次自检的结果（**服务端 `ok` + 文案**，见 `FieldTestResult`）。
  ///
  /// 类型从 `String?` 换成结构体是 F-0012-1 的修法本体（2026-10-01）：成败不再
  /// 由这一行文案推出来——本文件里**没有**任何 `contains('ok'|'ms'|'毫秒')`
  /// 的余地可写。
  final FieldTestResult? testResult;
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
          // 服务端回的是**生效值**（显式 0 = 不限制；省略时 = 服务端默认），
          // 所以这里不会是 null；兜底 0 只为类型安全。
          value: effTriInt(d.llmMaxTokens, llm.maxTokens) ?? 0,
          min: 0,
          max: 32768,
          // 2026-09-13：默认值从 512 提到 4096——推理模型的**思考也占这份预算**，
          // 512 时一个普通提问的思考就能把正文挤成半句，而半句切不出完整句，
          // 界面上一个字都不会出现（用户报「无模型返回」的根因之一）。
          // 文案必须说清「这是上限、不是配额」，否则用户不敢调大。
          description: '0 = 不限制；省略 = 服务端默认。'
              '**推理模型的思考也占这份预算**：调太小会把正文挤成半句甚至为空，'
              '表现为「模型没有返回」——不要调小',
          onChanged: (int v) => controller.edit((SettingsDraft draft) {
            draft.llmMaxTokens = Tri.set(v);
          }),
        ),
        ToggleField(
          label: '展示思考（reasoning）',
          icon: Icons.psychology_outlined,
          value: effTriBool(d.llmShowReasoning, llm.showReasoning) ?? false,
          description: '开启后，推理模型的思考会显示在气泡的「思考」折叠区（不会念出来）。'
              '关掉只是不显示——思考照样生成、照样不占正文，也不是为了提速。',
          onChanged: (bool v) => controller.edit((SettingsDraft draft) {
            draft.llmShowReasoning = Tri.set(v);
          }),
        ),
        ReadonlyField(
          label: '表演层（默认关）',
          icon: Icons.theater_comedy_outlined,
          // 2026-09-22 用户敲定的分工：主模型只管剧情；说辞整理与动作归表演层。
          // 这一行是**界面上的契约说明**——不让用户以为主模型还会“演”。
          text: kLlmPerformanceText,
          description: kLlmPerformanceDescription,
        ),
        ReadonlyField(
          label: '密钥绑定',
          icon: Icons.link,
          // P6：绑定名来自 `GET /api/v1/env`（`EnvKey.key`）——界面只显示
          // **绑到哪个变量名**，不显示值、也不再提供修改变量名的入口。
          // 值本身只往上走（在下面那行填）。
          text: envKey == null ? '未绑定密钥（无鉴权）' : '绑定到 ${envKey!.key}',
          description: '密钥本体永不下发；值请在下面填写（写进 .env）；'
              '改绑定 = 改 live2d-ai.toml 的 api_key_env',
        ),
        if (envKey != null)
          EnvKeyField(
            sectionLabel: '对话模型',
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
            description: '只做连通性，不会真的生成回复；结果内联显示',
            busy: testing,
            onPressed: () => onTest!(),
            result: testResult,
          ),
        ],
      ],
    );
  }
}
