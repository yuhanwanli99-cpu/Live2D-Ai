/// A1 资产守护网（A3 接触点）：main.dart 的导演按句 cue **调用点**。
///
/// # 为什么新增（现有断言的零检验力缺口）
///
/// test/action_cue_test.dart:290 已有：
///
///     expect(main.contains('_applyDirectorCueForSeq'), isTrue);
///
/// 它只问「这个名字出现过没有」——而 lib/main.dart 里 _applyDirectorCueForSeq
/// **定义在 :591**（void _applyDirectorCueForSeq(int? seq) {），**调用点在
/// :481-483**（_sentenceCueSubscription = _audio.sentenceStarts.listen(...)）。
/// 把整个调用点删掉、只留下定义，旧断言**照样全绿**——于是「导演 cue 被订阅到
/// 音频按句流」这条接线可以在没有任何红灯的情况下消失（重放覆盖 main.dart
/// 时最可能发生的正是这个）。本文件补的就是这个缺口。
///
/// # 断言类型：窄而精确的**结构**守卫（编排者 2026-09-27 裁决 (b)）
///
/// main.dart 经 app/browser_io.dart 依赖 package:web，在 flutter test
/// （Dart VM）里**加载不了**；Live2DStageState._bridge 又只能由 **web-only**
/// 的 buildLive2DHost(onTransport:) 挂上（VM 走 live2d_host_stub.dart，
/// 从不回调）。所以「cue 被订阅到音频流」在单元级**没有可达的行为路径**。
/// 这里退到结构守卫，但只认**调用语法**：_audio.sentenceStarts.listen( 之后
/// 紧跟 _applyDirectorCueForSeq。**不扫文案、不扫颜色字面量、不认裸名字。**
///
/// # 可证伪性（编码进本文件，不靠嘴说）
///
/// 编排者的验收手法是「把 :481-483 注释掉 → 本断言必须变红」。下面的
/// 判别力自证用例把这一步内建：同一段文本「未注释 → 命中、注释掉 → 不命中」，
/// 以及「只剩定义 → 不命中」。若有人把它退回成裸 contains(名字)，
/// 自证用例会先红。
library;

import 'package:flutter_test/flutter_test.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

/// 剥掉注释与字符串（共享词法器：`support/source_scan.dart`，W3-D3 起唯一定义）。
///
/// 剥注释是这条判据的**关键**：调用点被 // 注释掉之后，判据必须不再命中。
/// 接线调用点判据。
///
/// **为什么不是** contains('_applyDirectorCueForSeq')：定义
/// void _applyDirectorCueForSeq(int? seq) { 也含这个名字。本正则要求它出现在
/// _audio.sentenceStarts.listen( 的**实参位置**：定义不匹配、被注释掉不匹配、
/// 改名不匹配、换个流也不匹配。
final RegExp kDirectorCueCallSite = RegExp(
  r'_audio\s*\.\s*sentenceStarts\s*\.\s*listen\s*\(\s*_applyDirectorCueForSeq\b',
);

void main() {
  final String mainSource = stripCommentsAndStrings(
    readLibrarySource('lib/main.dart'),
  );

  test('结构：main.dart 把 _applyDirectorCueForSeq **调用**订到音频按句流上', () {
    expect(
      kDirectorCueCallSite.hasMatch(mainSource),
      isTrue,
      reason:
          '导演 cue 的唯一锚点是「该句音频开始播放」——'
          '_audio.sentenceStarts.listen(_applyDirectorCueForSeq) 这一行没了，'
          '动作包会在正确的时间点静默不下发（表演层断线而单测仍可能全绿）。'
          '这是 §3.3 main.dart（A3）的守护断言，类型 = 结构。',
    );
  });

  test('判别力自证：调用点未注释 → 命中；注释掉 → 不命中', () {
    const String callSite = '''
      _sentenceCueSubscription = _audio.sentenceStarts.listen(
        _applyDirectorCueForSeq,
      );
      void _applyDirectorCueForSeq(int? seq) {}
      ''';
    const String commentedOut = '''
      // _sentenceCueSubscription = _audio.sentenceStarts.listen(
      //   _applyDirectorCueForSeq,
      // );
      void _applyDirectorCueForSeq(int? seq) {}
      ''';

    expect(
      kDirectorCueCallSite.hasMatch(stripCommentsAndStrings(callSite)),
      isTrue,
      reason: '正样本（真实调用形状）必须命中，否则判据写错了',
    );
    expect(
      kDirectorCueCallSite.hasMatch(stripCommentsAndStrings(commentedOut)),
      isFalse,
      reason:
          '把 :481-483 注释掉 = 没有接线。旧断言 contains(名字) 在这里会失败地'
          '保持绿——本判据必须变红（编排者的验收手法）。',
    );
  });

  test('判别力自证（对真实文件）：去掉调用点后，名字仍在但判据必红', () {
    // 编排者的验收手法 = 「把 :481-483 注释掉 → 必红」。lib/** 是只读的
    // （任务硬约束：一行都不许改），所以这里在**内存里**对**真实 main.dart**
    // 做等价变换：只删掉判据命中的那个调用点，定义与别处的名字原样保留。
    final Iterable<RegExpMatch> hits = kDirectorCueCallSite.allMatches(
      mainSource,
    );
    expect(
      hits.length,
      1,
      reason: '真实文件里必须**恰好一处**调用点（多了说明判据太松）',
    );
    final String callSiteRemoved = mainSource.replaceRange(
      hits.single.start,
      hits.single.end,
      '_callSiteRemoved',
    );
    expect(
      callSiteRemoved.contains('_applyDirectorCueForSeq'),
      isTrue,
      reason: '定义（main.dart:591）还在 —— 与「只注释掉调用点」的现场同形',
    );
    expect(
      kDirectorCueCallSite.hasMatch(callSiteRemoved),
      isFalse,
      reason:
          '调用点没了 → 本判据必须红；而旧的 contains(名字) 断言在这里仍会绿'
          '（这正是它零检验力的证明）。',
    );
  });

  test('判别力自证：只剩定义（无调用）→ 不命中', () {
    const String definitionOnly = '''
      void _applyDirectorCueForSeq(int? seq) {}
      ''';
    expect(
      kDirectorCueCallSite.hasMatch(stripCommentsAndStrings(definitionOnly)),
      isFalse,
      reason: '定义本身不是接线：这正是「不是只定义」的字面要求',
    );
  });
}
