/// 背景库的一项：**要么是一张图，要么是一个内置图案**。
///
/// # 为什么是一个判别联合，而不是两个平行的 List
///
/// 背景库是一个**有序列表**（用户要能拖动排序、要能删第 3 张、要轮播）。
/// 两个平行数组（`List<String> images` + `List<int> patterns`）没法表达
/// 「这张图排在那张渐变前面」——要么放弃排序，要么每次排序要同时动两个数组
/// 并处理长度不一致。判别联合让**顺序、删除、去重、轮播**都只作用在一个列表上。
///
/// # 为什么图案要进这个列表
///
/// 程序化图案（渐变 / 网格 / 光斑）**零存储、零网络、任意分辨率不糊**。
/// 让它和图片同列，用户就能把「渐变」和「照片」排在一起轮播；
/// 它同时也是**存储配额被图片吃满时的安全退路**（图案不计入字符预算）。
///
/// # 纯逻辑
///
/// 本文件**只**依赖 `dart:core`，因此可以在 VM 上直接单测
/// （`DisplayPrefs` 的持久化逻辑与它一起被 `test/display_prefs_test.dart` 覆盖）。
/// 背景图片的**身份**：由内容算出的短 id。
///
/// # 为什么身份要由**内容**决定
///
/// 背景库的字节搬去 IndexedDB（见 `data/background_store.dart`）之后，
/// 偏好里只剩一个「这一项是谁」。这个 id 同时承担三件事，**所以它必须稳定
/// 且不靠外部计数器**：
///
/// 1. **去重**：同一张图导入两次应该是同一项，不是两项；
/// 2. **迁移**：旧存档里存的是 dataURL，搬进 IndexedDB 时需要一个键；
/// 3. **删得干净**：从库里移除一项要能指出「删 IndexedDB 里的哪条」。
///
/// 用「内容哈希」而不是「时间戳 + 计数器」：后者在**两次导入同一张图**时会
/// 造出两个 id，于是同一张图占两份空间——而用户加同一张图两次是常事
/// （换机器、换浏览器、误以为没加上又点了一次）。
///
/// # 为什么不用 FNV-1a / xxHash / `Object.hash`
///
/// **web 上 Dart 的 `int` 是 double**：位运算超过 2^53 之后 `&` 拿到的是
/// 舍入后的值，同一个字符串在 VM（真 64 位整型）与 web 上会算出**两个不同
/// 的 id**——「图不见了」就是这么来的。`Object.hash` 更糟：按平台而变。
///
/// 所以这里用 djb2 + 素数取模，**全程不超过 2^37**：
///
/// - `h < 2^31`，`h * 33 + c < 2^31 * 33 + 2^16 < 2^37`；
/// - 2^37 远小于 2^53，double 能**逐位精确**表示每一个中间值；
/// - 于是 VM 与 web 算出来的 id **逐字符相同**。
///
/// 代价是模数只有 31 位。**碰撞概率**：n 张图的生日碰撞约 `n² / 2^32`，
/// 8 张 ≈ 1.5e-5。就算撞上也只会让两张不同的图共用一个 id（后导入的覆盖
/// 先导入的），不会崩、不会丢别的项。
library;

/// 背景库的一项。
sealed class BackgroundItem {
  const BackgroundItem();

  /// 落进 JSON 用的判别键（`image` / `pattern`）。
  String get kind;

  /// 序列化。
  Map<String, Object?> toJson();

  /// 反序列化：**永不抛**。不认识或坏掉的一律返回 `null`（调用方丢弃这一项）。
  static BackgroundItem? fromJson(Object? raw) {
    if (raw is! Map) return null;
    final Object? kind = raw['kind'];
    return switch (kind) {
      'image' => BackgroundImage.fromJson(raw.cast<Object?, Object?>()),
      'pattern' => BackgroundPattern.fromJson(raw.cast<Object?, Object?>()),
      _ => null,
    };
  }

