/// 调试面板的「最近一条预设」显示快照 + 渲染面 ack → 快照的纯映射。
///
/// # 为什么搬到这里 / 为什么不再有本地计时器（阶段4d，V8）
///
/// v0 的 PresetStatus 住在 live2d_stage.dart：applyPreset 之后启动一个
/// Timer.periodic(200ms) 本地推演「还剩多少毫秒」，到点把快照清成 null。
/// 那是**前端镜像**：它只知道「我发了什么」，不知道渲染面的钳位 / 降级 /
/// 与 idle 叠加。阶段4d 删掉那个本地定时器与「remaining 驱动状态变化」——
/// 现在快照**只由渲染面 ack 生成**（[presetStatusUpdateFor]），真源在渲染面。
///
/// [PresetStatus.remaining] 保留：它是**调试面板的显示换算**，不再是驱动任何
/// 行为的计时器（面板按 ack 到达时的时间起算，ack 说结束就结束）。
library;

import 'render_events.dart';

/// 协议 O3 冻结的默认时长（ttl_ms 省略时）：body/head 900、expression 2600。
///
/// 这里只用于**调试面板的显示**；前端不据此调度任何东西，也不把它发出去。
const Map<String, int> kDefaultTtlMsByField = <String, int>{
  'body': 900,
  'head': 900,
  'expression': 2600,
};

/// 旧 preset_id 路径（无 field）的显示用默认时长（v0 手势包口径）。
const int kLegacyPresetTtlMs = 900;

int defaultTtlMsForField(String? field) =>
    (field == null ? null : kDefaultTtlMsByField[field]) ?? kLegacyPresetTtlMs;

/// 开发工具「动作调试」的本地状态（P0-3）。
///
/// **不是**权威状态：渲染面的 ack 才是（[presetStatusUpdateFor] 是它唯一的
/// 写入方）。这里只记「渲染面最近回执了哪一条、什么时候、显示用时长多久」。
class PresetStatus {
  const PresetStatus({
    required this.id,
    required this.source,
    required this.startedAt,
    required this.ttl,
  });

  final String id;

  /// render / debug / director …（显示用）
  final String source;
  final DateTime startedAt;
  final Duration ttl;

  /// 还剩多久（钳到 ≥ 0）。**显示换算**，不是计时器。
  Duration remaining(DateTime now) {
    final Duration elapsed = now.difference(startedAt);
    final Duration left = ttl - elapsed;
    return left.isNegative ? Duration.zero : left;
  }
}

/// ack → 面板快照的**下一次动作**（纯函数，VM 可测）。
enum PresetStatusAction { set, clear, ignore }

class PresetStatusUpdate {
  const PresetStatusUpdate(this.action, [this.status]);

  final PresetStatusAction action;
  final PresetStatus? status;
}

/// 渲染面 ack → 快照更新。
///
/// - preset-applied → 生成一条（id 取 ack 的 id，缺 id 时回落 field；时长取
///   渲染面给的 ttl_ms，缺省用 O3 默认）；
/// - preset-replaced / preset-expired / preset-dropped → 清空（该动画段结束）；
/// - segment-ended → **不动**（它是音频事件，与预设显示无关）。
PresetStatusUpdate presetStatusUpdateFor(
  RenderEvent event,
  DateTime now, {
  String source = 'render',
}) {
  switch (event.kind) {
    case RenderAckKind.applied:
      final String id = (event.id ?? '').trim();
      final String field = (event.field ?? '').trim();
      if (id.isEmpty && field.isEmpty) {
        return const PresetStatusUpdate(PresetStatusAction.ignore);
      }
      return PresetStatusUpdate(
        PresetStatusAction.set,
        PresetStatus(
          id: id.isEmpty ? field : id,
          source: source,
          startedAt: now,
          ttl: Duration(
            milliseconds:
                event.ttlMs ?? defaultTtlMsForField(field.isEmpty ? null : field),
          ),
        ),
      );
    case RenderAckKind.replaced:
    case RenderAckKind.expired:
    case RenderAckKind.dropped:
      return const PresetStatusUpdate(PresetStatusAction.clear);
    case RenderAckKind.segmentEnded:
      return const PresetStatusUpdate(PresetStatusAction.ignore);
  }
}
