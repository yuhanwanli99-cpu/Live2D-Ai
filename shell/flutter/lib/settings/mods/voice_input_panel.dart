/// `voice-input` 的产品面板（产品级加强波次）。
///
/// 职责：把 `backend` / `locale` 的语义说成人话，并把 sidecar 的失败处置指向文档。
/// 本文件由 voice-input 轨道独占，其他轨道不要改。
library;

import 'package:flutter/widgets.dart';

import 'mod_panel.dart';

class VoiceInputPanel extends ModPanel {
  const VoiceInputPanel();

  @override
  String get modId => 'voice-input';

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) => null;
}
