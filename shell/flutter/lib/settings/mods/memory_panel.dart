/// `memory` 的产品面板（产品级加强波次）。
///
/// 职责：可见条数 / hits / 清空；注入可关；与 persona 的策略在 UI 里说清。
/// 本文件由 memory 轨道独占，其他轨道不要改。
library;

import 'package:flutter/widgets.dart';

import 'mod_panel.dart';

class MemoryPanel extends ModPanel {
  const MemoryPanel();

  @override
  String get modId => 'memory';

  @override
  Map<String, String> get stateLabels => const <String, String>{
    'writes': '写入条数',
    'hits': '命中次数',
    'injects': '注入轮数',
    'errors': '错误数',
    'evicted': '已淘汰',
    'last_hits': '上轮命中',
    'top_k': '每轮注入条数',
    'max_records': '条数上限',
    'enabled_injection': '注入开关',
    'store_path': '记忆库路径',
    'turns_seen': '经历轮数',
  };

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) => null;
}
