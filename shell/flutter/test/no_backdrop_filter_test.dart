/// 性能与分层的**静态红线**（规格 §11.4 钉子 10/11 + §11.1 导入守卫）。
///
/// 这些规则全部是「静态扫描源码」，因为它们守的是**不该出现的东西**——
/// 而「不该出现的东西」用运行时测试是抓不住的（没写就是没写，
/// 但一旦有人写回去了，只有扫描能发现）。
///
/// 单独成文件的理由：这三条是**跨模块的红线**（性能 / 无障碍 / 分层），
/// 不属于任何单个模块的令牌门禁。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

/// **文字样式的形参名**：出现在某个调用实参里 ⇒ 这段颜色最终喂给了文字。
///
/// 2026-09-28（task-16 / F-0006-2，与 F-0006-3 同型）：钉子 11 原来的判据是
/// **同行**正则 `(Text|TextStyle|style)[^\n]*contentFaint`，于是最自然的跨行写法
///
/// ```dart
/// Text(
///   '正文',
///   style: theme.textTheme.labelSmall?.copyWith(
///     color: colors.contentFaint,          // ← 与 `style:` 不在同一行
///   ),
/// )
/// ```
///
/// **一个都拦不住**（`field_row.dart` / `appearance_background.dart` 就是这么
/// 绕过去的，审计的「反向证据」也因此把两处文字判成了非文字）。现在改成
/// **结构判定**：向上做括号配平找到所有包住它的调用，再看「它所在的那一段实参」
/// 有没有文字形参名——跨多少行都不影响。
const List<String> kTextStyleArgNames = <String>[
  'style',
  'labelStyle',
  'hintStyle',
  'textStyle',
  'contentTextStyle',
  'titleTextStyle',
  'subtitleTextStyle',
  'collapsedTextStyle',
  'counterStyle',
];

/// **文字的宿主调用**：这些调用的实参里出现颜色 ⇒ 无论形参名叫什么都是文字。
///
/// 覆盖 `Text('a', t.bodySmall.copyWith(color: faint))` 这种**省略 `style:`** 的
/// 位置实参写法（只看形参名会漏）。
const Set<String> kTextHosts = <String>{
  'Text',
  'SelectableText',
  'RichText',
  'TextSpan',
  'TextStyle',
  'DefaultTextStyle',
};

/// `contentFaint` 的**允许宿主白名单**：每一处都必须是「不承载文字」的通道。
///
/// key = `<相对包根的路径>::<宿主调用名>`；value = **为什么它不是文字**。
/// 新增一处 `contentFaint` 用法就会让「白名单对账」那条测试红——先确认它是
/// 图标/描边/手柄这类**非文字通道**，再把理由写进来。
///
/// 为什么用「宿主调用名」而不是行号：行号会随别人改文件漂移，宿主名表达的是
/// **场景**（`Icon` / 拖拽把手 / 开关拇指），挪行不影响判据。
const Map<String, String> kFaintDecorationHosts = <String, String>{
  'lib/ui/chat_panel.dart::Icon': '聊天面板里的图标（Icon 的 color，非文字通道）',
  'lib/ui/theme.dart::BottomSheetThemeData':
      '底部浮层的拖拽把手色（dragHandleColor，装饰）',
  'lib/ui/theme.dart::resolveWith<Color?>':
      '开关**关闭态**的拇指色（SwitchTheme 的 WidgetStateProperty，非文字元件）',
  // 2026-10-06（R4-T2）：背景域拆成三个 part，`_LibraryRow` 现在住在
  // `appearance_background_library.dart` —— 锚点跟着搬家，判据不变。
  'lib/settings/sections/appearance_background_library.dart::Icon':
      '背景库行首的拖拽手柄图标（Icon 的 color）',
  'lib/settings/sections/dev_tools_section.dart::Icon':
      '模型库空态的占位图标（Icon 的 color）',
};