  /// 两个项「是不是同一个」——用于去重。
  ///
  /// 图片按 [BackgroundImage.id] 比；图案按 id 比。
  bool sameAs(BackgroundItem other);

  /// 这一项**现在画得出来吗**。
  ///
  /// 图案永远为 `true`（它由 [CustomPainter] 现画，没有外部依赖）；
  /// 图片看字节在不在手（见 [BackgroundImage.isRenderable]）。
  bool get isRenderable;
}

/// 一张导入的图片：**身份 + 字节**。
///
/// # 为什么是「一个 id + 可选的字节」，而不是「一段 dataURL」
///
/// 2026-09-27 改：字节搬去了 IndexedDB（见 `data/background_store.dart`），
/// 偏好里只留一个 id。于是每一项有两副面孔：
///
/// | | 谁在用 | 落不落盘 |
/// | --- | --- | --- |
/// | [id] | 偏好 / 去重 / 删除 / IndexedDB 的键 | **落** |
/// | [dataUrl] | 渲染层（`Image.memory`）/ 缩略图 | **不落** |
///
/// [dataUrl] 为 `null` 的项 = 「知道有这么一张图，但字节还没读回来」。
/// **只有** [hydrateBackgrounds] 允许这种项存在：能补的补齐、存储
/// **明确回答「没有」**的摘掉；而**读失败**时它原样保留（宁可这一次画不出来，
/// 也绝不因为一次瞬时故障就把可能还在库里的图删掉）。渲染层用
/// [isRenderable] 判据跳过画不出的项。
///
/// # 为什么 [toJson] **只**写 id
///
/// 写了 dataURL 就等于把图又塞回 localStorage，整个搬库就白做了。
/// 这是本文件最重要的一行，回归在 `test/display_prefs_test.dart`
/// （「清单里不许出现 dataUrl」）。
///
/// # 旧档怎么办
///
/// 2026-09-27 之前的存档里是 `{'kind':'image','dataUrl':'…'}`。
/// 读它时**两个字段都用**（id 由内容算出来，见 `design/background_id.dart`），
/// 写回时只写 id —— 一次启动就完成搬家。
final class BackgroundImage extends BackgroundItem {
  const BackgroundImage({required this.id, this.dataUrl});

  /// 身份（由内容算出，见 `backgroundIdOf`）；也是字节库里的键。
  final String id;

  /// 渲染用字节。**不落盘**（见类头注的表）。
  final String? dataUrl;

  /// 现在能不能画。
  ///
  /// 为什么要有这个判据而不是到处 `dataUrl != null`：
  /// 「这一项存在」与「这一项画得出来」是**两件事**——
  /// 字节库打不开时，库里可以有图而一张都画不出来。
  /// 「背景库：3 项」但屏幕上一张都没有，比报错更糟。
  @override
  bool get isRenderable => dataUrl != null && dataUrl!.isNotEmpty;

  @override
  String get kind => 'image';

  @override
  Map<String, Object?> toJson() => <String, Object?>{
    'kind': 'image',
    'id': id,
  };

  static BackgroundImage? fromJson(Map<Object?, Object?> raw) {
    final Object? rawId = raw['id'];
    if (rawId is String && rawId.isNotEmpty) {
      return BackgroundImage(id: rawId, dataUrl: _legacyBytes(raw));
    }
    // 旧档：只有 dataUrl。id 由内容算 ⇒ 同一张图前后一致。
    final String? url = _legacyBytes(raw);
    if (url == null) return null;
    return BackgroundImage(id: backgroundIdOf(url), dataUrl: url);
  }

  /// 旧档里那一段（可能不存在 / 不合法）。
  static String? _legacyBytes(Map<Object?, Object?> raw) {
    final Object? url = raw['dataUrl'];
    return url is String && url.isNotEmpty ? url : null;
  }

  @override
  bool sameAs(BackgroundItem other) =>
      other is BackgroundImage && other.id == id;

