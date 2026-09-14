/// 舞台显示偏好：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 单独成文件的理由：这些值会被持久化（localStorage）并在启动时反序列化，
/// 「非法/缺失/越界输入回落到什么」是必须能回归的算术，不该埋在 UI 里。
/// 持久化本身在 `app/browser_io.dart` 做（那里才允许 `package:web`）。
library;

import '../design/theme_id.dart';

/// 舞台背景图 dataURL 的**长度上限**（字符数，不是字节）。
///
/// 取值理由：localStorage 的常见配额是 **5 MB / 源**，而 dataURL 是 base64
/// （比原始字节大 33%），且它会被塞进偏好记录一起写。留 ~1.5 M 字符
/// （约 1.1 MB 原始数据）给背景图，剩下的余量给其余偏好与将来新增字段——
/// 一旦超配额，**整份偏好都写不进去**（不只是背景图），那会连带丢掉主题、
/// 音量、口型设置，代价远大于「一张图没记住」。
///
/// 超限不是错误：**本次会话照常生效**，只是不写盘（UI 会如实说明）。
/// 下游裁剪（canvas 缩放）没有做——它需要浏览器 canvas，属于不可测代码，
/// 这一轮刻意不引入（见 `docs/verification/flutter-shell-manual-checklist.md`）。
const int kStageImageMaxChars = 1500000;

/// 壳背景图 dataURL 的长度上限。
///
/// **与 [kStageImageMaxChars] 同一个预算**：壳背景与舞台背景写在**同一条**
/// localStorage 记录里，给两份独立上限只会让「整份偏好都写不进去」更容易发生。
/// 所以这里只是给壳背景一个可读的别名，**不是第二套配额**。
const int kShellImageMaxChars = kStageImageMaxChars;

/// 舞台背景轮播列表（用户手动维护）的**项数上限**。
///
/// 16 张的理由见 [kStagePlaylistMaxChars]——真正的上限是**总字符预算**，
/// 这一条只挡住「项数无界」。
const int kStagePlaylistMaxItems = 16;

/// 轮播列表的**总长上限**（所有项字符数之和）。
///
/// # 为什么必须单独有一条（2026-09-14，Wave 2 硬约束）
///
/// localStorage 是**一条记录**存整份 `DisplayPrefs`（`jsonEncode(prefs.toJson())`）。
/// rc.5 的教训是：**一旦超配额，整份偏好都写不进去**——不只是列表，连主题、
/// 音量、口型设置一起丢。所以列表既要有单张上限（每项按
/// [kStageImageMaxChars] 校验），也要有**总长上限**：超出时保留前面的、
/// 丢掉放不下的，并在界面上如实说明「已达上限」，而不是悄悄把整份设置写坏。
const int kStagePlaylistMaxChars = kStageImageMaxChars;

/// 舞台显示偏好。
class DisplayPrefs {
  const DisplayPrefs({
    this.theme = AppThemeId.fallback,
    this.stageImage,
    this.shellImage,
    this.syncShellStageBg = true,
    this.stagePlaylist = const <String>[],
    this.scale = defaultScale,
    this.mouthSensitivity = defaultMouthSensitivity,
    this.lipSync = true,
    this.idleEnabled = true,
    this.muted = false,
    this.volume = defaultVolume,
    this.allowDragZoom = true,
    this.tier = defaultTier,
  });

  /// 配色主题（黑/白/蓝/灰，默认黑）。
  ///
  /// # 为什么主题放在**本地偏好**而不是服务端设置里
  ///
  /// 它是「这台设备看起来是什么样」，不是「服务端怎么工作」——
  /// 换台设备（或换个浏览器）本来就该各看各的。而且它**不需要保存按钮**：
  /// 点一下立刻生效（用户是为了「看着舒服」才切的，多一步保存会毁掉这个手感）。
  /// 与 `[tts]` 那种「核心链路配置」是两个世界的东西。
  final AppThemeId theme;

  /// 用户导入的舞台背景图（dataURL）。`null` = 不用背景图，只有纯色底。
  ///
  /// 走渲染面协议 `stage-bg`（`canvas` 的 CSS `background-image: cover`），
  /// **与舞台纯色底共存**：有图时盖住底色，清除后底色回来。
  /// 所以「纯黑/纯白舞台」与「自定义展台图」不是二选一，而是叠放关系。
  ///
  /// 超过 [kStageImageMaxChars] 的图**不进这里**（会毁掉整份偏好的写入），
  /// 那种情况只在会话内生效。
  final String? stageImage;

