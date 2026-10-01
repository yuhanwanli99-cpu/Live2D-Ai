/// **运行态文本来源的字体子集门禁**（2026-09-28，F-0005-5）。
///
/// # 为什么另开一支文件（而不是塞进 `font_subset_test.dart`）
///
/// `font_subset_test.dart` 的门禁**只扫 `lib/**/*.dart` 的源码字符串字面量**。
/// 可界面上的字并不都来自源码：模型输出、后端 WS `error` 帧的 message/hint、
/// `GET /settings` 的文案、Mod 的 `settings_spec` 标签、第三方 Mod 上报的运行态值
/// ——它们**运行时**才产生，源码扫描对它们**一个字符都看不到**。缺字后果与源码
/// 那条一样：CanvasKit 去 `fonts.gstatic.com` 取回退字体，**有网时看不出来，
/// 断网即豆腐块**（= 项目断网红线的原始故障形态）。
///
/// 本文件做两件事（判据/覆盖表/词法器从 `font_subset_test.dart` 复用，
/// 两处各写一份迟早会漂移）：
///
/// 1. **清单**：[kRuntimeTextSources] 逐条登记「谁产生 / 谁上屏 / 可不可自动化」。
///    `manual` 的条目**必须**写清缺什么前提——结构上断言，**不许静默省略**；
///    每条来源的载体锚点会被断言（文件在 + 标识符还在），清单不会随代码搬家腐烂。
/// 2. **可失败的检查**：文案在**本仓 Rust 源码**里的那几族（WS error 帧 /
///    settings / web_api / Mod `settings_spec`）真扫一遍；并有一条「构造含子集外
///    字符的**运行态来源**必须红」的判别力自证（把门禁改成空转 → 测试红）。
///
/// # 已知缺口（清单里逐条写了前提，这里只放结论）
///
/// 静态扫描**结构上**覆盖不了「运行时才拼出来的文本」：模型输出、语音转写、
/// 用户填的文件名/键名、第三方 Mod 的运行态值。要真正关掉这条缺口，缺的是
/// **运行期策略**（自托管兜底字体 / 引擎侧关回退 / 渲染前替换），不是再多一条扫描
/// ——当前仓库 `fontFamilyFallback` 零命中 = 没有兜底，所以这几类只能真机验。
///
/// # 另一条前提（CI 接线，**已由 task-17 补齐**）
///
/// 本文件的 Rust 扫描要求「整仓检出」（读 `../../crates/`）。而
/// `.github/workflows/flutter-checks.yml` 的 `paths:` 原本只匹配
/// `shell/flutter/**` ⇒ **只改后端文案的 PR 不会触发这条门禁**，要等下一次前端
/// 改动才暴露。**task-17（D0-fix）已把 `crates/*/src/**` 加进 paths**，这条缺口
/// 闭合（`*` 通配连「新加 Mod crate」的那次 PR 也会触发）。若哪天那条 paths 被
/// 删回去，本文件的扫描本身仍然有效，只是又退回「等下次前端改动」的触发时机。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'font_subset_test.dart'
    show
        SubsetRanges,
        collectOffenders,
        loadSubsets,
        stringLiterals,
        subsetMisses;

// ─────────────────────────────────────────────────────────────────────────────
// 运行态文本来源（2026-09-28，F-0005-5）
//
// `font_subset_test.dart` 的 ③ 只扫 `lib/**/*.dart` 的**源码字符串字面量**。可界面上的字并不都来自
// 源码：模型输出、后端 WS `error` 帧的 message/hint、`GET /settings` 的文案、
// Mod 的 `settings_spec` 标签、第三方 Mod 上报的运行态值——它们**运行时**才产生，
// CanvasKit 遇到子集外码点照样去 `fonts.gstatic.com` 取回退字体（断网即豆腐块）。
//
// 这一节做两件事：
//   1. **清单**：[kRuntimeTextSources] 逐条登记「谁产生 / 谁上屏 / 可不可自动化」，
//      `manual` 的条目**必须**写清缺什么前提（结构上断言，不许静默省略）；
//   2. **可失败的检查**：可静态扫的后端文案真扫一遍（红/绿都走同一条判据），
//      并有一条「构造含子集外字符的运行态来源**必须红**」的判别力自证。
// ─────────────────────────────────────────────────────────────────────────────

