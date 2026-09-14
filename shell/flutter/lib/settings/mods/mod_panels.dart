/// 已注册的 Mod 产品面板注册表（见 [ModPanel] 头注）。
///
/// **产品级加强波次的文件边界**：每个面板一个文件，五条轨道各改各的；
/// 这个注册表是**共享只读**的——加面板要改这里，但单面板的实现不进这里。
library;

import 'director_panel.dart';
import 'external_input_panel.dart';
import 'memory_panel.dart';
import 'mod_panel.dart';
import 'persona_panel.dart';
import 'voice_input_panel.dart';

/// 全部产品面板（顺序即「Mod 管理」里的渲染顺序无关——面板跟着各自的 Mod 卡片）。
const List<ModPanel> kModPanels = <ModPanel>[
  ExternalInputPanel(),
  PersonaPanel(),
  VoiceInputPanel(),
  MemoryPanel(),
  DirectorPanel(),
];

/// 按 id 找面板；没有专用面板的 Mod 返回 null。
ModPanel? modPanelFor(String id) {
  for (final ModPanel p in kModPanels) {
    if (p.modId == id) return p;
  }
  return null;
}

/// 某个 Mod 的运行态字段中文标签（没有面板 → 空表）。
Map<String, String> modStateLabelsFor(String id) =>
    modPanelFor(id)?.stateLabels ?? const <String, String>{};
