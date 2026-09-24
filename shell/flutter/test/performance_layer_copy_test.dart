/// **表演层文案门禁**（2026-09-22）：防「旧 director staging / 新表演层」两套说法回潮。
///
/// 这一组断言很土，但它守的是一件事：用户读到的界面必须明确
/// **表演层是主、staging 是遗留回退**，且 LLM 分区保留三态说明。
/// 文案回潮不会让任何测试变红——除非这里有钉子。所以钉子在这。
///
/// 对应的产品事实见 `docs/architecture/performance-layer-v0.md` §7 与
/// `docs/architecture/director-mod-v0.md` 顶注（同一张「谁主谁退」表）。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/mods/director_panel.dart';
import 'package:live2d_ai_shell/settings/sections/llm_section.dart';

void main() {
  group('LLM 分区：主模型不负责表演 + 三态 + 指向 [performance]', () {
    test('锚点句原文不变', () {
      expect(kLlmPerformanceText, '主模型不负责表演；表演层每轮 JSON');
    });

    test('三态（只说 / 只动 / noop=空字段）都在说明里', () {
      const String d = kLlmPerformanceDescription;
      expect(d, contains('只说'));
      expect(d, contains('speak 有值'));
      expect(d, contains('只动'));
      expect(d, contains('cues 有值'));
      expect(d, contains('noop'));
      expect(d, contains('空字段'));
      // 配置指向 [performance]，不是 director/mod。
      expect(d, contains('[performance]'));
      expect(d, contains('enabled'));
      expect(d, contains('base_url'));
    });
  });

  group('director 面板：staging 是遗留回退旁路，不是主路径', () {
    test('并存说明含「遗留 / staging / performance / SentenceReady」与处置句', () {
      const String n = kDirectorLegacyNotice;
      expect(n, contains('遗留'));
      expect(n, contains('staging_*'));
      expect(n, contains('performance'));
      expect(n, contains('SentenceReady'));
      // 一句话给出该去哪儿配。
      expect(n, contains('设置 → LLM → 表演层'));
      expect(n, contains('仅兼容旧配置'));
    });
  });
}
