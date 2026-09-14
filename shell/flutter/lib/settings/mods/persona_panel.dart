/// `persona` 的产品面板（产品级加强波次）。
///
/// 职责：让「导入一张角色卡 → 启用 → 对话人设变化 → 停用还原」尽量在 UI 内完成。
/// 本文件由 persona 轨道独占，其他轨道不要改。
library;

import 'package:flutter/widgets.dart';

import 'mod_panel.dart';

class PersonaPanel extends ModPanel {
  const PersonaPanel();

  @override
  String get modId => 'persona';

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) => null;
}