  /// 壳自己的背景图（dataURL）。**只在 `syncShellStageBg == false` 时使用**——
  /// 同步开着的时候壳画的就是 [stageImage]（一份真相，见 [effectiveShellImage]）。
  ///
  /// 2026-09-14（rc.5）：壳（聊天 / 侧栏背后的整片区域）也可以有一张背景图，
  /// 固定以低透明度铺在壳根，**不做**分区背景。超过 [kShellImageMaxChars]
  /// 的图同样读不进来（理由同 [stageImage]）。
  final String? shellImage;

  /// 壳背景是否**跟随舞台**（默认 `true`）。
  ///
  /// 开着时 [shellImage] 不参与渲染：壳画 [stageImage]，并且「壳背景」那一行的
  /// 选图 / 清图改的也是 [stageImage]——所以舞台与壳永远只有**一张图**，
  /// 不存在「舞台换了、壳还是旧的」这种两份真相。
  final bool syncShellStageBg;

  /// **实际要画的**壳背景图。
  ///
  /// 同步开 → 舞台那张（共用一份真相）；同步关 → 壳自己那张。
  /// 渲染层只读这个 getter，不自己去判 `syncShellStageBg`——
  /// 判据只有一处，才不会出现两处不一致。
  String? get effectiveShellImage => syncShellStageBg ? stageImage : shellImage;

  /// 舞台背景轮播列表（用户手动维护，dataURL，缺省空）。
  ///
  /// 图列表住在这里——与 [stageImage] 同一条 localStorage 记录、同一份长度
  /// 预算（[kStageImageMaxChars] / [kStagePlaylistMaxChars]）。用户在「外观与
  /// 互动」里增删 / 排序；当前舞台那张仍是 [stageImage]，两者数据相等时由
  /// [stagePlaylistIndexOf] 标出来。
  ///
  /// 列表为空是**合法状态**（就是「没有轮播图」）。三项预算：每项 ≤
  /// [kStageImageMaxChars]、总长 ≤ [kStagePlaylistMaxChars]、
  /// 项数 ≤ [kStagePlaylistMaxItems]。
  final List<String> stagePlaylist;

  /// 模型缩放（渲染面 `stage-config.scale`，同区间）。
  final double scale;

  /// 口型灵敏度：乘在渲染面「RMS → dB 映射」的结果上。
  ///
  /// 默认 [defaultMouthSensitivity] = 1.0 是**实测标定值**：真实 TTS 语音的
  /// 线性 RMS p90 ≈ 0.118，经 dB 映射后约开到 0.58（见
  /// `crates/l2d-wasm-demo/src/mouth.rs` 的标定表）。
  final double mouthSensitivity;

  /// 口型总开关。
  final bool lipSync;

  /// 空闲生命体征（呼吸/眨眼/微表情）。
  final bool idleEnabled;

  /// 是否静音（**只关声音，不关口型**）。
  ///
  /// 默认 `false`（2026-09-10 用户裁决**改为默认出声**）：产品默认就该能听见，
  /// 否则用户会以为 TTS 坏了。需要安静时由用户按静音，或跑测试/无人值守时用
  /// 服务端 `LIVE2D_AI_MUTE_AUDIO=1` 强制静音（那样任何客户端都发不出声）。
  final bool muted;

  /// 主音量（0..1，1 = 满音量）。**与 [muted] 正交**：静音时不看它，
  /// 且不重置它——取消静音后应恢复用户原来的音量。
  ///
  /// 只影响「听不听得到」，不影响口型：口型由服务端 `volume` / 本地包络驱动。
  /// 滑杆位置到线性增益是**非线性**换算（感知压缩），见 `audio/gain.dart`。
  final double volume;

  /// 是否允许**拖动与滚轮缩放**模型（渲染面协议 `sync.clickEnabled`）。
  ///
  /// # 文案必须是「允许拖动与缩放」，不是「点击互动」
  ///
  /// 这个开关在渲染面关掉的是**拖拽 / 滚轮 / 双击复位**，与「点击角色有没有
  /// 反应」无关。同类项目里就因为把它叫成「点击互动」而让用户以为点角色会
  /// 没反应（调研 control §9-7）。默认 `true`（保持既有手感）。
  final bool allowDragZoom;

  /// 渲染档位（4096 / 8192 / 16384）。
  ///
  /// **只在开发者模式里暴露**：它是性能与画质的权衡，需要档位知识
  /// （规格 §4.1.5 的 A 组把它列为 dev）。
  final int tier;

  /// 默认档位。
  static const int defaultTier = 8192;

  /// 合法档位（**唯一真源**，下拉/分段与 clamp 都读它）。
  static const List<int> tiers = <int>[4096, 8192, 16384];

