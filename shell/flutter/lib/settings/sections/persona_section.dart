/// 「角色卡」分区（规格 §4.1.1 + §13.3-4 的角色卡导入）。
///
/// # 导入是这一分区的主要价值
///
/// 酒馆（SillyTavern）卡有两种形态：**JSON** 与**内嵌在 PNG 里的 JSON**。
/// 支持这两条路，用户就能直接把已有的角色卡拖进来，而不是一条条抄。
///
/// **不内置任何预设角色卡**（用户明确要求）。
library;

import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../../api/settings_models.dart';
import '../../design/tokens.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';
import '../persona_card.dart';
import '../persona_import.dart';
import '../settings_controller.dart';
import 'pane_helpers.dart';

class PersonaSection extends StatelessWidget {
  const PersonaSection({
    required this.controller,
    required this.view,
    required this.devMode,
    this.onImport,
    this.importMessage,
    this.importFailed = false,
    super.key,
  });

  final SettingsController controller;
  final SettingsView view;
  final bool devMode;

  /// 用户选了文件 → 把字节交上来（文件选择在组合根，因为要 `package:web`）。
  final void Function(PersonaImportResult result)? onImport;

  /// 上次导入的结果文案。
  final String? importMessage;
  final bool importFailed;

  @override
  Widget build(BuildContext context) {
    final SettingsDraft d = controller.draft;
    final PersonaSettingsView p = view.persona;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '角色卡',
          description: '人设写在这里。默认系统提示词为空——角色由你定义，不预设。',
        ),
        TextFieldRow(
          label: '名称',
          icon: Icons.person_outline,
          value: effString(d.personaName, p.name),
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.personaName = v;
          }),
        ),
        TextFieldRow(
          label: '描述',
          icon: Icons.badge_outlined,
          value: effString(d.personaDescription, p.description),
          multiline: true,
          maxLines: 4,
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.personaDescription = v;
          }),
        ),
        TextFieldRow(
          label: '性格',
          icon: Icons.psychology_outlined,
          value: effString(d.personaPersonality, p.personality),
          multiline: true,
          maxLines: 3,
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.personaPersonality = v;
          }),
        ),
        TextFieldRow(
          label: '场景',
          icon: Icons.landscape_outlined,
          value: effString(d.personaScenario, p.scenario),
          multiline: true,
          maxLines: 3,
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.personaScenario = v;
          }),
        ),
        TextFieldRow(
          label: '开场白',
          icon: Icons.waving_hand_outlined,
          value: effString(d.personaFirst, p.first),
          multiline: true,
          maxLines: 3,
          description: '第一句话。留空则由模型自由开场',
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.personaFirst = v;
          }),
        ),
        const Divider(),
        const SectionHeader(
          title: '导入角色卡',
          // 反引号去掉：`Text` 不认 Markdown，`.json` 这种记号本身已经够清楚，
          // 留着反引号只会多两个符号（与 `**` 同源的问题）。
          description: '支持酒馆（SillyTavern）角色卡：.json，或**内嵌人设的 PNG**'
              '（tEXt / chara）。纯本地解析，不上传任何东西。',
        ),
        // **必须有这个按钮**。第一版只声明了 `onImport` 却没画按钮，
        // 于是整个导入功能在界面上**点不到**（而且被 dart2js 当死代码裁掉，
        // 连提示文案都没进构建产物——是靠搜产物发现的）。
        Align(
          alignment: Alignment.centerLeft,
          child: OutlinedButton.icon(
            onPressed: onImport == null
                ? null
                : () => onImport!(
                    // 真正的结果由宿主在读完文件后回填；这里只为触发选择框。
                    const PersonaImportResult(error: 'pending'),
                  ),
            icon: const Icon(Icons.upload_file_outlined, size: 16),
            label: const Text('选择角色卡文件（.json / .png）'),
          ),
        ),
        const SizedBox(height: Space.s3),
        if (importMessage != null)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s2),
            child: Text(
              importMessage!,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: importFailed
                    ? appPaletteOf(context).danger
                    : appColorsOf(context).contentMuted,
              ),
            ),
          ),
        if (devMode)
          TextFieldRow(
            label: '系统提示词（system_prompt）',
            icon: Icons.terminal_outlined,
            value: effString(d.personaSystemPrompt, p.systemPrompt),
            multiline: true,
            maxLines: 6,
            description: '与上面四项语义重叠；默认留空。改它容易把角色说崩',
            onChanged: (String v) => controller.edit((SettingsDraft draft) {
              draft.personaSystemPrompt = v;
            }),
          ),
        if (devMode)
          NumberField(
            label: '历史轮数上限',
            icon: Icons.history,
            value: effInt(d.personaMaxHistoryPairs, p.maxHistoryPairs),
            min: 0,
            max: 200,
            description: '0 = 不带历史。调大会显著增加延迟与费用',
            onChanged: (int v) => controller.edit((SettingsDraft draft) {
              draft.personaMaxHistoryPairs = v;
            }),
          ),
      ],
    );
  }
}

/// 把文件字节导入成草稿改动（**由组合根调用**：文件读取需要 `package:web`）。
///
/// 返回给用户看的结果文案。**未映射字段要列出来**——用户有权知道
/// 「导进来的东西少了什么」（例如酒馆的 `mes_example` 本项目没有对应字段）。
({String message, bool failed}) applyPersonaImport({
  required SettingsController controller,
  required Uint8List bytes,
}) {
  final PersonaImportResult result = importPersonaBytes(bytes);
  final PersonaCard? card = result.card;
  if (card == null) {
    return (message: '导入失败：${result.error ?? '未知原因'}', failed: true);
  }

  controller.edit((SettingsDraft draft) {
    // 只覆盖**卡里有内容**的字段：导入一张只填了名字的卡不该把描述清空。
    if (card.name.isNotEmpty) draft.personaName = card.name;
    if (card.description.isNotEmpty) draft.personaDescription = card.description;
    if (card.personality.isNotEmpty) draft.personaPersonality = card.personality;
    if (card.scenario.isNotEmpty) draft.personaScenario = card.scenario;
    if (card.first.isNotEmpty) draft.personaFirst = card.first;
    if (card.systemPrompt.isNotEmpty) {
      draft.personaSystemPrompt = card.systemPrompt;
    }
  });

  final StringBuffer message = StringBuffer(
    '已从 ${result.source == 'png' ? 'PNG' : 'JSON'}（${card.format}）读入 '
    '${card.filledCount} 项',
  );
  if (card.name.isNotEmpty) message.write('：${card.name}');
  if (card.unmapped.isNotEmpty) {
    message.write(
      '。未映射的字段（本项目没有对应项，已保留在卡里但不会写入）：'
      '${card.unmapped.take(6).join('、')}',
    );
    if (card.unmapped.length > 6) message.write(' 等 ${card.unmapped.length} 项');
  }
  message.write('。检查后点「保存」。');
  return (message: message.toString(), failed: false);
}
