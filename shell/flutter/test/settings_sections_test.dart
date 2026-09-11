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

    test('顺序 = 声明顺序：角色 → 能力 → 外观与互动 → 扩展/诊断/开发', () {
      expect(SettingsSection.values.map((SettingsSection s) => s.name), <String>[
        'persona',
        'models',
        'llm',
        'tts',
        // 2026-09-11：动作子系统移除时这里由 `actions` 改名为 `appearance`
        // （label「外观与互动」不变）。索引位置保持不变，改的是名字。
        'appearance',
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

  group('分区清单**恒定**（dev_mode 不再藏分区）', () {
    // 2026-09-11 修：过去 dev_mode 关时「开发模式」分区被过滤掉，
    // 而唯一能打开 dev_mode 的开关**就在那个分区里** —— 于是它在界面上
    // 永远打不开（只有 curl / `--dev-mode` 能开）。死循环。
    test('无论 dev_mode 如何，8 个分区都在（含「开发模式」）', () {
      final List<SettingsSection> sections = visibleSections();
      expect(sections, hasLength(8));
      expect(sections, contains(SettingsSection.developer));
      expect(sections, SettingsSection.values);
    });

    test('顺序 = 枚举声明顺序（不重排）', () {
      expect(visibleSections(), SettingsSection.values);
    });

    test('返回的是**不可变**列表（防调用方顺手改全局顺序）', () {
      expect(
        () => visibleSections().add(SettingsSection.persona),
        throwsUnsupportedError,
      );
    });

    test('不再有 isDevOnly 这个「藏分区」的口子', () {
      // 保留这条断言是为了说明：要藏的是**分区内的字段**，不是分区。
      // 哪天有人想加回来，先读 `visibleSections` 的头注。
      expect(
        SettingsSection.values.where((SettingsSection s) => s.name == 'developer'),
        hasLength(1),
      );
    });
  });
}
