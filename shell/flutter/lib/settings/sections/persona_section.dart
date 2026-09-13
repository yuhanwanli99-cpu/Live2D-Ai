/// 「人设」分区（M5.1，2026-09-13）：主链只剩**系统提示词**。
///
/// # 为什么这里变得这么小
///
/// 酒馆（SillyTavern）角色卡（name / description / personality / scenario /
/// first）与它的导入 UI 已从**主链迁出**——那属于扩展能力，按项目裁定走 Mod
/// 边界（标准 Mod `live2d-ai-mod-persona`）。主链只保留服务端 `[persona]`
/// 真正会用的两项：`system_prompt` 与 `max_history_pairs`。
///
/// 历史轮数是**会话基建**（不是人设），所以放开发者区；主可见项只有
/// 系统提示词一项。**不内置任何预设人设**（用户明确要求）。
library;

import 'package:flutter/material.dart';

import '../../api/settings_models.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../settings_controller.dart';
import 'pane_helpers.dart';

class PersonaSection extends StatelessWidget {
  const PersonaSection({
    required this.controller,
    required this.view,
    required this.devMode,
    super.key,
  });

  final SettingsController controller;
  final SettingsView view;
  final bool devMode;

  @override
  Widget build(BuildContext context) {
    final SettingsDraft d = controller.draft;
    final PersonaSettingsView p = view.persona;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '系统提示词',
          description: '主链人设只有系统提示词与历史轮数。'
              '酒馆角色卡现在是 Mod（去「Mod」分区配置），不再内嵌主链。',
        ),
        TextFieldRow(
          label: '系统提示词（system_prompt）',
          icon: Icons.terminal_outlined,
          value: effString(d.personaSystemPrompt, p.systemPrompt),
          multiline: true,
          maxLines: 6,
          description: '默认留空。改它容易把角色说崩',
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
