/// `director` 的产品面板（产品级加强波次）。
///
/// 职责：把「本轮决策」做成一等面板（emotion / intent / suggested_tts + 最近若干条），
/// 且**零投递语义不变**（面板只展示，不驱动动作、不写配置）。
/// 本文件由 director 轨道独占，其他轨道不要改。
library;

import 'package:flutter/widgets.dart';

import 'mod_panel.dart';

class DirectorPanel extends ModPanel {
  const DirectorPanel();

  @override
  String get modId => 'director';

  @override
  Map<String, String> get stateLabels => const <String, String>{
    'decisions': '决策数',
    'turns_seen': '见过轮数',
    'turns_ended': '结项轮数',
    'silent': '静默轮数',
    'errors': '错误数',
    'log_capacity': '日志容量',
    'emotion_lexicon': '情绪词表',
    'recent_decisions': '最近决策',
    'delivered': '已投递',
    'channel': '投递通道',
  };

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) => null;
}