/// 一条运行态文本来源的**覆盖面**。
enum RuntimeCoverage {
  /// 构建期可扫：来源在**本仓文件**里（Rust 文案）。本文件有提取器 + 真扫断言。
  scanned,

  /// 构建期扫不到：运行时才产生（模型输出 / 用户输入 / 第三方 Mod）。
  /// 这类条目**必须**写清 [RuntimeTextSource.precondition]。
  manual,
}

/// [kRuntimeTextSources] 的一条。
class RuntimeTextSource {
  const RuntimeTextSource({
    required this.id,
    required this.origin,
    required this.carrier,
    required this.coverage,
    this.scanRoots = const <String>[],
    this.precondition,
  });

  /// 稳定的来源标识（失败信息里会打出来，指回是哪条来源漏了）。
  final String id;

  /// 谁**产生**这段文本。
  final String origin;

  /// 谁把它**送上屏**：`<相对包根的路径>::<锚点标识符>`。
  /// 锚点会被断言（文件在 + 标识符还在）——清单不会随代码搬家而腐烂。
  final String carrier;

  final RuntimeCoverage coverage;

  /// [RuntimeCoverage.scanned] 时：要扫的仓内路径（文件或目录），相对包根。
  final List<String> scanRoots;

  /// [RuntimeCoverage.manual] 时：**自动化缺什么前提**（不许留空）。
  final String? precondition;

  String get anchorPath => carrier.split('::').first;
  String get anchorToken => carrier.split('::').last;
}

