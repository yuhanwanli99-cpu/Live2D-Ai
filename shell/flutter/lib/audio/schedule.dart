/// 音频分片调度算术与口型时间线：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 单独成文件的理由：这两条规则是本项目「很吵的杂音」与「口型超前」两个
/// 缺陷的全部算术核心，必须能在不启动浏览器、不依赖 WebAudio 的情况下回归。
library;

import 'dart:collection';

/// 一次排片的算术结果。
///
/// - [when]：应传给 AudioBufferSourceNode.start(when) 的时刻（AudioContext 时间轴）；
/// - [nextStartAfter]：本片排完之后，下一片的排空时刻。
class SlicePlan {
  const SlicePlan(this.when, this.nextStartAfter);

  /// 本片开始发声的时刻。
  final double when;

  /// 本片结束时刻，即下一片的起点（未考虑排空追赶）。
  final double nextStartAfter;

  @override
  String toString() =>
      'SlicePlan(when: $when, nextStartAfter: $nextStartAfter)';
}

/// 把一片音频排到播放时间轴上。
///
/// **核心不变量：时间轴永不回退。** [SlicePlan.when] 恒 ≥ 入口的 [nextStart]，
/// 因此任意两片在时间轴上互不重叠。WebAudio 会把同时发声的音源**逐样本相加**：
/// 重叠得到的是失真（削波、梳状滤波），不是「更响」。
///
/// **绝不能用「分片起始标记」重置时间轴。** 服务端的 audio.start 是「每个
/// TTS 流式分片（约 200 ms）的首帧」，不是「一轮语音的起点」——实测一轮 8.6 s
/// 语音会出现 44 个 start=true。若据此把 [nextStart] 拽回 [now]，而此刻上一片
/// 通常仍在播放（真实客户端实测：56 次重置里 52 次如此），两片便同时发声。
/// 后果（真实客户端 534 次 start 调用的实测值）：一轮 10.68 s 音频被压进
/// 13.99 s 时间轴（0.76×，物理上不可能）、529 对重叠、累计重叠 9.79 s、
/// 最多 **17 个音源同时响**、混音峰值 1.109（削波）——听感即「很吵的杂音」。
///
/// 真正的轮次边界由 epoch 变化驱动 AudioPlayer.interrupt()（清零 [nextStart]），
/// 不依赖本函数。
///
/// 只有时间轴**排空**（[nextStart] 落后于 [now]）时才锚定到 [now]：那是播放
/// 必须追赶实时的地方（TTS 供数慢于实时，或一句结束到下一句之间有数秒空档）。
/// 实测分片到达节奏正是「一句 2 s 音频在约 1 ms 内突发到齐 → 3.7~4.3 s 空档」，
/// 故排空锚定会频繁触发，而排片提前量稳定在 3.4 s 以内（无需额外限流）。
SlicePlan scheduleSlice({
  required double nextStart,
  required double now,
  required double durationSec,
}) {
  final start = nextStart < now ? now : nextStart;
  return SlicePlan(start, start + durationSec);
}

/// 「按实际播放时刻释放口型电平」的时间线。
///
/// TTS 分片是**突发到达**的：一句约 2 s 的音频在约 1 ms 内全部到齐，随后是数秒
/// 空档（LLM 还在生成下一句）。若在**到达时**就把电平推给渲染面，口型会比声音
/// 早 1.45 s（均值）/ 3.36 s（峰值，实测）——即「没出声嘴唇先动」。
///
/// 因此 feed() 只在到达时**计算**包络（顺序与音频一致），把
/// (播放时刻, 电平) 入队，由 [drain] 在到点时释放。
class LevelTimeline {
  final ListQueue<({double at, double level})> _pending =
      ListQueue<({double at, double level})>();

  /// 待释放项数量。
  int get length => _pending.length;

  /// 是否已排空。
  bool get isEmpty => _pending.isEmpty;

  /// 入队一个「到 [at] 时刻才应生效」的电平。
  ///
  /// 入队时刻被**结构性地**保证非降序：若 [at] 早于队尾，则夹到队尾（时间轴
  /// 只前进）。调用方 feed() 按到达序排片，[scheduleSlice] 的返回值本身
  /// 已非降序，因此这里的夹持是防御性的、正常路径不触发。
  void push(double at, double level) {
    final last = _pending.isEmpty ? null : _pending.last.at;
    _pending.addLast((at: last != null && at < last ? last : at, level: level));
  }

  /// 释放所有 at <= now 的电平，返回**最后一个**到期电平；无到期项（或全部
  /// 晚于 [now]）→ null。
  ///
  /// 同一 tick 内多个到期项只保留最后一个：消费端（Live2D 桥）自身按 <=30 Hz
  /// 节流，中间值不会显示，保留它只会多做一次无效编码。
  double? drain(double now) {
    double? last;
    while (_pending.isNotEmpty && _pending.first.at <= now) {
      last = _pending.removeFirst().level;
    }
    return last;
  }

  /// 清空（epoch 切换 / stop / dispose）。
  void clear() => _pending.clear();
}
