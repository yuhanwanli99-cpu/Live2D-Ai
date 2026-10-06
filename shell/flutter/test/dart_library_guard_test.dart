/// 守卫不得静默漏扫：源码扫描面必须覆盖 part 文件。
///
/// # 为什么要有这条门禁（2026-10-06，E1 实测）
///
/// test/ 里有一批**源码扫描守卫**（接线守卫 / 结构守卫 / 不许复活守卫），
/// 判据都是「某段源码里有没有这个东西」。而 part 拆分把**同一段代码**搬到
/// 同库的另一个文件里：只读库文件本身，守卫读到的是一半，
/// **断言看起来照样绿**——这比「测试没写」更坏，因为它给人已经守住了的错觉。
///
/// 实测抓到的四处（本轮全部修掉）：
///
/// | 位置 | 漏了什么 |
/// | --- | --- |
/// | test/ 里 14 处 File('lib/main.dart').readAsStringSync() | main.dart 的 4 个 part 全漏 |
/// | display_prefs_test.appearanceSectionSource() 手写拼接 | appearance_section.dart 的 3 个 part 漏 2 个 |
/// | setting_wiring_test._codeOf('lib/app/app_shell.dart') | 漏 app_shell_state.dart |
/// | action_scales_wiring_test.readLib('lib/main.dart') | 漏 main.dart 的 part（E1 拆分后 3 条当场变红） |
///
/// 修法统一走 test/support/dart_library.dart 的 readLibrarySource()
/// （库 + 它的 parts，深度优先、去重）。本门禁保证**下次再加 part 时不会有人漏**。
///
/// **E15（2026-10-06）：豁免从「整文件放行」收紧为「逐调用点 + 豁免自证」。**
/// 旧写法只要文件里出现一次目录遍历就 `continue`，**全文所有**变量路径读取点一起免检
/// （`no_backdrop_filter_test.dart` 的直读点就是这么溜过去的）。现在见文件末尾的
/// `varReadExemptions`：逐调用点登记 + 必须写理由 + 文件里必须仍有机械锚点
/// （遍历 / 非 part 库字面量），否则**豁免自身判红**。
///
/// **已知盲区（如实标注）**：把 part 库的路径装进变量、再经变量读到读取点，
/// 若同文件里不出现该库的字面量路径，静态上判不出来。当前 test/ 无此写法。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'support/source_scan.dart';

/// lib/ 下声明了 part 的库（相对仓库根的 posix 路径）。
///
/// 先剥注释、**保留字符串字面量**：part 'x.dart'; 里的路径就是字符串，
/// 剥掉字符串就没法判；而注释里写一句 part '...' 不该被当成声明。
Set<String> partDeclaringLibraries() {
  final Set<String> out = <String>{};
  for (final FileSystemEntity e in Directory('lib').listSync(recursive: true)) {
    if (e is! File || !e.path.endsWith('.dart')) continue;
    final String src = stripCommentsKeepStrings(e.readAsStringSync());
    if (RegExp(r"^part\s+'", multiLine: true).hasMatch(src)) {
      out.add(e.path.replaceAll(r'\\', '/'));
    }
  }
  return out;
}

/// test/ 下的 Dart 源码（含 support/）。
List<File> testSources() => Directory('test')
    .listSync(recursive: true)
    .whereType<File>()
    .where((File f) => f.path.endsWith('.dart'))
    .toList();

