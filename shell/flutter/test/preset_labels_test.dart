/// 预设标签表解析回归（2026-09-20 调研产出）。
///
/// 展示名的唯一真源是 `assets/actions/preset_labels.json`；Dart 侧**只解析**、
/// 不硬编码中文。坏表必须回落成「显示稳定 id」，而不是让面板崩 / 显示空按钮。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/preset_labels.dart';

void main() {
  test('解析出中文/英文/通道，display 是「中文（id）」', () {
    final PresetLabelTable t = PresetLabelTable.parse('''
    {"labels":{
      "smile":{"zh":"微笑","en":"Smile","channel":"expression"},
      "nod":{"zh":"点头","en":"Nod","channel":"motion"}
    }}''');
    expect(t.length, 2);
    expect(t.zh('smile'), '微笑');
    expect(t['nod']!.en, 'Nod');
    expect(t['nod']!.channel, 'motion');
    expect(t.display('smile'), '微笑（smile）');
  });

  test('未知 id / 空表：原样显示稳定 id（不谎报中文名）', () {
    expect(PresetLabelTable.empty.display('unhappy'), 'unhappy');
    final PresetLabelTable t = PresetLabelTable.parse(
      '{"labels":{"smile":{"zh":"微笑","en":"Smile"}}}',
    );
    expect(t.display('unhappy'), 'unhappy');
  });

  test('idsForChannel：按 channel 过滤、保持 JSON 顺序、排除 none 哨兵', () {
    final PresetLabelTable t = PresetLabelTable.parse('''
    {"labels":{
      "none":{"zh":"中性","en":"Neutral","channel":"expression"},
      "smile":{"zh":"微笑","en":"Smile","channel":"expression"},
      "nod":{"zh":"点头","en":"Nod","channel":"motion"},
      "shake":{"zh":"摇头","en":"Shake","channel":"motion"},
      "wiggle":{"zh":"扭","en":"Wiggle","channel":"dance"}
    }}''');
    // 顺序 = JSON 里的出现顺序（调试面板按钮顺序因此与真源一致）。
    expect(t.idsForChannel('expression'), <String>['smile']);
    expect(t.idsForChannel('motion'), <String>['nod', 'shake']);
    // 通用按 channel 过滤，不写死两个通道。
    expect(t.idsForChannel('dance'), <String>['wiggle']);
    expect(t.idsForChannel('nope'), isEmpty);
    // 取不到表 → 空列表（调用方必须如实说明，不许静默空按钮组）。
    expect(PresetLabelTable.empty.idsForChannel('expression'), isEmpty);
  });

  test('坏 JSON / 缺 labels / 缺 zh：一律回落空表，不抛', () {
    for (final String bad in <String>[
      'not json',
      '[]',
      '{}',
      '{"labels":[]}',
      '{"labels":{"x":{"en":"X"}}}',
    ]) {
      expect(
        PresetLabelTable.parse(bad).length,
        0,
        reason: '坏表必须回落（输入：$bad）',
      );
    }
  });
}