/// **已知陷阱宿主**：这些宿主的形参**同时**喂给文字通道，所以 `contentFaint`
/// 一律不许出现在它们身上——**连白名单也不许收**（防「顺手加进白名单」把陷阱放行）。
///
/// 2026-09-28（task-16）：`UiPhaseView.tone` 就是这一个——`state_pill.dart` 里
/// 同一颗 `tone` 驱动图标 / 实心点 / 描边 / 胶囊底 **和标签字色**
/// （`Text(view.label, …copyWith(color: view.tone))`）。只看形参名（`tone:`）
/// 看不出它是文字，所以这条陷阱必须**名字级**禁掉。
const Set<String> kFaintForbiddenHosts = <String>{'UiPhaseView'};

/// **未迁移的文字色债务**（动态判定，见 `findFaintUses`）。
///
/// `lib/ui/field_row.dart:228/:236`（滑杆两端的 min/max 刻度）在 task-16 里
/// **刻意不改**——该文件在 W1-b 手上，那两处由它的 worker 顺手迁移。
///
/// 这里用「按路径豁免 + 条数上限」而不是把两处写进白名单：**迁移完成后它们
/// 自然消失，断言不会因此变红，也不存在需要谁来删的死条目**（跨 worker 的
/// 台账如果要求「删了才绿」，两边会互相打红）。
const Set<String> kFaintTextDebtPaths = <String>{'lib/ui/field_row.dart'};

/// 每个债务路径上**最多**还能有几处文字色（防债务悄悄长大）。
const int kFaintTextDebtCap = 2;

/// 一个 `contentFaint` 出现点。
class FaintUse {
  const FaintUse({
    required this.line,
    required this.host,
    required this.snippet,
    required this.isText,
    required this.textReason,
  });

  final int line;

  /// 宿主调用名（`Icon` / `copyWith` / `resolveWith<Color?>` …）。
  final String host;

  final String snippet;

  /// 是不是文字场景（钉子 11 的判据）。
  final bool isText;

  /// 判定成文字时的依据（形参名或宿主名），失败信息里打出来。
  final String? textReason;

  String get where => '$host:$line';
}

/// 取「包含 `at` 的那一段实参」：`open` 之后**最后一个 depth-0 逗号**到 `at`。
///
/// 这是结构判定的关键一步：只看「向上所有的调用」会把**兄弟实参**的 `style:`
/// 也算进来（`MyWidget(leading: Text(…, style: …), trailing: Icon(color: faint))`
/// 的 `Icon` 会被误判成文字）。切到「自己所在的那一段实参」就不会。
String _argumentSegment(String s, int open, int at) {
  int depth = 0;
  int lastComma = open;
  for (int k = open + 1; k < at; k++) {
    final String c = s[k];
    if (c == '(' || c == '[' || c == '{') {
      depth++;
    } else if (c == ')' || c == ']' || c == '}') {
      depth--;
    } else if (c == ',' && depth == 0) {
      lastComma = k;
    }
  }
  return s.substring(lastComma + 1, at);
}

/// 调用头：`(` 之前的标识符链（可带 `.<name>` 与一个 `<...>` 泛型实参）。
String _callHeadBefore(String s, int open) {
  final String window = s.substring(open > 80 ? open - 80 : 0, open);
  final Match? m = RegExp(
    r'([A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*(?:<[^()\[\]{}]*>)?)\s*$',
  ).firstMatch(window);
  return m == null ? '' : m.group(1)!;
}

/// 宿主名：取标识符链的**最后一段**（`WidgetStateProperty.resolveWith<Color?>`
/// → `resolveWith<Color?>`），这样白名单的键不受调用方写法影响。
String faintHostKey(String head) {
  final int dot = head.lastIndexOf('.');
  return dot < 0 ? head : head.substring(dot + 1);
}