/// **运行态文本来源清单**（F-0005-5 要求的那张表）。
///
/// 判据不是「能不能写测试」，而是「这段文本会不会进 Flutter 的文字树」——
/// 进了文字树就归自托管字体管，就要被子集覆盖。
const List<RuntimeTextSource> kRuntimeTextSources = <RuntimeTextSource>[
  // ── 可静态扫：文案在**本仓** Rust 源码里 ──
  RuntimeTextSource(
    id: 'backend.ws_error_frame',
    origin: 'Rust `ErrorKind::message()/hint()` → WS `error` 帧的 message/hint',
    carrier: 'lib/ui/inline_notice.dart::InlineNotice',
    coverage: RuntimeCoverage.scanned,
    scanRoots: <String>['../../crates/live2d-ai-runtime/src/conversation'],
  ),
  RuntimeTextSource(
    id: 'backend.settings_view',
    origin: '`GET /api/v1/settings` 的展示文案（settings/view.rs 序列化出去）',
    carrier: 'lib/settings/settings_controller.dart::SettingsController',
    coverage: RuntimeCoverage.scanned,
    scanRoots: <String>['../../crates/live2d-ai-runtime/src/settings'],
  ),
  RuntimeTextSource(
    id: 'backend.web_api_json',
    origin: 'web_api 路由的 JSON 错误文案（前端 toast / 横幅直接显示）',
    carrier: 'lib/api/api_client.dart::ApiException',
    coverage: RuntimeCoverage.scanned,
    scanRoots: <String>['../../crates/live2d-ai-desktop/src/web_api'],
  ),
  RuntimeTextSource(
    id: 'backend.mod_settings_spec',
    origin: 'Mod 静态 `settings_spec` 的 label/description/placeholder（设置表单里上屏）',
    carrier: 'lib/settings/sections/dev_tools_section.dart::ModsSection',
    coverage: RuntimeCoverage.scanned,
    // **逐个点名**而不是扫整个 `crates/`：只扫「文案真的会上屏」的那些 crate，
    // 免得核心层的日志文案把这条门禁变成噪音（与「豁免面越小越难被侵蚀」同一条
    // 取舍）。漏没漏有对账：`每个 live2d-ai-mod-*/src 都在覆盖面内` 那条测试——
    // 新加 Mod crate 会让它对红，逼清单跟进。
    scanRoots: <String>[
      '../../crates/live2d-ai-mod-director/src',
      '../../crates/live2d-ai-mod-external-input/src',
      '../../crates/live2d-ai-mod-local-llm/src',
      '../../crates/live2d-ai-mod-memory/src',
      '../../crates/live2d-ai-mod-persona/src',
      '../../crates/live2d-ai-mod-pet-desktop/src',
      '../../crates/live2d-ai-mod-system/src',
      '../../crates/live2d-ai-mod-template/src',
      '../../crates/live2d-ai-mod-voice-input/src',
      '../../crates/live2d-ai-mod-wallpaper/src',
      '../../crates/live2d-ai-desktop/src/mod_registry.rs',
    ],
  ),

  // ── 扫不到：运行时才产生（每条写清缺什么前提） ──
  RuntimeTextSource(
    id: 'llm.text_delta',
    origin: '模型输出的正文（ChatController 逐 delta 落进气泡）',
    carrier: 'lib/ui/message_bubble.dart::_MessageBody',
    coverage: RuntimeCoverage.manual,
    precondition:
        '缺「样本语料 + 回放」装置：模型输出不可预测（emoji / CJK 扩展 / 生僻符号），'
        '构建期无从枚举。要自动化必须先有录制回放——把真实回合的 text_delta 存成 '
        'fixture，并约定语料必须覆盖 emoji 与 CJK 扩展区；在那之前只能真机起服务、'
        '让模型回一句含 emoji 的话，在 Network 面板过滤 gstatic.com（有请求即成立）。',
  ),
  RuntimeTextSource(
    id: 'llm.session_title',
    origin: '会话标题 = 模型首句（落盘后回显在会话列表）',
    carrier: 'lib/ui/session_sheet.dart::SessionSheet',
    coverage: RuntimeCoverage.manual,
    precondition: '与 llm.text_delta 同源（同一段模型文本截断），缺的是同一套录制回放语料。',
  ),
  RuntimeTextSource(
    id: 'asr.transcript',
    origin: 'WebSpeech 语音转写（用户设备产生的任意文本，含 emoji）',
    carrier: 'lib/voice/voice_listen_controller.dart::VoiceListenController',
    coverage: RuntimeCoverage.manual,
    precondition:
        '缺「运行期兜底字体/字形替换策略」：转写来自用户设备，构建期不可能枚举。'
        '要自动化得先有一条产品级策略（自托管兜底字体并显式声明，或渲染前对子集外'
        '码点做替换），然后才有东西可断言；当前仓库 fontFamilyFallback 零命中 = 无兜底。',
  ),
  RuntimeTextSource(
    id: 'user.background_filename',
    origin: '用户选的背景图文件名（入库后回显在背景库列表）',
    carrier: 'lib/app/browser_io.dart::pickImageDataUrl',
    coverage: RuntimeCoverage.manual,
    precondition:
        '用户输入不可枚举；需要先定「文件名回显要不要过滤/截断」的产品口径，'
        '并在那个纯函数上做子集外码点的替换断言。',
  ),
  RuntimeTextSource(
    id: 'user.env_key_name',
    origin: '用户在设置里填的 `.env` 键名（保存后回显在诊断/设置面板）',
    carrier: 'lib/settings/sections/env_key_field.dart::EnvKeyField',
    coverage: RuntimeCoverage.manual,
    precondition:
        '用户输入不可枚举；需要先定「键名是否限制为 ASCII 标识符」的校验口径，'
        '再对该校验函数做断言（当前只在界面上提示，没有硬校验可测）。',
  ),
  RuntimeTextSource(
    id: 'mod.runtime_value',
    origin: '第三方 Mod 上报的运行态值（`state_json` 里的值，经 formatModStateValue 上屏）',
    carrier: 'lib/settings/sections/dev_tools_section.dart::formatModStateValue',
    coverage: RuntimeCoverage.manual,
    precondition:
        '插件代码不在本仓，构建期无从枚举（本仓的 5 个 Mod 已被 backend.mod_settings_spec '
        '覆盖，但社区 Mod 的 state 值不在）。要自动化需要先在 Mod 契约侧声明「运行态值'
        '的字符白名单」或「只能是数据而非文案」，否则只能等前端加运行期兜底。',
  ),
];

/// Rust 测试源码？（`tests/` 目录、`tests.rs`、`tests_*.rs`、`*_tests.rs`）
///
/// 排除理由：那里的字符串只进断言失败信息，不会上屏，不属于运行态文本。
bool isRustTestSource(String path) {
  final String base = path.split('/').last;
  return path.contains('/tests/') ||
      base == 'tests.rs' ||
      base.startsWith('tests_') ||
      base.endsWith('_tests.rs');
}

