/// 动作预设 **id → 展示名**：唯一真源是 `assets/actions/preset_labels.json`
/// （经服务端静态路由 `GET /actions/preset_labels.json` 取到）。
///
/// # 为什么不在 Dart 里写一张中文表
///
/// 同类项目最典型的崩坏是「同一个数写 3 遍」：Rust 导演 Select、Flutter 调试面板、
/// 文档各写一套中文名，改一处忘两处。调研（`docs/research/preset-label-map-2026-09.md`）
/// 因此把展示名收进**一张 JSON**：Rust 侧 `include_str!` 读它，Flutter 侧 fetch 它，
/// 文档引用它。
///
/// 取不到表不是错误：面板回落显示稳定 id（**不隐藏、不谎报中文名**）。
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

/// 表情通道的包 id（与渲染面 `assets/actions/presets.json` 的 `kind=expression` 对齐）。
///
/// **显示用**分类：只影响调试面板倒计时与「表情 / 短动作」前缀。行为真源是渲染面
/// 的 `presets.json`；旧 id 已删除，不再按前缀识别。
///
/// **W3 残留（2026-09-21，已知重复）**：这是一份与 `preset_labels.json` 的
/// `channel` 字段重复的 id 列表。调试面板**已不再使用它**（改用
/// [PresetLabelTable.idsForChannel]）；它还被两处**没有标签表实例**的显示代码用：
/// `live2d/live2d_stage.dart`（倒计时显示用 ttl）与
/// `settings/mods/director_panel.dart`（通道标签）。删掉它要求同时改这两个
/// 文件（不在 W3 的文件归属内），所以本轮**记录不改**，见 W3 回报的未决问题。
const Set<String> kExpressionPresetIds = <String>{'smile', 'unhappy', 'surprised'};

/// 该 id 是否是表情包（只认上面的新 id 集合）。
bool isExpressionPreset(String id) => kExpressionPresetIds.contains(id);

/// 一条预设的展示信息。
class PresetLabel {
  const PresetLabel({required this.zh, required this.en, this.channel});

  /// 中文展示名。
  final String zh;

  /// 英文规范短名。
  final String en;

  /// `expression` / `motion`（可为 null）。
  final String? channel;
}

/// 不可变的标签表。
class PresetLabelTable {
  const PresetLabelTable(this._byId);

  final Map<String, PresetLabel> _byId;

  /// 空表：所有 id 原样显示。
  static const PresetLabelTable empty = PresetLabelTable(<String, PresetLabel>{});

  int get length => _byId.length;

  PresetLabel? operator [](String id) => _byId[id];

  /// 中文名；没有 → null（调用方回落 id）。
  String? zh(String id) => _byId[id]?.zh;

  /// 面板展示文案：`中文（id）`；没有标签时只回 id。
  String display(String id) {
    final PresetLabel? label = _byId[id];
    return label == null ? id : '${label.zh}（$id）';
  }

  /// 某个 `channel` 下的包 id，**保持 JSON 里的出现顺序**。
  ///
  /// 这是调试面板两块按钮清单（表情 / 手势）的**唯一来源**——面板不再自己
  /// 抄一份 id 列表（RESEARCH §4 E7）：单一真源就是 `preset_labels.json` 的
  /// `channel` 字段（`expression` / `motion`）。
  ///
  /// `none` 被**显式排除**：它在 JSON 里带 `channel: expression`，但语义是
  /// 渲染面的**撤销哨兵**（两槽同清），不是一条可播放的表情包——它另有自己那枚
  /// 「归零（none，两槽同清）」按钮。取不到表 → 空表 → 空列表（调用方必须
  /// 如实说明，不许静默显示空按钮组）。
  List<String> idsForChannel(String channel) => <String>[
    for (final MapEntry<String, PresetLabel> e in _byId.entries)
      if (e.key != 'none' && e.value.channel == channel) e.key,
  ];

  /// 从 JSON 文本解析。**坏表不抛**——返回 [empty]，面板照常打开（显示 id）。
  static PresetLabelTable parse(String source) {
    try {
      final Object? decoded = jsonDecode(source);
      if (decoded is! Map) return empty;
      final Object? raw = decoded['labels'];
      if (raw is! Map) return empty;
      final Map<String, PresetLabel> out = <String, PresetLabel>{};
      raw.forEach((Object? key, Object? value) {
        if (key is! String || value is! Map) return;
        final Object? zh = value['zh'];
        if (zh is! String || zh.trim().isEmpty) return;
        final Object? en = value['en'];
        final Object? channel = value['channel'];
        out[key] = PresetLabel(
          zh: zh,
          en: en is String ? en : '',
          channel: channel is String ? channel : null,
        );
      });
      return out.isEmpty ? empty : PresetLabelTable(out);
    } catch (_) {
      return empty;
    }
  }
}

/// 从同源静态路由读标签表（best-effort；失败 → [PresetLabelTable.empty]）。
Future<PresetLabelTable> fetchPresetLabels() async {
  try {
    final http.Response res = await http.get(
      Uri.base.resolve('/actions/preset_labels.json'),
    );
    if (res.statusCode != 200) return PresetLabelTable.empty;
    return PresetLabelTable.parse(res.body);
  } catch (_) {
    return PresetLabelTable.empty;
  }
}
