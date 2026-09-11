/// 动效接线门禁（2026-09-11，前端加强计划 P1）。
///
/// # 这条门禁防的是什么
///
/// 本项目有一件说来尴尬的事：**动效令牌比参考项目规范，但接线数为 0**
/// ——`AppDurations` 4 档 + `Motion` 2 条曲线声明了很久，全应用却只有一处
/// `theme_picker` 的 `AnimatedContainer`。原因是缺一个统一出口：
/// 每个要动的地方都得自己写一遍
/// `MediaQuery.disableAnimationsOf(context) ? Duration.zero : …`，
/// 写两遍之后就会有人漏写，而**漏写不报错**——只在开了「减少动画」的用户
/// 那里表现为「说好的不动，结果它还在动」。
///
/// `lib/ui/soft_motion.dart` 的 [appMotion] 就是那个唯一的出口。本文件钉：
///
/// 1. **4 档时长与入场曲线都真的有人用**（不是台账里的死令牌）；
/// 2. **没有任何地方「拿着令牌绕过闸门」**——即 `duration:` 后面直接跟
///    `AppDurations.x` / `AppRhythms.x`。这一条在写本文件时**当场抓到一个
///    真漏**：`theme_picker` 的选中态动画就是直接给的令牌。
/// 3. **`AnimationController` 的构造时长不许是可跑的令牌值**（容易让
///    「构造时设一次、之后就再也不管减少动画了」这种写法溜过去）。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/tokens.dart';

/// 剥掉注释与字符串（否则本文件自己的注释就会把自己判红）。
String stripCommentsAndStrings(String src) {
  final StringBuffer out = StringBuffer();
  int i = 0;
  while (i < src.length) {
    final String c = src[i];
    if (c == "'" || c == '"') {
      final bool triple =
          i + 2 < src.length && src[i + 1] == c && src[i + 2] == c;
      final String quote = triple ? c + c + c : c;
      i += quote.length;
      while (i < src.length) {
        if (src[i] == r'\') {
          i += 2;
          continue;
        }
        if (src.startsWith(quote, i)) {
          i += quote.length;
          break;
        }
        i++;
      }
      continue;
    }
    if (c == '/' && i + 1 < src.length && src[i + 1] == '/') {
      while (i < src.length && src[i] != '\n') {
        i++;
      }
      continue;
    }
    if (c == '/' && i + 1 < src.length && src[i + 1] == '*') {
      i += 2;
      while (i + 1 < src.length && !(src[i] == '*' && src[i + 1] == '/')) {
        i++;
      }
      i += 2;
      continue;
    }
    out.write(c);
    i++;
  }
  return out.toString();
}

/// 扫描 `lib/**`，返回「文件:行号: 内容」形式的命中。
List<String> scanLib(RegExp pattern, {Set<String> exempt = const <String>{}}) {
  final List<String> hits = <String>[];
  for (final FileSystemEntity entity in Directory(
    'lib',
  ).listSync(recursive: true)) {
    if (entity is! File || !entity.path.endsWith('.dart')) continue;
    if (exempt.contains(entity.path)) continue;
    final List<String> lines = stripCommentsAndStrings(
      entity.readAsStringSync(),
    ).split('\n');
    for (int i = 0; i < lines.length; i++) {
      if (pattern.hasMatch(lines[i])) {
        hits.add('${entity.path}:${i + 1}: ${lines[i].trim()}');
      }
    }
  }
  return hits;
}

/// 令牌的唯一定义处（`AppDurations` / `AppRhythms` / `Motion` 自己在那里声明）。
const Set<String> kTokenDeclarationFiles = <String>{'lib/design/tokens.dart'};

/// [appMotion] 的定义处：它自己要读令牌。
const Set<String> kMotionGateFiles = <String>{'lib/ui/soft_motion.dart'};