  /// 主音量默认值与区间。
  static const double defaultVolume = 1.0;
  static const double minVolume = 0.0;
  static const double maxVolume = 1.0;

  /// 缩放默认值与区间（与渲染面 clamp 一致）。
  static const double defaultScale = 1.0;
  static const double minScale = 0.5;
  static const double maxScale = 2.0;

  /// 口型灵敏度默认值与区间。
  ///
  /// 上限取 3.0（而非渲染面的 4.0）：再往上普通语音会长期贴着满幅，看起来像穿模。
  static const double defaultMouthSensitivity = 1.0;
  static const double minMouthSensitivity = 0.2;
  static const double maxMouthSensitivity = 3.0;

  DisplayPrefs copyWith({
    AppThemeId? theme,
    // 刻意**不能**用 `copyWith()` 把背景图设成「还是原值」——清除走
    // `clearStageImage: true`，这样 `copyWith()` 不会因为「参数缺省 = null」
    // 而把「设成 null（清除）」和「不改」混成同一件事。
    String? stageImage,
    bool clearStageImage = false,
    String? shellImage,
    bool clearShellImage = false,
    bool? syncShellStageBg,
    List<String>? stagePlaylist,
    double? scale,
    double? mouthSensitivity,
    bool? lipSync,
    bool? idleEnabled,
    bool? muted,
    double? volume,
    bool? allowDragZoom,
    int? tier,
  }) {
    return DisplayPrefs(
      theme: theme ?? this.theme,
      stageImage: clearStageImage ? null : (stageImage ?? this.stageImage),
      shellImage: clearShellImage ? null : (shellImage ?? this.shellImage),
      syncShellStageBg: syncShellStageBg ?? this.syncShellStageBg,
      stagePlaylist: stagePlaylist ?? this.stagePlaylist,
      scale: scale ?? this.scale,
      mouthSensitivity: mouthSensitivity ?? this.mouthSensitivity,
      lipSync: lipSync ?? this.lipSync,
      idleEnabled: idleEnabled ?? this.idleEnabled,
      muted: muted ?? this.muted,
      volume: volume ?? this.volume,
      allowDragZoom: allowDragZoom ?? this.allowDragZoom,
      tier: tier ?? this.tier,
    );
  }

  /// 序列化（写入 localStorage）。
  Map<String, Object?> toJson() => <String, Object?>{
    'theme': theme.wire,
    'stageImage': stageImage,
    'shellImage': shellImage,
    'syncShellStageBg': syncShellStageBg,
    'stagePlaylist': stagePlaylist,
    'scale': scale,
    'mouthSensitivity': mouthSensitivity,
    'lipSync': lipSync,
    'idleEnabled': idleEnabled,
    'muted': muted,
    'volume': volume,
    'allowDragZoom': allowDragZoom,
    'tier': tier,
  };

  /// 反序列化：**永不抛异常**。
  ///
  /// 缺字段 / 类型不符 / 非有限数 / 越界值一律回落到默认值或夹到区间内——
  /// 一份坏的存储不该让舞台无法渲染（这正是单测覆盖的部分）。
  factory DisplayPrefs.fromJson(Map<String, Object?>? json) {
    if (json == null) return const DisplayPrefs();
    return DisplayPrefs(
      // 旧存档没有这个字段 → 用默认（黑），不是「假装用户选过白」。
      theme: AppThemeId.fromWire(json['theme']),
      // 坏值（非字符串 / 超限）一律当作「没有背景图」，**不抛**：
      // 一份被外部塞大的存储不该让舞台渲染不出来。
      stageImage: _readImage(json['stageImage'], kStageImageMaxChars),
      shellImage: _readImage(json['shellImage'], kShellImageMaxChars),
      // 旧存档没有这个字段 → 默认跟随舞台（不是「各画各的」）。
      syncShellStageBg: _readBool(json['syncShellStageBg'], true),
      // 列表：坏值 / 超预算的项**逐项丢弃**，绝不因为一条坏数据把整份偏好判死
      // （`browser_io` 那层已经保证「读不出来就回落默认」）。
      stagePlaylist: readStagePlaylist(json['stagePlaylist']),
      scale: clampScale(_readDouble(json['scale'], defaultScale)),
      mouthSensitivity: clampMouthSensitivity(
        _readDouble(json['mouthSensitivity'], defaultMouthSensitivity),
      ),
      lipSync: _readBool(json['lipSync'], true),
      idleEnabled: _readBool(json['idleEnabled'], true),
      muted: _readBool(json['muted'], false),
      volume: clampVolume(_readDouble(json['volume'], defaultVolume)),
      // 旧存档没有这两个字段 → 用新默认值（不是 false/0）。
      allowDragZoom: _readBool(json['allowDragZoom'], true),
      tier: clampTier(_readInt(json['tier'], defaultTier)),
    );
  }