void main() {
  test('扫描面确实覆盖到了 lib 下的 part 声明（防门禁空转）', () {
    final Set<String> libs = partDeclaringLibraries();
    expect(
      libs,
      isNotEmpty,
      reason:
          '一个 part 声明都没扫到 = 判据与源码形状脱节，这条门禁在空转。'
          '若 part 真的从 lib 下全部退役，应连同本文件一起删掉（那是好消息）。',
    );
  });

  test('没有任何 test 用 File(...) 直读声明了 part 的库', () {
    final Set<String> libs = partDeclaringLibraries();
    final List<String> offenders = <String>[];
    for (final File e in testSources()) {
      // 先剥注释、保留字符串：读取点的库路径本身是字符串字面量，
      // 而本文件头注里就写着违规写法（不剥注释就会自我判红）。
      final String src = stripCommentsKeepStrings(e.readAsStringSync());
      for (final String lib in libs) {
        final RegExp read = RegExp(
          r"File\(\s*'" + RegExp.escape(lib) + r"'\s*\)\s*\.readAsStringSync\(\)",
        );
        if (read.hasMatch(src)) {
          offenders.add('${e.path.replaceAll(r'\\', '/')} -> $lib');
        }
      }
    }
    expect(
      offenders,
      isEmpty,
      reason:
          '这些守卫只读库文件、漏掉它的 part，扫描面是一半而断言看起来还绿。'
          '改用 support/dart_library.dart 的 readLibrarySource(...)。当前命中：'
          '${offenders.join(' | ')}',
    );
  });

  test('没有「按路径读源码」的辅助函数 / 变量路径 + part 库字面量', () {
    // 实测先例：action_scales_wiring_test.dart 的
    //   String readLib(String rel) => File(rel).readAsStringSync();
    // 它读的正是 lib/main.dart，3 条断言在 E1 拆分后**当场变红**——
    // 不是产品回归，是守卫压根没看到 part 里的代码。
    //
    // 两条判据（都精确，不做数据流推断）：
    // ① 定义了「收一个路径参数、按路径读源码」的辅助函数；
    // ② 文件里既有变量路径读取、又出现 part 库的**字面量路径**
    //    （硬编码清单迟早把它喂进 File(path)）；但**目录递归遍历**是例外，
    //    遍历天然覆盖 part。
    final RegExp helperDef = RegExp(
      r"^[A-Za-z_][\w<>,?\s]*\s+[A-Za-z_]\w*\s*\([^)]*\)\s*=>\s*"
      r"File\(\s*[A-Za-z_]\w*\s*\)\s*\.readAsStringSync\(\)",
      multiLine: true,
    );
    final RegExp varRead = RegExp(
      r"File\(\s*[A-Za-z_]\w*\s*\)\s*\.readAsStringSync\(\)",
    );
    final Set<String> libs = partDeclaringLibraries();
    final List<String> offenders = <String>[];
    // 调用点 key → 出现次数（判「豁免是否失准」）与 → 所在文件源码（判锚点是否还在）。
    final Map<String, int> siteCounts = <String, int>{};
    final Map<String, String> siteSources = <String, String>{};
    int scanned = 0;
    for (final File e in testSources()) {
      if (e.path.endsWith('support/dart_library.dart')) continue; // 唯一实现点
      scanned++;
      final String src = stripCommentsKeepStrings(e.readAsStringSync());
      final String rel = e.path.replaceAll(r'\\', '/');
      if (helperDef.hasMatch(src)) {
        offenders.add('$rel -> 定义了按路径读源码的辅助函数');
        continue;
      }
      // 逐个**调用点**收集（E15：不再整文件放行）。
      for (final RegExpMatch m in varRead.allMatches(src)) {
        final String key = _callSiteKey(rel, m.group(0)!);
        siteCounts[key] = (siteCounts[key] ?? 0) + 1;
        siteSources[key] = src;
      }
    }
    // 判定一：白名单**失准**也要红 —— 每条豁免必须恰好命中一个真实调用点。
    final List<String> staleExemptions = <String>[];
    for (final VarReadExemption c in varReadExemptions) {
      final int hits = siteCounts[c.key] ?? 0;
      if (hits != 1) {
        staleExemptions.add('${c.key}（命中 $hits 次）');
      } else if (c.reason.trim().isEmpty) {
        staleExemptions.add('${c.key}（没写理由）');
      } else if (!_stillJustified(siteSources[c.key]!, libs)) {
        staleExemptions.add('${c.key}（文件里已找不到遍历 / 非 part 库字面量）');
      }
    }
    // 判定二：**未豁免**的调用点，若所在文件出现 part 库字面量路径 ⇒ 判红。
    for (final String key in siteCounts.keys) {
      if (varReadExemptions.any((VarReadExemption c) => c.key == key)) continue;
      if (libs.any((String lib) => siteSources[key]!.contains("'$lib'"))) {
        offenders.add('$key -> 变量路径读取 + 出现 part 库字面量路径');
      }
    }
    // 防零命中空转：扫到的文件太少 = 路径写错了。
    expect(scanned, greaterThan(30), reason: '扫到的测试文件太少，门禁在空转');
    expect(
      staleExemptions,
      isEmpty,
      reason:
          '这些豁免已经对不上真实调用点，或者文件里已找不到当初放行的依据 —— '
          '等于凭空放行。删掉它、或者把依据补回来。当前：'
          '${staleExemptions.join(' | ')}',
    );
    expect(
      offenders,
      isEmpty,
      reason:
          '路径经变量的读取，静态上判不出被读的是不是「声明了 part 的库」——'
          '今天读 main.dart，明天就可能读另一个。改用 support/dart_library.dart 的'
          'readLibrarySource(...)；确属遍历 / 硬编码清单的，逐调用点登记进'
          'varReadExemptions 并写明理由。当前命中：${offenders.join(' | ')}',
    );
  });
}