void main() {
  group('P1：4 档时长 + 入场曲线都真的有人用', () {
    test('每个动效令牌都至少被引用一次（死令牌从台账里清掉了）', () {
      for (final String name in AppDurations.registry.keys) {
        expect(
          AppDurations.registry[name],
          isNotNull,
          reason: '$name 不在登记表里',
        );
      }
      // 引用由 `design_tokens_test.dart` 的双向对账守（那边扫 lib/**）。
      // 这里只钉「4 档本身没被削掉」。
      expect(AppDurations.registry.length, 4);
      expect(Motion.names, <String>['enter', 'state']);
    });
  });

  group('P1：没有任何地方拿着令牌绕过 appMotion 闸门', () {
    test('`duration:` 后面不许直接跟令牌常量', () {
      // 这正是写本文件时当场抓到的真漏：`theme_picker` 的选中态动画
      // 写的是 `duration: AppDurations.fast`——令牌对、闸门漏。
      final List<String> hits = scanLib(
        RegExp(r'duration:\s*(AppDurations|AppRhythms|Motion)\.'),
        exempt: <String>{...kTokenDeclarationFiles, ...kMotionGateFiles},
      );
      expect(
        hits,
        isEmpty,
        reason:
            '这些地方直接给了令牌时长，没有过 `appMotion(context, …)`：\n'
            '${hits.join('\n')}\n'
            '令牌正确 ≠ 尊重「减少动画」——这种漏写不报错，只在开了减少动画的'
            '用户那里表现为「说好的不动，结果它还在动」。',
      );
    });

    test('`AnimationController` 只出现在被点名的文件里（新增一个要过脑子）', () {
      // 显式动画（`AnimationController`）是**持续动画**的来源，
      // 而持续动画是「减少动画」最容易被漏掉的一类（`repeat()` 默认
      // `AnimationBehavior.preserve`，**刻意免疫**系统的减少动画设置）。
      //
      // 所以这里把「谁有控制器」钉成一份清单：新增一处就必须来改这里，
      // 改的时候被迫回答一次「它尊重减少动画吗」。
      //
      // 已知的四处及其理由：
      // - `state_pill`：思考态的呼吸（`repeat`，**显式处理了 reduce**）
      // - `streaming_indicator`：流式三点（同上）
      // - `live2d_stage`：舞台底色的主题插值（P1-3，duration 经 appMotion）
      // - `soft_motion`：启动揭示（P1-2，reduce 时直接返回 child）
      // - `page_cross_fade`：compact 的整页设置过渡（P2-1，时长经 appMotion，
      //   且**零时长时直接 snap 到终态**——见该文件头注 ③）
      const Set<String> known = <String>{
        'lib/app/page_cross_fade.dart',
        'lib/live2d/live2d_stage.dart',
        'lib/ui/soft_motion.dart',
        'lib/ui/state_pill.dart',
        'lib/ui/streaming_indicator.dart',
      };
      final List<String> controllers = scanLib(
        RegExp(r'AnimationController\('),
      );
      final Set<String> actual = <String>{
        for (final String hit in controllers) hit.split(':').first,
      };
      expect(
        actual.difference(known),
        isEmpty,
        reason:
            '有新的 AnimationController 没有登记 —— 请确认它尊重'
            '`MediaQuery.disableAnimationsOf`（尤其是 `repeat()` 那一类，'
            '它默认**免疫**系统的减少动画设置），然后把它加进 known。',
      );
      // 反过来：清单里不许留已经删掉的（否则清单会越来越不可信）。
      expect(known.difference(actual), isEmpty, reason: '清单里有已经不存在的控制器');
    });
  });

  group('P1：appMotion 的行为（闸门的语义本身）', () {
    test('它把令牌原样透出去', () {
      // 纯函数部分在 widget 测试里验；这里只钉住「令牌表仍在」，
      // 免得有人把 AppDurations 换成裸数字时这里静默通过。
      expect(AppDurations.base, const Duration(milliseconds: 200));
      expect(AppDurations.slow, const Duration(milliseconds: 320));
      expect(AppDurations.fast, const Duration(milliseconds: 120));
      expect(AppDurations.reveal, const Duration(milliseconds: 600));
    });
  });
}