  /// 按 [id] 判等，**不看 [dataUrl]**。
  ///
  /// 为什么：同一张图在「字节读回来之前」与「之后」必须判为**同一项**，
  /// 否则「刚导入」这一次 [DisplayPrefs] 比较会判成「变了」，
  /// 连带把整个偏好重写一遍，而重写又要重新序列化所有图。
  @override
  bool operator ==(Object other) =>
      other is BackgroundImage && other.id == id;

  @override
  int get hashCode => Object.hash('image', id);

  @override
  String toString() =>
      'BackgroundImage($id, ${isRenderable ? '${dataUrl!.length} chars' : 'bytes pending'})';
}

/// 一个内置图案（`BackgroundPatternId` 里的枚举值）。
final class BackgroundPattern extends BackgroundItem {
  const BackgroundPattern(this.id);

  final int id;

  @override
  String get kind => 'pattern';

  @override
  bool get isRenderable => true;

  @override
  Map<String, Object?> toJson() => <String, Object?>{
    'kind': 'pattern',
    'id': id,
  };

  static BackgroundPattern? fromJson(Map<Object?, Object?> raw) {
    final Object? id = raw['id'];
    if (id is! int || !BackgroundPatternId.isValid(id)) return null;
    return BackgroundPattern(id);
  }

  @override
  bool sameAs(BackgroundItem other) =>
      other is BackgroundPattern && other.id == id;

  @override
  bool operator ==(Object other) =>
      other is BackgroundPattern && other.id == id;

  @override
  int get hashCode => Object.hash('pattern', id);

  @override
  String toString() => 'BackgroundPattern($id)';
}

/// 内置图案的取值表（**唯一真源**：判合法、判循环边界、判单测枚举都读它）。
///
/// 顺序即设置里图库里的**显示顺序**。
abstract final class BackgroundPatternId {
  /// 线性渐变（左上 → 右下，两色由主题 accent 与面色混合而来）。
  static const int gradient = 0;

  /// 径向光晕（三团大光晕，位置与半径固定 → 稳定、不会呼吸）。
  static const int glow = 1;

  /// 细网格（7 px 点阵，**固定随机种子** → 不会每帧重排）。
  static const int grid = 2;

  /// 斜细纹（45°，低对比）。
  static const int stripes = 3;

  /// 全部取值。
  static const List<int> values = <int>[gradient, glow, grid, stripes];

  /// 是否是合法取值。
  static bool isValid(int value) => values.contains(value);
}

/// 图案的中文名（设置里显示；**不用图标**，用户裁决「尽量少用图片」）。
String backgroundPatternLabel(int id) => switch (id) {
  BackgroundPatternId.gradient => '渐变',
  BackgroundPatternId.glow => '光晕',
  BackgroundPatternId.grid => '网格',
  BackgroundPatternId.stripes => '斜纹',
  _ => '未知图案',
};

/// 取模用的素数（`2^31 - 1`，梅森素数）。
///
/// 必须是奇素数：`2^31 - 1` 恰好是奇数，且 33 与它互质 ⇒ 哈希不退化。
const int kBackgroundHashModulus = 0x7FFFFFFF;

/// djb2 变体：`h = h * 33 + c`（mod [kBackgroundHashModulus]）。
int backgroundFingerprint(String text) {
  int h = 5381;
  for (int i = 0; i < text.length; i++) {
    h = (h * 33 + text.codeUnitAt(i)) % kBackgroundHashModulus;
  }
  return h;
}

/// 一张背景图的 id（也是它在字节库里的键）。
///
/// 形如 `bg1a2b3c4d`：**带前缀**，这样它在日志 / IndexedDB 里一眼能认出
/// 「这是背景字节，不是别的东西」。
///
/// [dataUrl] 用 dataURL 整体（含 `data:image/png;base64,` 头）参与哈希，
/// 不是只用 base64 段——**同一张图但换了 MIME 头**（浏览器对不同扩展名
/// 给的类型不同）应当算两张不同的记录，行为上更可预期。
String backgroundIdOf(String dataUrl) =>
    'bg${backgroundFingerprint(dataUrl).toRadixString(16).padLeft(8, '0')}';