/// 列出一个扫描根（文件或目录）下的**产品** Rust 源码。
List<File> rustSourcesUnder(String root) {
  if (FileSystemEntity.typeSync(root) == FileSystemEntityType.file) {
    return <File>[File(root)];
  }
  return Directory(root)
      .listSync(recursive: true)
      .whereType<File>()
      .where((File f) => f.path.endsWith('.rs') && !isRustTestSource(f.path))
      .toList();
}

/// 从 **Rust** 源码里抽出字符串字面量（跳过注释与字符字面量）。
///
/// 与 Dart 那支 [stringLiterals] 的三处**刻意差异**：
///   1. 只认 `"` 界定的字符串。Rust 的字符字面量是 `'…'`（还有生命周期 `'a`），
///      而 `clean_transcript` 里的 `'\u{200B}' | '\u{FEFF}'` 是**清洗字符类**，
///      不是上屏文案——把字符字面量算进来会把它误判成缺字；
///   2. 支持 `r"…"` / `r#"…"#` 原始字符串（Rust 的常见写法）；
///   3. 解 Rust 形态的转义：`\u{XXXX}`（没有 `\uXXXX`）与 `\xHH`。
List<String> rustStringLiterals(String src) {
  final List<String> out = <String>[];
  int i = 0;
  final int n = src.length;

  int decodeEscape(int at, StringBuffer buf) {
    // 返回「消耗掉的字符数」；buf 里写入解出的字符。
    if (at + 1 >= n) return 1;
    final String c = src[at + 1];
    if (c == 'u' && at + 2 < n && src[at + 2] == '{') {
      final int close = src.indexOf('}', at + 3);
      if (close > 0) {
        final int? cp = int.tryParse(
          src.substring(at + 3, close),
          radix: 16,
        );
        if (cp != null) {
          buf.writeCharCode(cp);
          return close - at + 1;
        }
      }
      return 2;
    }
    if (c == 'x' && at + 3 < n) {
      final int? b = int.tryParse(src.substring(at + 2, at + 4), radix: 16);
      if (b != null) {
        buf.writeCharCode(b);
        return 4;
      }
      return 2;
    }
    switch (c) {
      case 'n':
        buf.write('\n');
      case 't':
        buf.write('\t');
      case 'r':
        buf.write('\r');
      case '0':
        buf.write('\x00');
      default:
        buf.write(c);
    }
    return 2;
  }

  while (i < n) {
    // 行注释
    if (src[i] == '/' && i + 1 < n && src[i + 1] == '/') {
      while (i < n && src[i] != '\n') {
        i++;
      }
      continue;
    }
    // 块注释（Rust 可嵌套）
    if (src[i] == '/' && i + 1 < n && src[i + 1] == '*') {
      int depth = 1;
      i += 2;
      while (i < n && depth > 0) {
        if (src.startsWith('/*', i)) {
          depth++;
          i += 2;
        } else if (src.startsWith('*/', i)) {
          depth--;
          i += 2;
        } else {
          i++;
        }
      }
      continue;
    }
    // 字符字面量：**整段跳过**（`'x'` / `'\u{200B}'` / 生命周期 `'a`）
    if (src[i] == "'") {
      i++;
      while (i < n && src[i] != "'" && src[i] != '\n') {
        if (src[i] == r'\') i++;
        i++;
      }
      i++;
      continue;
    }
    // 原始字符串：r"…" / r#"…"# / r##"…"##（字节串 br"…" 同理）
    if (src[i] == 'r' &&
            i + 1 < n &&
            (src[i + 1] == '"' || src[i + 1] == '#') ||
        src.startsWith('br"', i) ||
        src.startsWith('br#', i)) {
      int j = src.startsWith('br', i) ? i + 2 : i + 1;
      int hashes = 0;
      while (j < n && src[j] == '#') {
        hashes++;
        j++;
      }
      if (j < n && src[j] == '"') {
        j++;
        final String end = '"${'#' * hashes}';
        final int close = src.indexOf(end, j);
        if (close < 0) break;
        out.add(src.substring(j, close));
        i = close + end.length;
        continue;
      }
    }
    // 普通字符串 / 字节串
    final int quoteAt = src.startsWith('b"', i) ? i + 1 : i;
    if (quoteAt < n && src[quoteAt] == '"') {
      int j = quoteAt + 1;
      final StringBuffer buf = StringBuffer();
      while (j < n) {
        if (src[j] == r'\') {
          j += decodeEscape(j, buf);
          continue;
        }
        if (src[j] == '"') break;
        buf.write(src[j]);
        j++;
      }
      out.add(buf.toString());
      i = j + 1;
      continue;
    }
    i++;
  }
  return out;
}

void main() {
  group('④ Rust 词法器（后端文案扫描的门禁自身钉子）', () {
    test('注释里的码点不算', () {
      const String src = '''
// "注释里的 ▍"
/* "块注释里的 ⌘" */
let s = "正文";
''';
      final List<String> lits = rustStringLiterals(src);
      expect(lits, contains('正文'));
      expect(lits.any((String s) => s.contains('▍')), isFalse);
      expect(lits.any((String s) => s.contains('⌘')), isFalse);
    });

    test('**字符字面量整段跳过**（清洗字符类不是上屏文案）', () {
      // 这是真实源码里的形态（`crates/live2d-ai-mod-voice-input/src/lib.rs`）：
      // `matches!(ch, '\u{200B}' | '\u{FEFF}' | …)` 是清洗逻辑，零宽字符
      // 不在子集里；若把字符字面量当文案，这条扫描会**误红**。
      const String src = r'''
let skip = matches!(ch, '\u{200B}' | '\u{FEFF}' | '\u{200C}');
let msg = "监听端口";
''';
      final List<String> lits = rustStringLiterals(src);
      expect(lits, <String>['监听端口']);
      expect(subsetMisses(lits.single, loadSubsets()), isEmpty);
    });

    test('原始字符串与 `\\u{…}` / `\\xHH` 转义都解得出来', () {
      expect(
        rustStringLiterals(r'let a = r#"原样\n"#;'),
        contains(r'原样\n'),
      );
      expect(
        rustStringLiterals(r'let b = "\u{258D}";').single.runes.toList(),
        <int>[0x258D],
      );
      expect(rustStringLiterals(r'let c = "\x41BC";').single, 'ABC');
    });

    test('真实后端源码里确实抽得到大量非 ASCII 字面量（防「零命中=通过」）', () {
      // 提取器坏了（例如把整文件当注释）会让扫描零样本→假绿。
      int nonAscii = 0;
      for (final RuntimeTextSource src in kRuntimeTextSources.where(
        (RuntimeTextSource s) => s.coverage == RuntimeCoverage.scanned,
      )) {
        for (final String root in src.scanRoots) {
          for (final File f in rustSourcesUnder(root)) {
            for (final String lit in rustStringLiterals(
              f.readAsStringSync(),
            )) {
              if (lit.runes.any((int c) => c >= 0x80)) nonAscii++;
            }
          }
        }
      }
      expect(
        nonAscii,
        greaterThan(200),
        reason: '只抽到 $nonAscii 条非 ASCII 后端字面量 —— 词法器可能坏了',
      );
    });
  });

  group('⑤ 运行态文本来源清单（F-0005-5：不许静默省略）', () {
    test('清单非空，且每条都声明了「可自动化」或「缺什么前提」', () {
      expect(kRuntimeTextSources.length, greaterThanOrEqualTo(8));
      final Set<String> ids = <String>{};
      for (final RuntimeTextSource src in kRuntimeTextSources) {
        expect(src.id, isNotEmpty);
        expect(ids.add(src.id), isTrue, reason: 'id 重复：${src.id}');
        expect(src.origin, isNotEmpty, reason: '${src.id} 没写谁产生它');
        expect(
          src.carrier,
          contains('::'),
          reason: '${src.id} 的载体要写成 `<路径>::<锚点>`',
        );
        if (src.coverage == RuntimeCoverage.scanned) {
          expect(src.scanRoots, isNotEmpty, reason: '${src.id} 说可扫却没给扫描根');
        } else {
          expect(
            src.precondition,
            isNotNull,
            reason: '${src.id} 扫不到又不说缺什么前提 = 静默省略',
          );
          expect(
            src.precondition!.trim().length,
            greaterThan(20),
            reason: '${src.id} 的前提写得太短，等于没说',
          );
        }
      }
    });

    test('每条来源的**载体锚点**都还在（清单不会随代码搬家而腐烂）', () {
      for (final RuntimeTextSource src in kRuntimeTextSources) {
        final File file = File(src.anchorPath);
        expect(
          file.existsSync(),
          isTrue,
          reason: '${src.id} 的载体文件不见了：${src.anchorPath}',
        );
        expect(
          file.readAsStringSync(),
          contains(src.anchorToken),
          reason:
              '${src.id} 的载体锚点 `‎${src.anchorToken}` 在 '
              '${src.anchorPath} 里找不到了 —— 清单要跟着改（别静默留一条死条目）',
        );
      }
    });

    test('每个 `live2d-ai-mod-*/src` 都在覆盖面内（新 Mod crate 逃不掉）', () {
      final Set<String> covered = <String>{
        for (final RuntimeTextSource s in kRuntimeTextSources)
          if (s.coverage == RuntimeCoverage.scanned)
            for (final String root in s.scanRoots) root.replaceAll('//', '/'),
      };
      final List<String> crates = Directory('../../crates')
          .listSync()
          .whereType<Directory>()
          .map((Directory d) => d.path)
          .where(
            (String p) => p.split('/').last.startsWith('live2d-ai-mod-'),
          )
          .toList();
      expect(crates, isNotEmpty, reason: '一个 Mod crate 都没找到，路径写错了');
      for (final String crate in crates) {
        final String src = '$crate/src';
        if (!Directory(src).existsSync()) continue;
        // 归一化：`../../crates/x/src` 与 `crates/x/src` 都算覆盖。
        final bool ok = covered.any(
          (String root) => root.endsWith('${crate.replaceFirst('../', '')}/src'),
        );
        expect(
          ok,
          isTrue,
          reason:
              'Mod crate `$crate` 不在运行态来源清单的覆盖面内 —— 它的 '
              '`settings_spec` 文案会上屏，缺字同样会拉 gstatic。请把它加进 '
              'backend.mod_settings_spec 的 scanRoots（并在这里保持对账）。',
        );
      }
    });

    test('清单里点名了审计的三类运行态来源（模型输出 / 后端文案 / Mod 运行态值）', () {
      final Set<String> ids = kRuntimeTextSources
          .map((RuntimeTextSource s) => s.id)
          .toSet();
      expect(ids, contains('llm.text_delta'), reason: '模型输出');
      expect(ids, contains('backend.ws_error_frame'), reason: '后端文案');
      expect(ids, contains('mod.runtime_value'), reason: 'Mod 运行态值');
    });
  });

  group('⑥ 后端文案（可静态扫的运行态来源）必须被自托管子集覆盖', () {
    test('crates/ 下上屏的那几族 Rust 字面量全部在子集内', () {
      final List<SubsetRanges> subsets = loadSubsets();
      final List<(String, String)> samples = <(String, String)>[];
      final List<RuntimeTextSource> scanned = kRuntimeTextSources
          .where((RuntimeTextSource s) => s.coverage == RuntimeCoverage.scanned)
          .toList();
      expect(scanned, isNotEmpty, reason: '一条可扫来源都没有 = 覆盖面为零');

      for (final RuntimeTextSource src in scanned) {
        for (final String root in src.scanRoots) {
          expect(
            FileSystemEntity.typeSync(root),
            isNot(FileSystemEntityType.notFound),
            reason:
                '${src.id} 的扫描根不存在：$root。\n'
                '前提是「整仓检出」——`flutter test` 的 CWD 是 `shell/flutter/`，'
                '所以要能读到 `../crates/`。单独拷 shell/flutter/ 跑测试会在这里红，'
                '这是**刻意**的：宁可红，也不静默跳过一整族运行态来源。',
          );
          for (final File f in rustSourcesUnder(root)) {
            for (final String lit in rustStringLiterals(
              f.readAsStringSync(),
            )) {
              samples.add((src.id, lit));
            }
          }
        }
      }

      // 非空性：扫描根写错 / 提取器坏了 → 零样本 → 「零命中=通过」的假绿。
      expect(
        samples.length,
        greaterThan(2000),
        reason: '只扫到 ${samples.length} 条后端字面量，扫描根可能写错了',
      );
      final int nonAscii = samples
          .where(
            ((String, String) s) => s.$2.runes.any((int c) => c >= 0x80),
          )
          .length;
      expect(
        nonAscii,
        greaterThan(400),
        reason: '非 ASCII 后端字面量只有 $nonAscii 条 —— 提取器可能坏了',
      );

      final Map<int, Set<String>> offenders = collectOffenders(samples, subsets);
      final String detail = offenders.entries
          .map(
            (MapEntry<int, Set<String>> e) =>
                '  ${String.fromCharCode(e.key)}  U+${e.key.toRadixString(16).toUpperCase().padLeft(4, '0')}'
                '  ← 来源：${e.value.join(', ')}',
          )
          .join('\n');
      expect(
        offenders,
        isEmpty,
        reason:
            '后端文案里有字符不在自托管字体子集内。它们经 WS error 帧 / settings\n'
            'JSON / Mod settings_spec **运行时**上屏，CanvasKit 会去 fonts.gstatic.com\n'
            '拉回退字体 —— 有网时看不出来，断网就是豆腐块。\n'
            '修法：① 文案改成子集内的字符；② 或加进子集（重新子集化后跑\n'
            'python3 scripts/font_subset_ranges.py）。\n违规字符：\n$detail',
      );
    });
  });

  group('⑦ 判别力自证：构造含子集外字符的**运行态来源**必须红', () {
    test('校准位：emoji / `▍` / CJK 扩展 B 确实不在子集里', () {
      final List<SubsetRanges> subsets = loadSubsets();
      for (final int cp in <int>[0x1F389, 0x258D, 0x20000]) {
        expect(
          subsets.every((SubsetRanges r) => r.covers(cp)),
          isFalse,
          reason: 'U+${cp.toRadixString(16)} 已进子集 → 判别力自证的样例会失去意义',
        );
      }
    });

    test('干净的运行态样本 → 绿', () {
      final List<SubsetRanges> subsets = loadSubsets();
      final Map<int, Set<String>> offenders = collectOffenders(<(String, String)>[
        ('backend.ws_error_frame', '上游非成功状态 401，请检查密钥'),
        ('backend.mod_settings_spec', '监听端口（提示；实际端口见 live2d-ai.toml [web].port）'),
        ('llm.text_delta', '你好，世界。'),
      ], subsets);
      expect(offenders, isEmpty);
    });

    test('把**运行态**那段换成子集外字符 → 必须红，且指回是哪条来源', () {
      final List<SubsetRanges> subsets = loadSubsets();
      // 这三条覆盖审计点名的三类：模型输出 / 后端文案 / Mod 运行态值。
      final Map<int, Set<String>> offenders = collectOffenders(<(String, String)>[
        ('llm.text_delta', '你好，世界 🎉'),
        ('backend.ws_error_frame', '这一句没有收尾 ▍'),
        ('mod.runtime_value', '𠀀'),
      ], subsets);

      expect(
        offenders.keys.toSet(),
        containsAll(<int>{0x1F389, 0x258D, 0x20000}),
        reason: '构造的运行态来源必须被判红（否则这条门禁是空转的）',
      );
      expect(offenders[0x1F389], contains('llm.text_delta'));
      expect(offenders[0x258D], contains('backend.ws_error_frame'));
      expect(offenders[0x20000], contains('mod.runtime_value'));
    });

    test('同一段文本走 `lib/**` 源码扫描**不会**红（正是缺口的形状）', () {
      // 缺口的本质：源码字面量扫描只看得到**字面量**。运行时拼出来的文本
      // （这里是插值，真实路径上是模型输出 / WS frame）它一条都看不到。
      const String runtimeText = '你好，世界 🎉';
      final List<String> fromSource = stringLiterals(
        r'const String uiCopy = "$llmText";',
      );
      expect(
        fromSource,
        <String>[r'$llmText'],
        reason: 'Dart 词法器只看到插值占位（ASCII），看不到运行时取值',
      );
      final List<SubsetRanges> subsets = loadSubsets();
      expect(
        collectOffenders(<(String, String)>[
          ('lib/**/*.dart 源码扫描', fromSource.single),
        ], subsets),
        isEmpty,
        reason: '源码那半扫的是占位符 —— 所以它对运行态文本零覆盖',
      );
      expect(
        collectOffenders(<(String, String)>[
          ('llm.text_delta', runtimeText),
        ], subsets),
        isNotEmpty,
        reason: '同一段文本换成**运行态来源**就必须红 —— 这才是补齐的那一半',
      );
    });
  });
}
