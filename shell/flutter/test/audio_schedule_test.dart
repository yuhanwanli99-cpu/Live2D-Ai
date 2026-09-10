import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/audio/schedule.dart';

/// 一片 20 ms（服务端 WS 音频分片固定 20 ms）。
const double kSliceSec = 0.02;

/// 真实客户端抓包得到的**到达节奏**（2026-09-10，一轮三句话）：
/// 每 10 片（= 一个 200 ms TTS 分片）在约 1 ms 内突发到齐，句间有数秒空档。
List<double> realArrivals() {
  final out = <double>[];
  // 三句：17 / 13 / 14 个 200 ms 分片，句内 1 ms 间隔，句间 4.286 s / 3.743 s。
  var t = 6.62;
  for (final chunks in [17, 13, 14]) {
    for (var c = 0; c < chunks; c++) {
      for (var i = 0; i < 10; i++) {
        out.add(t);
        t += 0.001;
      }
    }
    t += chunks == 17 ? 4.286 : 3.743;
  }
  return out;
}

/// 按到达序列驱动排片。
List<SlicePlan> runTimeline(
  List<double> arrivals, {
  required bool legacyRestartOnSliceStart,
}) {
  var nextStart = 0.0;
  final plans = <SlicePlan>[];
  for (var i = 0; i < arrivals.length; i++) {
    final now = arrivals[i];
    // 新规则：时间轴只前进（scheduleSlice 的唯一锚定条件是「排空」）。
    // 旧缺陷规则：每个分片首帧（每 10 片）把时间轴拽回 now。
    if (legacyRestartOnSliceStart && i % 10 == 0) nextStart = now;
    final plan = scheduleSlice(
      nextStart: nextStart,
      now: now,
      durationSec: kSliceSec,
    );
    plans.add(plan);
    nextStart = plan.nextStartAfter;
  }
  return plans;
}

/// 重叠对数（后续片开始早于先前片结束）+ 最大同时发声数。
({int overlaps, int maxConcurrent, double overlapSec}) overlapStats(
  List<SlicePlan> plans,
) {
  var overlaps = 0;
  var overlapSec = 0.0;
  var prevEnd = double.negativeInfinity;
  for (final p in plans) {
    if (p.when < prevEnd - 1e-9) {
      overlaps++;
      overlapSec += prevEnd - p.when;
    }
    prevEnd = p.nextStartAfter > prevEnd ? p.nextStartAfter : prevEnd;
  }
  final edges = <({double t, int d})>[];
  for (final p in plans) {
    edges.add((t: p.when, d: 1));
    edges.add((t: p.nextStartAfter, d: -1));
  }
  edges.sort((a, b) => a.t == b.t ? a.d - b.d : a.t.compareTo(b.t));
  var cur = 0;
  var maxConcurrent = 0;
  for (final e in edges) {
    cur += e.d;
    if (cur > maxConcurrent) maxConcurrent = cur;
  }
  return (overlaps: overlaps, maxConcurrent: maxConcurrent, overlapSec: overlapSec);
}

void main() {
  group('scheduleSlice', () {
    test('分片全部瞬时到齐时按序顺排，绝不重叠', () {
      // 44 个 200 ms 分片（每个 10 片）在 t=0 同时到达：模拟 TTS 突发供数。
      final plans = runTimeline(
        List<double>.filled(440, 0),
        legacyRestartOnSliceStart: false,
      );
      expect(plans.length, 440);
      for (var i = 0; i < plans.length; i++) {
        expect(plans[i].when, closeTo(i * kSliceSec, 1e-9), reason: '第 $i 片');
      }
      final stats = overlapStats(plans);
      expect(stats.overlaps, 0);
      expect(stats.maxConcurrent, 1);
    });

    test('时间轴排空后才锚定到 now', () {
      var nextStart = 0.0;
      final first = scheduleSlice(
        nextStart: nextStart,
        now: 1.0,
        durationSec: kSliceSec,
      );
      expect(first.when, 1.0);
      nextStart = first.nextStartAfter;

      // 未排空：继续顺排，不回退。
      final second = scheduleSlice(
        nextStart: nextStart,
        now: 1.005,
        durationSec: kSliceSec,
      );
      expect(second.when, closeTo(1.02, 1e-9));

      // 已排空（now 已越过排空点）：锚定到 now，不产生「补播」。
      final third = scheduleSlice(
        nextStart: second.nextStartAfter,
        now: 5.0,
        durationSec: kSliceSec,
      );
      expect(third.when, 5.0);
    });

    test('when 恒 >= 入口 nextStart（时间轴单调，任何到达节奏下都不重叠）', () {
      final plans = runTimeline(
        realArrivals(),
        legacyRestartOnSliceStart: false,
      );
      final stats = overlapStats(plans);
      expect(plans.length, 440);
      expect(stats.overlaps, 0, reason: '任何到达节奏下都不得重叠');
      expect(stats.maxConcurrent, 1);
      // 三句共 8.8 s 音频，句间 8.03 s 空档 → 跨度约 16.8 s。
      expect(plans.last.nextStartAfter, greaterThan(16.0));
    });

    test('回归钉子：旧的「分片起始重置」规则在同一到达节奏下必然重叠', () {
      final plans = runTimeline(
        realArrivals(),
        legacyRestartOnSliceStart: true,
      );
      final stats = overlapStats(plans);
      // 这正是「很吵的杂音」：同一份音频被多路叠加播放。
      expect(stats.overlaps, greaterThan(300));
      expect(stats.maxConcurrent, greaterThan(5));
      expect(stats.overlapSec, greaterThan(5.0));
    });
  });

  group('LevelTimeline', () {
    test('按播放时刻释放，而不是按到达时刻', () {
      final line = LevelTimeline();
      // 模拟：音频 1 ms 内到齐，但要 3 s 后才出声。
      line.push(3.00, 0.4);
      line.push(3.02, 0.8);
      line.push(3.04, 0.2);

      expect(line.drain(0.5), isNull, reason: '还没到播放时刻，不得驱动口型');
      expect(line.drain(2.99), isNull);
      expect(line.drain(3.00), 0.4);
      expect(line.drain(3.01), isNull, reason: '同一 tick 不重复释放');
      expect(line.drain(3.05), 0.2, reason: '一 tick 内多个到期项只保留最后一个');
      expect(line.isEmpty, isTrue);
      expect(line.drain(99), isNull);
    });

    test('入队时刻结构性非降序（时间轴只前进）', () {
      final line = LevelTimeline();
      line.push(10.0, 1.0);
      line.push(3.0, 2.0); // 早于队尾：夹到队尾而非插入过去
      expect(line.length, 2);
      expect(
        line.drain(9.999),
        isNull,
        reason: '第二项被夹到 10.0；若仍停在 3.0，这里就会提前释放',
      );
      expect(line.drain(10.0), 2.0, reason: '两项同在 10.0 到期，取最后一个');
      expect(line.isEmpty, isTrue);
    });

    test('clear 清空全部待释放项', () {
      final line = LevelTimeline();
      line.push(1.0, 0.5);
      line.clear();
      expect(line.isEmpty, isTrue);
      expect(line.drain(99), isNull);
    });
  });
}
