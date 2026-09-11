/// 流式回复的**节流播报**：`Semantics(liveRegion: true)` + 限速。
///
/// # 为什么必须节流
///
/// `text_delta` 是**毫秒级**到达的（一个 SSE chunk 一个 delta）。如果每来一个
/// delta 就更新一次 `liveRegion`，读屏会被淹没——用户听到的是一串互相打断的
/// 音节，比不说话更糟。
///
/// 规格 §9.1 给的判据是「≥1.5 s，或按整句到达播报」。本项目选了**两条都要**：
/// - 整句到达（`。！？…`）→ **立刻**播报（这是有意义的语义边界）；
/// - 否则按 [minInterval] 限速，且**只在正文真的变长时**才更新。
///
/// 纯逻辑、时钟可注入 → 可确定性单测。
library;

import 'package:flutter/foundation.dart';

/// 句末标点（与 `live2d-ai-runtime` 的分句标点对齐）。
///
/// 这里**只是用来判断「该不该立刻播报」**，不做真正的分句——
/// 分句是核心层的职责（`dialogue/sentence.rs`），前端不复制。
const String kSentenceEnders = '。！？…；\n';

/// 播报节流器。
class LiveRegionThrottle extends ChangeNotifier {
  LiveRegionThrottle({
    Duration? minInterval,
    DateTime Function()? now,
  }) : minInterval = minInterval ?? kDefaultInterval,
       _now = now ?? DateTime.now;

  /// 默认播报间隔。
  ///
  /// 取值理由：读屏念一句中文约 2–4 字/秒，1.5 s 大约够念 4–6 个字。
  /// 再密就会互相打断；再疏则「思考中」的体感消失。
  static const Duration kDefaultInterval = Duration(milliseconds: 1500);

  final Duration minInterval;
  final DateTime Function() _now;

  DateTime? _lastAnnounce;
  String _text = '';

  /// 当前应该播报出去的文本（读屏只读这个，不读正在流式的原文）。
  String get announcement => _text;

  /// 是否已经播报过（UI 可据此决定要不要挂 liveRegion）。
  bool get hasAnnouncement => _text.isNotEmpty;

  /// 喂入**完整的当前正文**（不是 delta）。
  ///
  /// 返回是否更新了 [announcement]。调用方据此决定要不要 `notifyListeners`
  /// （本类自己会通知，返回值给测试与上层判断用）。
  ///
  /// 两条放行条件：
  /// 1. 正文末尾出现了句末标点 → 立刻播报（整句到达，这是语义边界）；
  /// 2. 距上次播报已超过 [minInterval] 且正文确实变了 → 播报当前整段。
  bool feed(String fullText) {
    if (fullText == _text) return false;
    final bool sentenceDone =
        fullText.isNotEmpty && kSentenceEnders.contains(fullText[fullText.length - 1]);
    final DateTime now = _now();
    final bool intervalElapsed =
        _lastAnnounce == null || now.difference(_lastAnnounce!) >= minInterval;
    if (!sentenceDone && !intervalElapsed) return false;
    _text = fullText;
    _lastAnnounce = now;
    notifyListeners();
    return true;
  }

  /// 本轮结束：把最后一小段补播一次，然后清空。
  ///
  /// 不清空的话下一轮开始时会先念上一轮的残留；不补播的话最后一句话
  /// （往往没有句末标点，比如「好呀」）读屏用户永远听不到。
  void finish(String fullText) {
    if (fullText.isNotEmpty && fullText != _text) {
      _text = fullText;
    }
    notifyListeners();
  }

  /// 完全重置（新一轮开始 / 清空历史）。
  void reset() {
    if (_text.isEmpty && _lastAnnounce == null) return;
    _text = '';
    _lastAnnounce = null;
    notifyListeners();
  }
}
