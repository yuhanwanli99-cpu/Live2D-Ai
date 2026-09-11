/// 酒馆（SillyTavern）角色卡解析：**纯逻辑、零新依赖、可在 VM 上单测**。
///
/// 支持两种形态（同一份解析器）：
/// - **V1 扁平**：`{"name":…,"description":…,"first_mes":…}`
/// - **V2 嵌套**：`{"spec":"chara_card_v2","data":{…}}`（字段在 `data` 里）
///
/// # 为什么必须支持 V2
///
/// 2023 年后的酒馆卡基本都是 V2。只认 V1 会让用户导入时**静默拿到空角色**——
/// 字段名一个都对不上，界面显示「已导入」但四项全空。这是最难查的一类问题。
///
/// # 关于字段名
///
/// 酒馆用 `first_mes`（first message），本项目设置里叫 `first`；
/// 酒馆用 `mes_example`（示例对话）而本项目**没有**对应字段——
/// 这种情况**如实报告「未映射」**，不悄悄丢掉（见 [PersonaCard.unmapped]）。
library;

import 'dart:convert';

/// 解析出的角色卡（字段已归一化到本项目的命名）。
class PersonaCard {
  const PersonaCard({
    this.name = '',
    this.description = '',
    this.personality = '',
    this.scenario = '',
    this.first = '',
    this.systemPrompt = '',
    this.creatorNotes = '',
    this.tags = const <String>[],
    this.unmapped = const <String>[],
    this.format = 'unknown',
  });

  final String name;
  final String description;
  final String personality;
  final String scenario;

  /// 开场白（酒馆的 `first_mes`）。
  final String first;

  /// 自定义系统提示词（酒馆 V2 的 `data.system_prompt`；本项目的 dev 字段）。
  final String systemPrompt;

  /// 作者备注（只展示，不写入设置）。
  final String creatorNotes;

  final List<String> tags;

  /// 卡里有、但本项目**没有对应字段**的键名。
  ///
  /// 界面要把这个列出来——用户有权知道自己导入的东西少了什么。
  final List<String> unmapped;

  /// `v1` / `v2` / `unknown`。
  final String format;

  /// 是否四项主要人设字段全空（多半是解析没对上字段名）。
  bool get isEmpty =>
      name.isEmpty &&
      description.isEmpty &&
      personality.isEmpty &&
      scenario.isEmpty &&
      first.isEmpty;

  /// 参与写入的字段数（非空计数）。
  int get filledCount => <String>[
    name,
    description,
    personality,
    scenario,
    first,
  ].where((String s) => s.isNotEmpty).length;
}

/// 本项目**有**对应字段的键（归一化后）。
const Set<String> kMappedPersonaKeys = <String>{
  'name',
  'description',
  'personality',
  'scenario',
  'first_mes',
  'system_prompt',
  // 只展示：不算「未映射」，但也不写进设置。
  'creator_notes',
  'creatorcomment',
  'tags',
};

/// 解析角色卡 JSON 文本。**永不抛**：坏 JSON 返回 `null`，由调用方提示。
PersonaCard? parsePersonaCardJson(String raw) {
  final Object? decoded;
  try {
    decoded = jsonDecode(raw);
  } catch (_) {
    return null;
  }
  if (decoded is! Map) return null;
  final Map<String, Object?> root = _stringKeys(decoded);

  // V2：字段在 `data` 里。判定看 `spec` 前缀**或** `data` 里有没有人设字段
  // （有些卡 spec 字段缺失/写错，但 data 是齐的）。
  final Object? dataRaw = root['data'];
  final bool looksV2 =
      dataRaw is Map &&
      ((root['spec'] is String &&
              (root['spec']! as String).startsWith('chara_card_v2')) ||
          (dataRaw.containsKey('first_mes') || dataRaw.containsKey('name')));
  final Map<String, Object?> fields = looksV2 ? _stringKeys(dataRaw) : root;

  final String name = _str(fields['name']);
  final String description = _str(fields['description']);
  final String personality = _str(fields['personality']);
  final String scenario = _str(fields['scenario']);
  final String first = _str(fields['first_mes']);
  final String systemPrompt = _str(fields['system_prompt']);
  final String creatorNotes = _str(
    fields['creator_notes'] ?? fields['creatorcomment'],
  );
  final Object? tagsRaw = fields['tags'];
  final List<String> tags = tagsRaw is List
      ? tagsRaw.whereType<String>().toList()
      : const <String>[];

  final List<String> unmapped = <String>[
    for (final String key in fields.keys)
      if (!kMappedPersonaKeys.contains(key)) key,
  ]..sort();

  final PersonaCard card = PersonaCard(
    name: name,
    description: description,
    personality: personality,
    scenario: scenario,
    first: first,
    systemPrompt: systemPrompt,
    creatorNotes: creatorNotes,
    tags: tags,
    unmapped: unmapped,
    format: looksV2 ? 'v2' : 'v1',
  );

  // 判据是「**有没有出现过任何已映射的键**」，而不是「值是否非空」：
  //
  // - `{"foo":1}` 没有任何已映射键 → 这不是角色卡，返回 null（否则界面会
  //   显示「已导入」而四项全空，用户完全不知道发生了什么）；
  // - `{"name":123}` 有已映射键但值类型不对 → **是**角色卡，只是 name 为空。
  //   用「值非空」判会把它也拒掉，而那是一张真的卡里的一处坏字段。
  final bool sawMappedKey = fields.keys.any(kMappedPersonaKeys.contains);
  if (!sawMappedKey) return null;
  return card;
}

/// 只保留字符串键（JSON 数字键之类直接丢，不让它把 `Map<String, _>` 弄脏）。
Map<String, Object?> _stringKeys(Map<Object?, Object?> raw) {
  final Map<String, Object?> out = <String, Object?>{};
  raw.forEach((Object? k, Object? v) {
    if (k is String) out[k] = v;
  });
  return out;
}

String _str(Object? value) => value is String ? value : '';