/// 向上做括号配平，返回包住 `at` 的调用 `(宿主头, 它所在的那段实参)`，由内到外。
///
/// 跨行、跨任意层 `copyWith(withValues(…))` 都成立；遇到 `;` / `[` / `{` 就停
/// （那是语句与列表的边界，再往上就是**别的**兄弟了）。
List<(String, String)> faintEnclosingCalls(String s, int at) {
  final List<(String, String)> out = <(String, String)>[];
  int depth = 0;
  int i = at - 1;
  while (i >= 0 && out.length < 12) {
    final String c = s[i];
    if (c == ')' || c == ']' || c == '}') {
      depth++;
    } else if (c == '(' || c == '[' || c == '{') {
      if (depth > 0) {
        depth--;
      } else if (c == '(') {
        out.add((_callHeadBefore(s, i), _argumentSegment(s, i, at)));
      } else {
        break; // 列表 / 块边界
      }
    } else if (depth == 0 && c == ';') {
      break;
    }
    i--;
  }
  return out;
}

/// 扫一段源码里**所有** `contentFaint` 用法（先剥注释与字符串字面量）。
///
/// 传进来的应当是**未剥注释**的原文——函数内部自己剥，保证调用方拿到的行号
/// 与源码一致（`stripCommentsAndStrings` 不新增/不删除换行）。
List<FaintUse> findFaintUses(String source) {
  final String s = stripCommentsAndStrings(source);
  final List<FaintUse> out = <FaintUse>[];
  final RegExp argNames = RegExp(
    '\\b(?:${kTextStyleArgNames.join('|')})\\s*:',
  );
  for (final RegExpMatch m in RegExp(r'\bcontentFaint\b').allMatches(s)) {
    final List<(String, String)> calls = faintEnclosingCalls(s, m.start);
    String? reason;
    for (final (String head, String args) in calls) {
      final bool isTextHost =
          kTextHosts.contains(head) || kTextHosts.contains(head.split('.').first);
      if (isTextHost) {
        reason = '宿主 $head';
        break;
      }
      final RegExpMatch? arg = argNames.firstMatch(args);
      if (arg != null) {
        reason = '实参 ${arg.group(0)!.trim()}';
        break;
      }
    }
    final int line = '\n'.allMatches(s.substring(0, m.start)).length + 1;
    final int lineStart = s.lastIndexOf('\n', m.start) + 1;
    final int lineEnd = s.indexOf('\n', m.start);
    out.add(
      FaintUse(
        line: line,
        host: calls.isEmpty ? '(顶层)' : faintHostKey(calls.first.$1),
        snippet: s
            .substring(lineStart, lineEnd < 0 ? s.length : lineEnd)
            .trim(),
        isText: reason != null,
        textReason: reason,
      ),
    );
  }
  return out;
}

/// 只挑**文字场景**的用法（钉子 11 的判据）。
List<FaintUse> findFaintTextUses(String source) =>
    findFaintUses(source).where((FaintUse u) => u.isText).toList();

