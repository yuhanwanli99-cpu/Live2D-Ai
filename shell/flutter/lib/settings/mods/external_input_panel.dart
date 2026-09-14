/// `external-input` 的产品面板（产品级加强波次）。
///
/// 职责：把 `state_json` 里的**接受 / 拒绝 / 忙 / v2_ignored 计数**摊成用户能读的
/// 一行摘要，并给出「这个 Mod 现在到底在听什么」的一句话。
/// 本文件由 external-input 轨道独占，其他轨道不要改。
library;

import 'package:flutter/widgets.dart';

import 'mod_panel.dart';

class ExternalInputPanel extends ModPanel {
  const ExternalInputPanel();

  @override
  String get modId => 'external-input';

  @override
  Map<String, String> get stateLabels => const <String, String>{
    'accepts': '已接受',
    'rejects': '已拒绝',
    'busy': '忙碌拒绝',
    'v2_ignored': '礼物 v2 兜底',
    'ready': '已就绪',
  };

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) => null;
}
