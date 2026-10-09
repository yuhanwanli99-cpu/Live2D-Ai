/// 朗读高亮：**当前播放的是哪一句** → 它在气泡正文里的那一段。
///
/// 纯逻辑（只依赖 [SpokenSpan]），因为判据要在 `flutter test` 的 VM 上跑得到：
/// `ChatController` 经 `ws_client.dart` 依赖 `package:web`，加载不了。
///
/// # 三条不可动摇的前提（来自服务端契约）
///
/// 1. 句子号的唯一真源是 `text_delta.sentence_seq`（与音频帧**同一个数**，
///    从 1 起）。**不要**用标点把气泡正文重新切一遍——每条 delta 就是一整句。
///    **2026-10-08**：这条 delta 带的是**上屏文本**（二路清洗交回的 `display`：
///    emoji / 颜文字 / 括号里的动作描写都留着），**与送 TTS 的 `speech` 不再是
///    同一串字**——高亮只按 `sentence_seq` 落在**上屏文本**上，序号对不上就
///    不高亮（本文件不碰 `speech`，也拿不到它）。
/// 2. 播放中的句号由 `AudioPlayer` 给出（`sentenceStarts` + 舞台时钟
///    `stageClock`，二者同源）。**不要**另做时钟。
/// 3. 序号对不上、或此刻没有播放 → **不高亮**（宁可没有，不可高亮错字）。
library;

import 'package:flutter/foundation.dart';

import 'chat_message.dart';

/// 要高亮的那一段在**拼接正文**里的起止（`[start, end)`，UTF-16 码元下标）。
class SpokenHighlight {
  const SpokenHighlight({required this.start, required this.end});

  final int start;
  final int end;

  int get length => end - start;
}

/// 把已上屏的句子按上屏顺序拼成正文——[SpokenHighlight] 的坐标系。
String joinSpoken(List<SpokenSpan> spoken) =>
    spoken.map((SpokenSpan span) => span.text).join();

/// 给定 [spoken] 与当前播放的句号，返回该句在拼接正文里的起止。
///
/// `null`（= **不高亮**）有三种情况，缺一不可：
/// - [playingSeq] 为 `null`：此刻没有播放（轮次之间 / 停止后 / 换会话）；
/// - 没有任何 [SpokenSpan.seq] 等于它：序号对不上（缺 `sentence_seq` 的那一条、
///   或上一轮播放号落到了新气泡上）；
/// - 命中的那一句是空串：空区间没有可高亮的东西。
SpokenHighlight? spokenHighlight(List<SpokenSpan> spoken, int? playingSeq) {
  if (playingSeq == null) return null;
  int offset = 0;
  for (final SpokenSpan span in spoken) {
    if (span.seq == playingSeq) {
      if (span.text.isEmpty) return null;
      return SpokenHighlight(start: offset, end: offset + span.text.length);
    }
    offset += span.text.length;
  }
  return null;
}

/// 拼接正文能不能直接当 [text] 的坐标用。
///
/// 正常路径下二者逐字相等（`bubble.text` 就是逐条 delta 追加出来的）。但只要
/// 中间夹过一条**没有 `sentence_seq`** 的 delta，拼接正文就会短一截，此时按它
/// 算出来的下标会**指到别的字上**——宁可整条不高亮，也不高亮错。
bool highlightFitsText(String text, List<SpokenSpan> spoken) {
  final String joined = joinSpoken(spoken);
  return joined.isNotEmpty && text.startsWith(joined);
}

/// 朗读高亮的**状态**：当前播放的句号（`null` = 不高亮）。
///
/// 状态放这里（而不是 `chat_controller.dart`，那个文件已接近 800 行上限）：
/// 控制器只把两条音频流原样转进来、在轮次/会话边界处 [clear]。
class SpokenHighlightController extends ChangeNotifier {
  int? _playingSeq;

  /// 当前正在播放的句号；`null` = 没有高亮。
  int? get playingSeq => _playingSeq;

  /// 一句**开始播放**（`AudioPlayer.sentenceStarts` 的原样转发）。
  void sentenceStarted(int? seq) {
    if (_playingSeq == seq) return;
    _playingSeq = seq;
    notifyListeners();
  }

  /// 舞台时钟采样（`AudioPlayer.stageClock` 的原样转发）。
  ///
  /// `playing == false` 就是「这一句不响了」：自然播完 / 被 interrupt /
  /// 缓冲暂停都走这一条。**不另做时钟**——判据就是音频播放时钟自己报的。
  void onStageClock({required bool playing}) {
    if (!playing) clear();
  }

  /// 灭掉高亮（播放结束 / 轮次收口 / 停止 / 切会话 / 新消息）。
  void clear() {
    if (_playingSeq == null) return;
    _playingSeq = null;
    notifyListeners();
  }
}