  /// 缩放夹到 `[minScale, maxScale]`；非有限值回落默认。
  static double clampScale(double value) {
    if (!value.isFinite) return defaultScale;
    return value.clamp(minScale, maxScale);
  }

  /// 口型灵敏度夹到 `[minMouthSensitivity, maxMouthSensitivity]`；非有限值回落默认。
  static double clampMouthSensitivity(double value) {
    if (!value.isFinite) return defaultMouthSensitivity;
    return value.clamp(minMouthSensitivity, maxMouthSensitivity);
  }

  /// 主音量夹到 `[minVolume, maxVolume]`；非有限值回落默认（满音量）。
  static double clampVolume(double value) {
    if (!value.isFinite) return defaultVolume;
    return value.clamp(minVolume, maxVolume);
  }

  static double _readDouble(Object? raw, double fallback) {
    if (raw is num) {
      final value = raw.toDouble();
      if (value.isFinite) return value;
    }
    return fallback;
  }

  static bool _readBool(Object? raw, bool fallback) =>
      raw is bool ? raw : fallback;

  /// 读背景图：非字符串 / 空串 / 超过 [maxChars] 都当作没有。
  static String? _readImage(Object? raw, int maxChars) {
    if (raw is! String || raw.isEmpty) return null;
    if (raw.length > maxChars) return null;
    return raw;
  }

  /// 读轮播列表：**逐项宽容**，超出预算的部分保留前面的、丢掉放不下的。
  ///
  /// 规则（三条预算各自独立生效，坏值只丢自己那一项）：
  /// 1. 非 `List` → 空列表；项非字符串 / 空串 / 单项超 [kStageImageMaxChars] → 丢该项；
  /// 2. 项数达到 [kStagePlaylistMaxItems] → 停止（后面的丢掉）；
  /// 3. 累计字符数会把总长推过 [kStagePlaylistMaxChars] → 停止（同上）。
  static List<String> readStagePlaylist(Object? raw) {
    if (raw is! List) return const <String>[];
    final List<String> out = <String>[];
    int total = 0;
    for (final Object? item in raw) {
      if (out.length >= kStagePlaylistMaxItems) break;
      if (item is! String || item.isEmpty) continue;
      if (item.length > kStageImageMaxChars) continue;
      if (total + item.length > kStagePlaylistMaxChars) break;
      out.add(item);
      total += item.length;
    }
    return out;
  }

  /// 读 int（非数字回落默认）。
  static int _readInt(Object? raw, int fallback) =>
      raw is num ? raw.toInt() : fallback;

  @override
  bool operator ==(Object other) =>
      other is DisplayPrefs &&
      other.theme == theme &&
      other.stageImage == stageImage &&
      other.shellImage == shellImage &&
      other.syncShellStageBg == syncShellStageBg &&
      _sameList(other.stagePlaylist, stagePlaylist) &&
      other.scale == scale &&
      other.mouthSensitivity == mouthSensitivity &&
      other.lipSync == lipSync &&
      other.idleEnabled == idleEnabled &&
      other.muted == muted &&
      other.volume == volume &&
      other.allowDragZoom == allowDragZoom &&
      other.tier == tier;

  @override
  int get hashCode => Object.hash(
    theme,
    stageImage,
    shellImage,
    syncShellStageBg,
    Object.hashAll(stagePlaylist),
    scale,
    mouthSensitivity,
    lipSync,
    idleEnabled,
    muted,
    volume,
    allowDragZoom,
    tier,
  );

  @override
  String toString() =>
      'DisplayPrefs(theme: ${theme.wire}, stageImage: ${stageImage?.length ?? 0} chars, '
      'shellImage: ${shellImage?.length ?? 0} chars, syncShell: $syncShellStageBg, '
      'playlist: ${stagePlaylist.length} items, '
      'scale: $scale, mouth: $mouthSensitivity, '
      'lipSync: $lipSync, idle: $idleEnabled, muted: $muted, '
      'volume: $volume, allowDragZoom: $allowDragZoom, tier: $tier)';
}

/// 把档位夹到 [DisplayPrefs.tiers] 里的**最近合法值**。
///
/// 不是简单把区间夹住：`10000` 落在 8192 与 16384 之间，夹成区间边界会得到
/// 一个渲染面根本不认识的档位。取最近合法档位才是「宽容但正确」。
int clampTier(int value) {
  int best = DisplayPrefs.tiers.first;
  int bestDistance = (value - best).abs();
  for (final int tier in DisplayPrefs.tiers) {
    final int distance = (value - tier).abs();
    if (distance < bestDistance) {
      best = tier;
      bestDistance = distance;
    }
  }
  return best;
}

