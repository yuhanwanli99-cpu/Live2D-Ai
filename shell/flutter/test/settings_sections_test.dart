import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/settings_sections.dart';

void main() {
  group('分区声明：单点真相（规格 §4.0 / 钉子 12）', () {
    test('恰好 8 个分区', () {
      expect(SettingsSection.values, hasLength(8));
    });

    test('**每项都有非空的 label 与 description**（用户不该先学会黑话）', () {
      for (final SettingsSection s in SettingsSection.values) {
        expect(s.label.trim(), isNotEmpty, reason: '${s.name} 缺 label');
        expect(
          s.description.trim(),
          isNotEmpty,
          reason: '${s.name} 缺 description —— AIRI 的教训是每项都要一句人话说明',
        );
      }
    });

    test('label 与 description 都**无重号**（防复制粘贴漏改）', () {
      final List<String> labels =
          SettingsSection.values.map((SettingsSection s) => s.label).toList();
      expect(labels.toSet(), hasLength(labels.length), reason: 'label 有重复');
      final List<String> descriptions = SettingsSection.values
          .map((SettingsSection s) => s.description)
          .toList();
      expect(
        descriptions.toSet(),
        hasLength(descriptions.length),
        reason: 'description 有重复',
      );
    });

    test('description 不是 label 的复读（那等于没写）', () {
      for (final SettingsSection s in SettingsSection.values) {
        expect(
          s.description.contains(s.label),
          isFalse,
          reason: '${s.name} 的说明只是把标题又说了一遍',
        );
      }
    });

    test('图标两两不同（导航里靠形状区分 8 项）', () {
      final Set<IconData> icons =
          SettingsSection.values.map((SettingsSection s) => s.icon).toSet();
      expect(icons, hasLength(8));
    });

    test('顺序 = 声明顺序：角色 → 能力 → 行为 → 扩展/诊断/开发', () {
      expect(SettingsSection.values.map((SettingsSection s) => s.name), <String>[
        'persona',
        'models',
        'llm',
        'tts',
        'actions',
        'mods',
        'diagnostics',
        'developer',
      ]);
    });

    test('index 与声明顺序一致（NavigationRail 的 selectedIndex 直接吃它）', () {
      for (int i = 0; i < SettingsSection.values.length; i++) {
        expect(SettingsSection.values[i].index, i);
      }
    });
  });

  group('byIndex：越界回落第一个，**不抛**', () {
    test('合法索引', () {
      expect(SettingsSection.byIndex(0), SettingsSection.persona);
      expect(SettingsSection.byIndex(7), SettingsSection.developer);
    });

    test('越界（导航状态不该能让页面崩）', () {
      expect(SettingsSection.byIndex(-1), SettingsSection.persona);
      expect(SettingsSection.byIndex(8), SettingsSection.persona);
      expect(SettingsSection.byIndex(9999), SettingsSection.persona);
    });
  });

  group('dev_mode 只门控「开发模式」分区', () {
    test('dev 关 → 7 个分区', () {
      final List<SettingsSection> sections = visibleSections(devMode: false);
      expect(sections, hasLength(7));
      expect(sections, isNot(contains(SettingsSection.developer)));
    });

    test('dev 开 → 8 个分区，且顺序不变', () {
      final List<SettingsSection> sections = visibleSections(devMode: true);
      expect(sections, SettingsSection.values);
    });

    test('只有 developer 是 isDevOnly', () {
      for (final SettingsSection s in SettingsSection.values) {
        expect(s.isDevOnly, s == SettingsSection.developer, reason: s.name);
      }
    });

    test('过滤后顺序与原顺序一致（不重排）', () {
      final List<SettingsSection> visible = visibleSections(devMode: false);
      final List<SettingsSection> expected = SettingsSection.values
          .where((SettingsSection s) => s != SettingsSection.developer)
          .toList();
      expect(visible, expected);
    });
  });
}