void main() {
  /// 扫到的 `lib/**` 源文件（去掉注释与字符串字面量后再匹配）。
  List<File> libSources() =>
      Directory('lib')
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
        reason:
            'BackdropFilter 模糊不到 iframe 平台视图（上游 issue #184996），'
            '却照付每帧离屏渲染的代价。GlassPanel 用半透明纯色 + 1px 描边。\n'
            '${hits.join('\n')}',
      );
    });

    // 2026-09-27：**一处刻意、且被限定死的例外**。
    //
    // 原规则禁的是「用 ImageFilter.blur 绕过 BackdropFilter」——
    // 那个批评对 `BackdropFilter` 完全成立：它模糊的是**整块合成结果**，
    // 每帧离屏渲染，而结果里还混着一个根本模糊不到的 iframe 平台视图。
    //
    // 背景模糊是**另一回事**：`ImageFiltered` 只作用于**一张静态图自己的图层**，
    // 外面还套了 `RepaintBoundary` —— 栅格化一次之后就不再重算，
    // 静止时每帧代价为 0。它模糊的是「那张图」，不是「整个壳」。
    //
    // 为什么值得开这个口子：模糊是背景系统里最被需要的一档
    // （一张高清照片直接当背景会抢模型的注意力），而没有它的话
    // 「模糊」滑杆就是一个**没有接线**的控件 —— 那正是本项目 P4 要治的病。
    //
    // 例外被限定在三件事上：① 只允许出现在这一个文件；
    // ② sigma 由偏好上界（8 px）压着；③ 必须有 `RepaintBoundary`。
    // 任何一条被破坏，下面两条测试都会红。
    const String blurExceptionFile = 'lib/ui/shell_backdrop.dart';

    test('ImageFilter.blur 只许出现在背景层那一个文件里', () {
      final List<String> hits = scan(
        RegExp(r'ImageFilter\.blur'),
        skip: (String path) => path == blurExceptionFile,
      );
      expect(
        hits,
        isEmpty,
        reason:
            '模糊的例外只给 $blurExceptionFile（静态图层 + RepaintBoundary）；'
            '别处出现就说明有人开始拿它做逐帧的活。\n${hits.join('\n')}',
      );
    });

    test('那个文件里必须真的套了 RepaintBoundary（否则例外不成立）', () {
      final String source = File(blurExceptionFile).readAsStringSync();
      expect(
        source.contains('RepaintBoundary'),
        isTrue,
        reason: '没有 RepaintBoundary 就没有「只算一次」这回事，例外立刻失效',
      );
      expect(
        source.contains('ImageFiltered'),
        isTrue,
        reason: '例外存在的前提是这一处真的在用 ImageFiltered',
      );
    });
  });

  group('钉子 11：`contentFaint` 不得承载文字', () {
    /// `contentFaint` 的**定义处**（令牌声明文件）：那里 `contentFaint` 是字段名，
    /// 不是用法——与 `design_tokens_lint_test.dart` 的豁免同一条口径。
    const String tokenDeclarationFile = 'lib/design/tokens.dart';

    /// 扫全部 `lib/**` 的 `contentFaint` 用法。
    List<(String, FaintUse)> allFaintUses() {
      final List<(String, FaintUse)> out = <(String, FaintUse)>[];
      for (final File file in libSources()) {
        for (final FaintUse use in findFaintUses(
          file.readAsStringSync(),
        )) {
          out.add((file.path, use));
        }
      }
      return out;
    }

    test('判据本身有效：确实扫到了 contentFaint 用法（防「零命中=通过」）', () {
      final List<(String, FaintUse)> sites = allFaintUses();
      expect(sites, isNotEmpty, reason: '一个 contentFaint 都没扫到，判据写坏了');
      // 真正的**用法**（排除令牌定义文件）应当既有装饰用法、也确实还看得见现实：
      // 判据若退化成「什么都匹配不到」，上面那条空转通过就又回来了。
      final List<(String, FaintUse)> usages = sites
          .where(((String, FaintUse) s) => s.$1 != tokenDeclarationFile)
          .toList();
      expect(
        usages.length,
        greaterThanOrEqualTo(4),
        reason: '装饰用法（图标/手柄/开关拇指）至少有 4 处，扫到的太少说明判据坏了',
      );
    });

    test('没有把 contentFaint 传给任何 Text / TextStyle（**跨行写法也算**）', () {
      // 判据是**结构化**的（见文件头的 `findFaintTextUses`），不是同行正则：
      // 跨行的 `style: …\n copyWith(color: contentFaint)` 一样拦得住。
      final List<String> hits = <String>[];
      final Map<String, int> debt = <String, int>{};
      for (final (String path, FaintUse use) in allFaintUses()) {
        if (!use.isText) continue;
        if (kFaintTextDebtPaths.contains(path)) {
          debt[path] = (debt[path] ?? 0) + 1;
          continue;
        }
        hits.add('$path:${use.line}: ${use.snippet}  （判定依据：${use.textReason}）');
      }
      expect(
        hits,
        isEmpty,
        reason:
            '`contentFaint`（令牌自注「仅装饰/图标，**不得承载文字信息**」）'
            '被当文字色用了。承载文字的次要文本请用 `contentMuted`。\n'
            '${hits.join('\n')}',
      );
      for (final MapEntry<String, int> e in debt.entries) {
        expect(
          e.value,
          lessThanOrEqualTo(kFaintTextDebtCap),
          reason:
              '${e.key} 上的未迁移文字色债务长大了（${e.value} > $kFaintTextDebtCap）'
              '—— 债务只许缩小，不许新增。',
        );
      }
    });

    test('白名单对账：每处用法都必须是「非文字通道」，且逐条写明理由', () {
      final List<String> unknown = <String>[];
      final Set<String> used = <String>{};
      for (final (String path, FaintUse use) in allFaintUses()) {
        if (path == tokenDeclarationFile) continue; // 令牌定义处
        if (kFaintTextDebtPaths.contains(path)) continue; // 债务由上一条管
        final String key = '$path::${use.host}';
        used.add(key);
        if (kFaintForbiddenHosts.contains(use.host) ||
            !kFaintDecorationHosts.containsKey(key)) {
          unknown.add('$key:${use.line}: ${use.snippet}');
        }
      }
      expect(
        unknown,
        isEmpty,
        reason:
            '新增了 `contentFaint` 用法。它只允许出现在**非文字通道**'
            '（图标 / 描边 / 手柄 / 开关拇指…）：先确认宿主不是文字，再把 '
            '`<路径>::<宿主>` 连同理由加进 kFaintDecorationHosts。\n'
            '**已知陷阱宿主**（${kFaintForbiddenHosts.join('、')}）连白名单也不收：'
            '它的形参同时喂给标签字色。\n'
            '${unknown.join('\n')}',
      );
      expect(used, isNotEmpty, reason: '一处用法都没有 = 白名单在空转');
      for (final MapEntry<String, String> e in kFaintDecorationHosts.entries) {
        expect(
          kFaintForbiddenHosts.contains(e.key.split('::').last),
          isFalse,
          reason: '白名单里放了已知陷阱宿主 `${e.key}` —— 它的形参同时喂给文字通道',
        );
        expect(
          e.value.trim().length,
          greaterThan(8),
          reason: '白名单条目 `${e.key}` 的理由太短——「为什么不承载文字」要写清楚',
        );
      }
      // 未使用的白名单条目**不判红**：本轮有别的 worker 在并行迁移图标/文字色，
      // 要求「删了才绿」会让两边的树互相打红。收口时人工清（这里不打印噪音）。
    });

    test('合成用例①：跨行写法被拦下，**旧同行正则对它零命中**（判别力自证）', () {
      const String crossLine = '''
Widget _summary(BuildContext context) {
  return Text(
    widget.summary!,
    style: Theme.of(context).textTheme.labelSmall?.copyWith(
      color: colors.contentFaint,
    ),
  );
}
''';
      final List<FaintUse> hits = findFaintTextUses(crossLine);
      expect(hits, hasLength(1), reason: '跨行写法必须被拦下');
      expect(hits.single.textReason, contains('Text'));
      // 自证：旧判据（同行正则）对这段源码**零命中** —— 缺口真实存在过。
      expect(
        RegExp(
          r'(Text|TextStyle|style)[^\n]*contentFaint',
        ).hasMatch(stripCommentsAndStrings(crossLine)),
        isFalse,
        reason: '旧判据竟然能拦下跨行写法？那这条自证失去意义',
      );
    });

    test('合成用例②：装饰用法零命中（不误报图标 / 描边 / 手柄）', () {
      const String decorations = '''
Widget _row() {
  return Row(
    children: <Widget>[
      Icon(Icons.drag_indicator, size: 16, color: colors.contentFaint),
      Divider(color: colors.contentFaint),
      const SizedBox(width: 4),
    ],
  );
}
''';
      expect(findFaintTextUses(decorations), isEmpty);
    });

    test('合成用例③：兄弟实参的 `style:` 不算（结构判定不越界）', () {
      // 只看「向上所有调用」会把兄弟实参的 `style:` 算进来 —— 所以判据取的是
      // 「自己所在的那一段实参」。这条防的就是那个误报。
      const String sibling = '''
Widget _w() {
  return MyWidget(
    leading: Text('标签', style: t.bodySmall),
    trailing: Icon(Icons.x, color: colors.contentFaint),
  );
}
''';
      expect(findFaintTextUses(sibling), isEmpty);
    });

    test('合成用例④：宿主是 Text 时省略 `style:` 也算（位置实参）', () {
      const String positional = '''
Widget _t() => Text('正文', t.bodySmall!.copyWith(color: colors.contentFaint));
''';
      final List<FaintUse> hits = findFaintTextUses(positional);
      expect(hits, hasLength(1));
      expect(hits.single.textReason, contains('Text'));
    });

    test('合成用例⑤：`contentFaint` 在注释/字符串里不算（防自己把自己判红）', () {
      const String noise = '''
// Text(style: colors.contentFaint)  ← 注释里提到不算
const String note = 'style: contentFaint';
''';
      expect(findFaintUses(noise), isEmpty);
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
      // 2026-09-27：背景库的数据类型。它自己零 import（下面那条递归断言会验），
      // 所以放行不放宽「纯逻辑」这个前提。
      "import '../design/background_item.dart';",
    };

    test('这三个文件只 import dart:*（或零依赖的本地文件）', () {
      for (final String path in pureDartOnly) {
        // 库 + 它的全部 part：`display_prefs.dart` 2026-10-06 拆了 5 个 part，
        // 只读库文件就少扫 5 个文件的 import（part 文件本身不许有 import，
        // 所以扫描面取并集在语义上只更强、不放宽）。
        final String source = readLibrarySource(path);
        final List<String> imports = RegExp(
          r"^import .*$",
          multiLine: true,
        ).allMatches(source).map((RegExpMatch m) => m.group(0)!).toList();
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
        reason:
            '${hits.join('\n')}\n'
            '（`package:web` 让整个文件在 flutter test 里加载不了；'
            '如果能抽出纯逻辑，就该抽出来）',
      );
    });
  });

  group('性能：舞台必须被 RepaintBoundary 包住', () {
    test('`StageHost.build` 的返回值就是 RepaintBoundary（30 Hz 口型不拖累聊天列表）', () {
      // 2026-09-28（F-0005-1）：这条守卫从前的断言对象是
      // `lib/app/app_shell.dart`，判据是全文 `contains('RepaintBoundary')`——
      // 而那个文件里**唯一**的命中是一句说「不要直接铺 RepaintBoundary」的
      // 注释 ⇒ 断言恒真，删掉真正的保护也照样绿（同一个文件里：
      // `test/semantics_test.dart` 还抄了一份一模一样的空转断言）。
      //
      // 真正的包裹在 `lib/ui/stage_host.dart` 的 `build` 返回值上。这里改成：
      // ① 扫那个文件；② **先剥注释与字符串**（复用 `design_tokens_test.dart`
      // 的词法器，本文件其它红线也用它）；③ 锚定在 `build` 的 return 上，
      // 而不是「全文出现过这个词」——后者在任何别处出现都会假绿。
      final String source = stripCommentsAndStrings(
        File('lib/ui/stage_host.dart').readAsStringSync(),
      );
      expect(
        RegExp(
          r'Widget build\(BuildContext context\)\s*\{\s*return RepaintBoundary\(',
        ).hasMatch(source),
        isTrue,
        reason:
            '舞台宿主的 build 不再直接返回 RepaintBoundary：30 Hz 的口型重绘'
            '会拖累整棵聊天列表（判据锚定 build 的 return，不是全文 contains）。',
      );
    });
  });
}