/// 两个列表是否逐项相等（不引 Flutter 的 foundation 库，
/// 保持本文件「纯 Dart、VM 可测」的定位）。
bool _sameList(List<String> a, List<String> b) {
  if (identical(a, b)) return true;
  if (a.length != b.length) return false;
  for (int i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

/// 「加入轮播」的结果：**新列表 + 是否加进去了 + 没加的原因**。
///
/// 原因用稳定字符串（不是给人看的完整句子）：UI 只负责把原因翻译成一句
/// 可读文案，纯逻辑测试直接断言原因——文案会改，原因不该跟着改。
class StagePlaylistAppendResult {
  const StagePlaylistAppendResult({
    required this.playlist,
    required this.added,
    required this.reason,
  });

  /// 返回的列表（成功时是 `current + [dataUrl]`；失败时**原样返回** `current`——
  /// 绝不「悄悄膨胀」）。
  final List<String> playlist;

  /// 是否真的加进去了。
  final bool added;

  /// `added` / `empty` / `item_too_large` / `limit_reached` / `budget_exceeded`。
  final String reason;
}

/// 把一张图追加进轮播列表（纯函数；三条预算逐条把关）。
///
/// 超限时**不修改**列表并给出原因——调用方据此给用户一句可读反馈，
/// 而不是让列表无界膨胀到把整份偏好写坏（见 [kStagePlaylistMaxChars]）。
StagePlaylistAppendResult appendToStagePlaylist(
  List<String> current,
  String? dataUrl,
) {
  if (dataUrl == null || dataUrl.isEmpty) {
    return StagePlaylistAppendResult(
      playlist: current,
      added: false,
      reason: 'empty',
    );
  }
  if (dataUrl.length > kStageImageMaxChars) {
    return StagePlaylistAppendResult(
      playlist: current,
      added: false,
      reason: 'item_too_large',
    );
  }
  if (current.length >= kStagePlaylistMaxItems) {
    return StagePlaylistAppendResult(
      playlist: current,
      added: false,
      reason: 'limit_reached',
    );
  }
  int total = 0;
  for (final String item in current) {
    total += item.length;
  }
  if (total + dataUrl.length > kStagePlaylistMaxChars) {
    return StagePlaylistAppendResult(
      playlist: current,
      added: false,
      reason: 'budget_exceeded',
    );
  }
  return StagePlaylistAppendResult(
    playlist: <String>[...current, dataUrl],
    added: true,
    reason: 'added',
  );
}

/// 第 [index] 张在列表里的下标；找不到（含 [stageImage] 为 null）→ `-1`。
///
/// UI 用它把「当前舞台那张」标出来。判据是**数据相等**（同一份 dataURL）：
/// 列表可以被删除 / 重排，用数据比对才不会在编辑列表后指错人。
int stagePlaylistIndexOf(List<String> playlist, String? stageImage) {
  if (stageImage == null) return -1;
  for (int i = 0; i < playlist.length; i++) {
    if (playlist[i] == stageImage) return i;
  }
  return -1;
}

/// 删除第 [index] 张（Wave 3）。
///
/// - 越界（负数 / `>= length`）→ **原样返回**入参（不抛、不猜）；
/// - 成功 → 返回**新列表**（不改入参；删除只会让总长变小，三条预算不会被破坏）；
/// - 不动 [DisplayPrefs.stageImage]：删除当前张不会清空舞台——用户看到的那张
///   仍然在屏幕上，是否还在列表里由调用方决定。
List<String> removeStagePlaylistAt(List<String> current, int index) {
  if (index < 0 || index >= current.length) return current;
  return <String>[...current]..removeAt(index);
}

/// 把第 [from] 张移到 [to]（都是 0-based 下标，闭区间）。
///
/// - 越界（`from` / `to` 不在 `0..length`）/ `from == to` → **原样返回**入参；
/// - 成功 → 返回**重排后的新列表**（项集合与项数都不变，所以三条预算不受影响）。
///
/// 「上移」= `moveStagePlaylist(list, i, i - 1)`，「下移」= `(list, i, i + 1)`。
List<String> moveStagePlaylist(List<String> current, int from, int to) {
  if (from < 0 || from >= current.length) return current;
  if (to < 0 || to >= current.length) return current;
  if (from == to) return current;
  final List<String> out = <String>[...current];
  final String item = out.removeAt(from);
  out.insert(to, item);
  return out;
}
