/// 「模型对话」分区（原「人设」，2026-10-09 只改显示名）。
///
/// # 这里有什么
///
/// 主链服务端 [persona] 真正会用的两项：**系统提示词**与**记住几轮**。
/// 角色卡（酒馆卡）导入是 **persona 扩展**的能力，入口在「扩展」分区的
/// persona 卡片（settings/mods/persona_panel.dart）——那一份命令与入参
/// 就是契约真源。
///
/// # 2026-10-09 本轮
///
/// 导航 label 与页标题都用「模型对话」这四个字；枚举值仍是
/// SettingsSection.persona（跳转 / byIndex(0) 不变）。控件下面那些
/// 功能介绍全删，只留控件名、placeholder 与数字项的 min-max 范围。
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
        const SectionHeader(title: '模型对话'),
        TextFieldRow(
          label: '系统提示词',
          icon: Icons.terminal_outlined,
          value: effString(d.personaSystemPrompt, p.systemPrompt),
          multiline: true,
          maxLines: 6,
          onChanged: (String v) => controller.edit((SettingsDraft draft) {
            draft.personaSystemPrompt = v;
          }),
        ),
        NumberField(
          label: '记住几轮对话',
          icon: Icons.history,
          value: effInt(d.personaMaxHistoryPairs, p.maxHistoryPairs),
          min: 0,
          max: 200,
          // 数字项只留一行范围（数从控件上抄，不改夹持）。
          description: '0-200',
          onChanged: (int v) => controller.edit((SettingsDraft draft) {
            draft.personaMaxHistoryPairs = v;
          }),
        ),
      ],
    );
  }
}