/// 逐个**调用点**的豁免（E15：不是整文件豁免）。
class VarReadExemption {
  const VarReadExemption(this.key, this.reason);

  /// `<相对仓库根的 posix 路径> :: <折叠空白后的调用表达式>`（行号不进 key：挪行不算改语义）。
  final String key;

  /// 为什么这个调用点不算漏扫（**必填**，空字符串判红）。
  final String reason;
}

/// 调用点 key：文件 + 折叠空白后的匹配文本。
String _callSiteKey(String rel, String expr) =>
    '$rel :: ${expr.replaceAll(RegExp(r"\s+"), ' ').trim()}';

/// 豁免**仍有依据**的机械锚点（必要条件，不是充分条件；充分性靠 [VarReadExemption.reason]
/// 供人复核）：文件里仍有目录递归遍历，或仍有一个**不声明 part** 的 `lib/...dart` 字面量。
bool _stillJustified(String src, Set<String> libs) {
  if (src.contains('listSync(recursive: true)')) return true;
  return RegExp(r"'(lib/[^']+\.dart)'")
      .allMatches(src)
      .any((RegExpMatch m) => !libs.contains(m.group(1)!));
}

/// 当前的四条豁免（每条都必须仍然对得上一个真实调用点，否则判红）。
///
/// 事实核对（2026-10-06 实测，`libs` = 声明 part 的库）：
/// - `lib/ui/shell_backdrop.dart`、`lib/live2d/render_events.dart`、
///   `lib/settings/sections/director_observer_section.dart` **都不声明 part**；
/// - `no_backdrop_filter_test.dart:586` 读的是 `zeroDependencyLocals` 里的本地文件路径；
/// - `design_tokens_lint_test.dart:499` 的路径来自 `_filesUnder(prefix)` 目录遍历。
const List<VarReadExemption> varReadExemptions = <VarReadExemption>[
  VarReadExemption(
    'test/no_backdrop_filter_test.dart :: File(blurExceptionFile).readAsStringSync()',
    'blurExceptionFile = lib/ui/shell_backdrop.dart（不声明 part）⇒ 读整文件就是读整个库',
  ),
  VarReadExemption(
    'test/no_backdrop_filter_test.dart :: File(path).readAsStringSync()',
    'path 由 zeroDependencyLocals 的本地文件路径拼成（lib/settings/ 下、不声明 part 的纯逻辑文件）',
  ),
  VarReadExemption(
    'test/director_observer_test.dart :: File(path).readAsStringSync()',
    'path 取自硬编码 guardedFiles（render_events.dart / director_observer_section.dart，均不声明 part）',
  ),
  VarReadExemption(
    'test/design_tokens_lint_test.dart :: File(path).readAsStringSync()',
    'path 来自 _filesUnder(prefix) 的目录递归遍历（遍历天然覆盖 part）',
  ),
];
