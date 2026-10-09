import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/settings_sections.dart';

void main() {
  group('分区声明：单点真相（规格 §4.0 / 钉子 12）', () {
    test('恰好 7 个分区（2026-10-09：一级从 8 项收成 7 项）', () {
      expect(SettingsSection.values, hasLength(7));
    });

    test('**每项都有非空的 label 与 description**（用户不该先学会黑话）', () {
      for (final SettingsSection s in SettingsSection.values) {
        expect(s.label.trim(), isNotEmpty, reason: '${s.name} 缺 label');
        expect(
          s.description.trim(),
          isNotEmpty,
          reason: '${s.name} 缺 description —— 每项都要一句短的人话',
        );
      }
    });

    test('label 与 description 都**无重号**（防复制粘贴漏改）', () {
      final List<String> labels = SettingsSection.values
          .map((SettingsSection s) => s.label)
          .toList();
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

    test('图标两两不同（导航里靠形状区分 7 项）', () {
      final Set<IconData> icons = SettingsSection.values
          .map((SettingsSection s) => s.icon)
          .toSet();
      expect(icons, hasLength(7));
    });

    test('顺序 = 声明顺序（2026-10-09 起固定为这 7 项）', () {
      expect(
        SettingsSection.values.map((SettingsSection s) => s.name),
        <String>[
          'persona',
          'models',
          'service',
          'theme',
          'motion',
          'mods',
          'developer',
        ],
      );
    });

    test('index 与声明顺序一致（NavigationRail 的 selectedIndex 直接吃它）', () {
      for (int i = 0; i < SettingsSection.values.length; i++) {
        expect(SettingsSection.values[i].index, i);
      }
    });
  });

  group('byIndex：越界回落第一个，**不抛**', () {
    test('合法索引（0 仍是 persona，最后一项是 developer）', () {
      expect(SettingsSection.byIndex(0), SettingsSection.persona);
      expect(SettingsSection.byIndex(6), SettingsSection.developer);
    });

    test('越界（导航状态不该能让页面崩）', () {
      expect(SettingsSection.byIndex(-1), SettingsSection.persona);
      expect(SettingsSection.byIndex(7), SettingsSection.persona);
      expect(SettingsSection.byIndex(9999), SettingsSection.persona);
    });
  });

  group('2026-10-09：标题与说明的口径', () {
    test('persona 的显示名是「模型对话」（枚举值不改）', () {
      expect(SettingsSection.persona.label, '模型对话');
      expect(SettingsSection.persona.description, '系统提示词与记住的轮数');
      expect(SettingsSection.persona.description.contains('人设'), isFalse);
      expect(SettingsSection.persona.description.contains('模型对话'), isFalse);
    });

    test('原「对话」+「语音合成」合成「模型服务」', () {
      expect(SettingsSection.service.label, '模型服务');
      expect(SettingsSection.service.description.contains('TTS'), isFalse);
      expect(SettingsSection.service.description.contains('LLM'), isFalse);
      final List<String> names = SettingsSection.values
          .map((SettingsSection s) => s.name)
          .toList();
      expect(names.contains('llm'), isFalse);
      expect(names.contains('tts'), isFalse);
    });

    test('原「外观与互动」拆成「主题」+「Live2D 动作」', () {
      expect(SettingsSection.theme.label, '主题');
      expect(SettingsSection.theme.description, '配色与背景');
      expect(SettingsSection.motion.label, 'Live2D 动作');
      final List<String> names = SettingsSection.values
          .map((SettingsSection s) => s.name)
          .toList();
      expect(names.contains('appearance'), isFalse);
    });

    test('一级「诊断」取消：内容收进开发模式页', () {
      final List<String> names = SettingsSection.values
          .map((SettingsSection s) => s.name)
          .toList();
      expect(names.contains('diagnostics'), isFalse);
    });

    test('Mod → 「扩展」', () {
      expect(SettingsSection.mods.label, '扩展');
      expect(SettingsSection.mods.description.contains('Mod'), isFalse);
    });
  });

  group('可见分区：**没有任何一级会随 devMode 消失**', () {
    test('两个 devMode 下都是同一份 7 项', () {
      final List<SettingsSection> off = visibleSections();
      final List<SettingsSection> on = visibleSections(devMode: true);
      expect(off, hasLength(7));
      expect(on, hasLength(7));
      expect(off, SettingsSection.values);
      expect(on, SettingsSection.values);
      expect(visibleSections(devMode: false), visibleSections());
    });

    test('「开发模式」永远在（藏了它就再也打不开）', () {
      for (final bool dev in <bool>[false, true]) {
        expect(
          visibleSections(devMode: dev),
          contains(SettingsSection.developer),
          reason: 'devMode=$dev',
        );
      }
    });

    test('返回的是**不可变**列表（防调用方顺手改全局顺序）', () {
      expect(
        () => visibleSections().add(SettingsSection.persona),
        throwsUnsupportedError,
      );
      expect(
        () => visibleSections(devMode: true).add(SettingsSection.persona),
        throwsUnsupportedError,
      );
    });
  });
}
