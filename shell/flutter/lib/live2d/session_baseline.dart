/// 会话 baseline 的**应用语义**（协议 V10 §9.3 / O8 / D28，阶段4f）。
///
/// host 已两路交付 baseline——**任一路到达即立即应用**到舞台：
/// ① `POST /api/v1/chat` / `/chat/stop` **响应体的 `baseline` 字段**
///   （`api/api_client.dart` 转发进 [baselineApplication]）；
/// ② 既有 `action_cue` 帧上的 `{baseline:true, reason}`（Gate 4 **D31**；
///   帧结构只增不改）。
///
/// # 本文件是纯逻辑（无 IO / 无 `package:web`），可在 VM 单测
///
/// - 两路交付的原文**用同一个解码器**（[ActionCue.fromJson]）解释：
///   - HTTP 路径：`baseline` 原文（单个 cue 对象 / cue 对象数组）；
///   - WS 路径：`action_cue` 帧的 `cues` 列表（已由事件层解好）。
///   因此不存在「两套解释」。
/// - 空 baseline（`null` / 空数组 / 坏形状 / 无非对象元素）→ **恰好一次
///   撤销**（`id == 'none'`，等价于 `applyPreset('none')` 回待机）——
///   这是 D28 的硬要求：**不得退化成「什么都不做」**。
/// - 未知字段一律忽略（V11）：[ActionCue] 的 `fromJson` 只取认识的键。
///
/// # 不建前端镜像（V8 / V10）
///
/// 本文件只做「原文 → 一次性下发清单」；不缓存、不比较、不推演剩余时长。
/// 下发清单用完即丢——前端没有任何 baseline 状态。
library;

import '../api/ws_frame.dart';

/// 一次 `preset` 下发的参数（与 `Live2DStageState.applyPreset` 的具名参数同形）。
///
/// `null` = **不指定**，由渲染面 / 舞台用缺省（不是「零」）。把「缺省」与
/// 「显式 0」分开，是为了让 baseline 里没写的键不改变既有缺省语义（V11）。
class BaselinePresetCall {
  const BaselinePresetCall({
    required this.id,
    this.intensity,
    this.ttlMs,
    this.field,
    this.x,
    this.y,
    this.z,
    this.hold,
    this.at,
    this.seq,
    this.sentenceSeq,
  });

  /// **回待机**：`preset_id == 'none'` 撤销哨兵（渲染面翻成两槽 `Revoke`）。
  const BaselinePresetCall.revoke()
    : id = 'none',
      intensity = null,
      ttlMs = null,
      field = null,
      x = null,
      y = null,
      z = null,
      hold = null,
      at = null,
      seq = null,
      sentenceSeq = null;

  /// 下发给渲染面的 `preset` id（legacy preset_id，或 v1 的语义 id）。
  final String id;

  /// 强度；`null` = 用舞台缺省（[kDefaultPresetIntensity]）。
  final int? intensity;

  /// 生效时长（毫秒）；`null` = 用渲染面缺省。
  final int? ttlMs;

  /// v1 三族字段之一（body / head / expression）；`null` = legacy 路径。
  final String? field;

  final double? x;
  final double? y;
  final double? z;

  /// v1：true 保持到下次指令。
  final bool? hold;

  /// v1 锚点（now / seg:N / after_prev）。
  final String? at;

  /// v1 plan 内序号。
  final int? seq;

  /// 目标句序号（baseline 里通常没有；缺省 `null`）。
  final int? sentenceSeq;

  @override
  String toString() => field == null ? id : '$id(field=$field)';
}

/// host 交付的 baseline **原文**（HTTP 响应的 `baseline` 字段）→ cue 列表。
///
/// 合法形状（与 host 侧 `baseline_cues` 同口径，见
/// `crates/live2d-ai-desktop/src/web_api/chat_routes.rs`）：
/// - 一个 cue 对象 → 单条；
/// - 一个 cue 对象数组 → 原样（**非对象元素丢弃**）；
/// - `null` / 其它标量 / 坏形状 → 空表。
///
/// 空表**不是终点**：[baselineApplicationFromCues] 会把它翻成一次撤销。
List<ActionCue> parseBaselineCues(Object? raw) {
  final List<Object?> items = raw is List
      ? raw
      : (raw is Map ? <Object?>[raw] : const <Object?>[]);
  final List<ActionCue> cues = <ActionCue>[];
  for (final Object? item in items) {
    if (item is! Map) continue; // 非对象元素丢弃（host 同口径）
    cues.add(ActionCue.fromJson(Map<String, Object?>.from(item)));
  }
  return cues;
}

/// cue 列表 → **一次性下发清单**。
///
/// 空列表 ⇒ 恰好一条 [BaselinePresetCall.revoke]（回待机）。
/// 绝不返回空清单——空清单就是 D28 明令禁止的「什么都不做」。
List<BaselinePresetCall> baselineApplicationFromCues(List<ActionCue> cues) {
  if (cues.isEmpty) {
    return const <BaselinePresetCall>[BaselinePresetCall.revoke()];
  }
  return cues.map(_callFrom).toList(growable: false);
}

/// host 交付的 baseline 原文 → 一次性下发清单（HTTP 路径入口）。
List<BaselinePresetCall> baselineApplication(Object? raw) =>
    baselineApplicationFromCues(parseBaselineCues(raw));

/// 单条 cue → 下发参数。
///
/// id 取值口径（与 runtime `FieldCue::to_wire_cue` 对齐）：
/// - legacy（无 `field`）→ `preset_id` 原样；
/// - v1 且有 `preset_id` → `preset_id`（表演层的语义占位）；
/// - v1 只有 `id` → `id`（expression 面板 id）；
/// - v1 `body`/`head` 无 id → 字段名本身（与 runtime 同口径）；
/// - v1 `expression` 无 id → 空串（没有可下的表情，交给渲染面按空 id 处理）。
BaselinePresetCall _callFrom(ActionCue cue) {
  final String field = cue.field ?? '';
  final String id;
  if (field.isEmpty || cue.presetId.isNotEmpty) {
    id = cue.presetId;
  } else if (cue.id != null && cue.id!.isNotEmpty) {
    id = cue.id!;
  } else {
    id = field == 'expression' ? '' : field;
  }
  return BaselinePresetCall(
    id: id,
    // 0 = 帧里没写（ActionCue 的宽容缺省）→ 交给舞台缺省，而不是「强度 0」。
    intensity: cue.intensity > 0 ? cue.intensity : null,
    ttlMs: cue.ttlMs > 0 ? cue.ttlMs : null,
    field: field.isEmpty ? null : field,
    x: cue.x,
    y: cue.y,
    z: cue.z,
    hold: cue.hold,
    at: cue.at,
    seq: cue.seq,
    sentenceSeq: cue.sentenceSeq > 0 ? cue.sentenceSeq : null,
  );
}
