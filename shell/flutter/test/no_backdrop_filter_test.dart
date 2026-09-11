import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'design_tokens_test.dart' show stripCommentsAndStrings;

/// 性能与分层的**静态红线**（规格 §11.4 钉子 10/11 + §11.1 导入守卫）。
///
/// 这些规则全部是「静态扫描源码」，因为它们守的是**不该出现的东西**——
/// 而「不该出现的东西」用运行时测试是抓不住的（没写就是没写，
/// 但一旦有人写回去了，只有扫描能发现）。
///
/// 单独成文件的理由：这三条是**跨模块的红线**（性能 / 无障碍 / 分层），
/// 不属于任何单个模块的令牌门禁。
void main() {
  /// 扫到的 `lib/**` 源文件（去掉注释与字符串字面量后再匹配）。
  List<File> libSources() => Directory('lib')
      .listSync(recursive: true)
      .whereType<File>()
      .where((File f) => f.path.endsWith('.dart'))
      .toList();

  /// 逐行扫描并返回 `路径:行号: 内容` 形式的命中列表。
  List<String> scan(RegExp pattern, {bool Function(String path)? skip}) {
    final List<String> hits = <String>[];
    for (final File file in libSources()) {
      if (skip != null && skip(file.path)) continue;
      final List<String> lines = stripCommentsAndStrings(
        file.readAsStringSync(),
      ).split('\n');
      for (int i = 0; i < lines.length; i++) {
        if (pattern.hasMatch(lines[i])) {
          hits.add('${file.path}:${i + 1}: ${lines[i].trim()}');
        }
      }
    }
    return hits;
  }

  test('扫描确实覆盖到了源码（防路径写错导致「零命中=通过」）', () {
    // 这是最危险的假通过：路径写错 → 一个文件都没扫 → 全部规则空转通过。
    final List<File> sources = libSources();
    expect(sources, isNotEmpty);
    expect(sources.length, greaterThanOrEqualTo(30));
    expect(
      sources.any((File f) => f.path.endsWith('main.dart')),
      isTrue,
      reason: '连 main.dart 都没扫到，说明扫描根路径不对',
    );
  });

  group('钉子 10：`BackdropFilter` 命中数为 0', () {
    test('lib/** 里没有任何 BackdropFilter', () {
      final List<String> hits = scan(RegExp(r'BackdropFilter'));
      expect(
        hits,
        isEmpty,
        reason: 'BackdropFilter 模糊不到 iframe 平台视图（上游 issue #184996），'
            '却照付每帧离屏渲染的代价。GlassPanel 用半透明纯色 + 1px 描边。\n'
            '${hits.join('\n')}',
      );
    });

    test('也不许用 ImageFilter.blur 绕过（同一笔代价）', () {
      final List<String> hits = scan(RegExp(r'ImageFilter\.blur'));
      expect(hits, isEmpty, reason: hits.join('\n'));
    });
  });

  group('钉子 11：`contentFaint` 不得承载文字', () {
    test('没有把 contentFaint 传给任何 Text / TextStyle', () {
      // `contentFaint` 是 onSurface @0.60 —— 只允许用于装饰与图标，
      // 承载文字会低于对比度门槛（规格 §9.5）。这是很容易顺手写出的低对比文案。
      final List<String> hits = scan(
        RegExp(r'(Text|TextStyle|style)[^\n]*contentFaint'),
      );
      expect(
        hits,
        isEmpty,
        reason: 'contentFaint（@0.60）只允许给图标/装饰用；文字请用 contentMuted。\n'
            '${hits.join('\n')}',
      );
    });

    test('图标用 contentFaint 是允许的（并确认确实有人这么用）', () {
      final List<String> hits = scan(RegExp(r'color:\s*[^\n]*contentFaint'));
      // 不断言非空（将来可能全改用 contentMuted）；只要求这些命中**不含** Text。
      for (final String hit in hits) {
        expect(hit.contains('Text('), isFalse, reason: hit);
      }
    });
  });

  group('导入守卫（规格 §11.1）：纯逻辑文件不许碰 Flutter/Web', () {
    /// **完全不 import 任何包**（只用 `dart:*`）——最强的纯逻辑。
    const List<String> pureDartOnly = <String>[
      'lib/settings/display_prefs.dart',
      'lib/audio/gain.dart',
      'lib/design/breakpoints.dart',
    ];

    /// 允许 `pureDartOnly` 里的文件依赖的**同样零依赖**的本地文件。
    ///
    /// 2026-09-11：`display_prefs.dart` 需要 `AppThemeId`（主题标识）。
    /// 判定标准不是「路径白名单」，而是**依赖闭包是否仍然零依赖**——
    /// 所以下面同时断言这些被依赖的文件自己 `import` 数为 0。
    /// 这样放宽不会变成后门：往 `theme_id.dart` 里加一个
    /// `import 'package:flutter/material.dart'` 会当场挂。
    const Set<String> zeroDependencyLocals = <String>{
      "import '../design/theme_id.dart';",
    };

    test('这三个文件只 import dart:*（或零依赖的本地文件）', () {
      for (final String path in pureDartOnly) {
        final String source = File(path).readAsStringSync();
        final List<String> imports = RegExp(r"^import .*$", multiLine: true)
            .allMatches(source)
            .map((RegExpMatch m) => m.group(0)!)
            .toList();
        for (final String line in imports) {
          expect(
            line.contains("'dart:") ||
                zeroDependencyLocals.contains(line.trim()),
            isTrue,
            reason: '$path 只能 import dart:*（纯逻辑，可在任何环境跑）：$line',
          );
        }
        expect(
          source.contains("package:flutter"),
          isFalse,
          reason: '$path 不该依赖 Flutter（那样主题一变就要跑 widget 测试）',
        );
      }
    });

    test('被放行的那几个本地文件自己**零 import**（放宽不是后门）', () {
      for (final String line in zeroDependencyLocals) {
        final String quoted = RegExp(r"'([^']+)'").firstMatch(line)!.group(1)!;
        // 相对 `lib/settings/`（`pureDartOnly` 里的文件都在那一层）。
        final List<String> parts = <String>[
          'lib',
          'settings',
          ...quoted.split('/'),
        ];
        final List<String> stack = <String>[];
        for (final String part in parts) {
          if (part == '..') {
            stack.removeLast();
          } else if (part != '.') {
            stack.add(part);
          }
        }
        final String path = stack.join('/');
        final String source = File(path).readAsStringSync();
        expect(
          RegExp(r"^import ", multiLine: true).hasMatch(source),
          isFalse,
          reason: '$path 必须零 import，否则「纯逻辑」这个放行理由不成立',
        );
      }
    });

    test('design/** 与 state/** 不许 import package:web', () {
      // 2026-09-11：原先这里还有 `lib/actions/`（动作子系统）——该目录已随
      // 动作功能整条删除，条件里就不再留一个永远匹配不到的死路径。
      final List<String> hits = scan(
        RegExp(r"import 'package:web"),
        skip: (String path) =>
            !path.startsWith('lib/design/') && !path.startsWith('lib/state/'),
      );
      expect(hits, isEmpty, reason: hits.join('\n'));
    });

    test('`package:web` 只出现在真正需要 DOM 的地方（白名单）', () {
      // 允许清单：本地存储（main）、WS 客户端、渲染面 host、音频播放。
      // **新增一项都要先问「能不能不碰 DOM」**——碰了就等于放弃 VM 可测性。
      const Set<String> allowed = <String>{
        'lib/main.dart',
        'lib/api/ws_client.dart',
        'lib/live2d/live2d_host_web.dart',
        'lib/audio/audio_player.dart',
      };
      final List<String> hits = scan(
        RegExp(r"import 'package:web"),
        skip: (String path) => allowed.contains(path),
      );
      expect(
        hits,
        isEmpty,
        reason: '${hits.join('\n')}\n'
            '（`package:web` 让整个文件在 flutter test 里加载不了；'
            '如果能抽出纯逻辑，就该抽出来）',
      );
    });
  });

  group('性能：舞台必须被 RepaintBoundary 包住', () {
    test('`AppShell` 里舞台外层有 RepaintBoundary', () {
      // 30 Hz 的口型如果让整棵聊天列表跟着重绘，长会话下会明显掉帧。
      final String source = File('lib/app/app_shell.dart').readAsStringSync();
      expect(source.contains('RepaintBoundary'), isTrue);
    });
  });
}
