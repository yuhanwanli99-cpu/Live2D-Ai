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

/// 一段串**是不是图片形态的 dataURL**（DEC-5 的**唯一判据**）。
///
/// # 为什么要有它（2026-09-28 收紧 isRenderable）
///
/// 旧判据是「非空串」，于是 `'x'`、被截断的记录、手改过的存储都算
/// 「画得出来」：界面说「有背景」，屏幕上**一张都没有**（P4 静默失效）。
/// 现在的形态要求是三条：
///
/// 1. 以 `data:image/` 开头（`data:text/plain` 之类的**不是**图片）；
/// 2. 有逗号（dataURL 元数据与 payload 的分界）；
/// 3. 逗号之后**非空**（空 payload 解出来是 0 字节，画不出来）。
///
/// 只判形态、**不判 base64 能不能解**：真正的解码在浏览器里
/// （`ui/shell_backdrop.dart` 的 `decodeDataUrlBytes`），而「形态合理但解不开」
/// 不该反向改写用户的数据（见 [BackgroundImageState]）。
bool isDataImageUrl(String url) {
  if (!url.startsWith('data:image/')) return false;
  final int comma = url.indexOf(',');
  if (comma < 0) return false;
  return comma + 1 < url.length;
}

/// 逐图样式覆盖的**合法区间**（唯一真源）。
///
/// # 为什么住在这里而不是 `settings/display_prefs.dart`
///
/// 依赖是单向的：`display_prefs.dart` import 本文件，本文件**不许**反向
/// import（会成环）。所以区间常量只能住这边，由 `DisplayPrefs` 引用
/// （`maxImageFit = BackgroundStyleRange.maxFit` 等），两边不会各长一套。
abstract final class BackgroundStyleRange {
  /// 铺法：`0=cover / 1=contain / 2=stretch / 3=tile`。
  static const int minFit = 0;
  static const int maxFit = 3;

  /// 九宫格位置：`0` 左上 … `4` 居中 … `8` 右下。
  static const int minAlign = 0;
  static const int maxAlign = 8;

  /// 不透明度。
  static const double minOpacity = 0.0;
  static const double maxOpacity = 1.0;
}

/// 一项图片的**三态**（UI 与渲染层共用的判据）。
///
/// | 态 | 含义 | 界面该说什么 |
/// | --- | --- | --- |
/// | [ready] | 字节在手里、形态是 dataURL 图片 | 正常（画） |
/// | [pending] | 字节还没从字节库读回来 | **不是错误**：瞬时态，别说成坏图 |
/// | [corrupt] | 字节在、但形态不是 dataURL 图片 | **要如实标出来**（重新导入） |
enum BackgroundImageState {
  /// 画得出来。
  ready,

  /// 等字节（水合之前 / 存储读失败时）。
  pending,

  /// 字节在、形态坏。
  corrupt,
}

/// # 逐图样式（2026-09-28 · Stage B）
///
/// [BackgroundImage.opacity] / [BackgroundImage.fit] / [BackgroundImage.align]
/// 是**可选覆盖**：`null` = 没设过 = 回落全局
/// （`DisplayPrefs.backgroundOpacity` / `imageFit` / `imageAlign`）。
/// 它们住在项上而不是另开一张表，是因为与「哪一项」同生命周期——
/// 删除、排序、轮播都只动一个列表。
///
/// 2026-10-07 起这三项不再落盘：`fromJson` 忽略旧键，`toJson` 不写它们。
/// 字段留在类上，水合仍用 `copyWith(dataUrl:)` 补字节。产品路径上它们保持 null。
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
  const BackgroundImage({
    required this.id,
    this.dataUrl,
    this.opacity,
    this.fit,
    this.align,
  });

  /// 身份（由内容算出，见 `backgroundIdOf`）；也是字节库里的键。
  final String id;

  /// 渲染用字节。**不落盘**（见类头注的表）。
  final String? dataUrl;

  /// 不透明度覆盖（`null` = 回落 `DisplayPrefs.backgroundOpacity`）。
  final double? opacity;

  /// 铺法覆盖（`null` = 回落 `DisplayPrefs.imageFit`）。
  final int? fit;

  /// 位置覆盖（`null` = 回落 `DisplayPrefs.imageAlign`）。
  final int? align;

  /// 这一项处在哪一态（见 [BackgroundImageState]）。
  BackgroundImageState get state {
    final String? url = dataUrl;
    if (url == null) return BackgroundImageState.pending;
    return isDataImageUrl(url)
        ? BackgroundImageState.ready
        : BackgroundImageState.corrupt;
  }

  /// 现在能不能画。
  ///
  /// 为什么要有这个判据而不是到处 `dataUrl != null`：
  /// 「这一项存在」与「这一项画得出来」是**两件事**——
  /// 字节库打不开时，库里可以有图而一张都画不出来。
  /// 「背景库：3 项」但屏幕上一张都没有，比报错更糟。
  ///
  /// # 2026-09-28 收紧（DEC-5）
  ///
  /// 旧判据是「非空串」，于是一段不是 dataURL 的串也算画得出来（界面在骗人）。
  /// 现在走 [isDataImageUrl]（形态三条）。
  ///
  /// **坏图怎么让 UI 知道**（外观区只读字段，不需要回调）：
  ///
  /// - [state] == [BackgroundImageState.corrupt] → 字节在但形态坏 ⇒
  ///   背景库那一项如实标出来（「这项字节有问题，重新导入」）；
  /// - [state] == [BackgroundImageState.pending] → 等字节，**不是**错误；
  /// - **形态合法但浏览器解不开**（编码不支持 / base64 坏了）只有渲染层知道：
  ///   `ShellBackdrop` 在那里静默回落底色，**不抛、不删项、不改数据**——
  ///   一次解码失败不构成「这张图坏了」，所以这一态**不进** [state]。
  @override
  bool get isRenderable => state == BackgroundImageState.ready;

  /// 字节在、但不是 dataURL 图片形态（要如实告诉用户的那一态）。
  bool get isCorrupt => state == BackgroundImageState.corrupt;

  /// 字节还没读回来（水合之前 / 存储读失败）。
  bool get bytesPending => state == BackgroundImageState.pending;

  /// 字段总数（供测试做「加了字段忘了 copyWith / toJson」的结构枚举）。
  ///
  /// 加字段时**必须**同时改这里、[toValuesMap] 与 `toJson`；测试对着它逐一
  /// 验证，漏一个就红（P4：漏字段 = 静默失效）。
  static const int kStructuralFieldCount = 5;

  /// 供结构枚举读的**字段名 → 值**（少一个字段，测试就该红）。
  Map<String, Object?> toValuesMap() => <String, Object?>{
    'id': id,
    'dataUrl': dataUrl,
    'opacity': opacity,
    'fit': fit,
    'align': align,
  };

  /// 复制并替换若干字段。
  ///
  /// [clearStyle] 是「把三项覆盖一起清掉」的显式开关——与
  /// `DisplayPrefs.copyWith(clearStageImage: …)` 同一个理由：参数缺省为 null
  /// 分不清「设成 null（= 回落全局）」与「不改」。
  ///
  /// **水合补字节必须走它**：`copyWith(dataUrl: read.dataUrl)`。直接 new 一个
  /// `BackgroundImage(id: …)` 会把逐图样式丢掉（`data/background_hydration.dart`）。
  BackgroundImage copyWith({
    String? id,
    String? dataUrl,
    double? opacity,
    int? fit,
    int? align,
    bool clearStyle = false,
  }) => BackgroundImage(
    id: id ?? this.id,
    dataUrl: dataUrl ?? this.dataUrl,
    opacity: clearStyle ? null : (opacity ?? this.opacity),
    fit: clearStyle ? null : (fit ?? this.fit),
    align: clearStyle ? null : (align ?? this.align),
  );

  @override
  String get kind => 'image';

  @override
  Map<String, Object?> toJson() => <String, Object?>{
    'kind': 'image',
    'id': id,
  };

  static BackgroundImage? fromJson(Map<Object?, Object?> raw) {
    // 逐图 opacity / fit / align：旧键忽略。铺法与遮罩已固定，不再按项覆盖。
    final Object? rawId = raw['id'];
    if (rawId is String && rawId.isNotEmpty) {
      return BackgroundImage(id: rawId, dataUrl: _legacyBytes(raw));
    }
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

  /// 按 [id]（+ 逐图样式）判等，**不看 [dataUrl]**。
  ///
  /// 为什么不看 [dataUrl]：同一张图在「字节读回来之前」与「之后」必须判为
  /// **同一项**，否则「刚导入」这一次 [DisplayPrefs] 比较会判成「变了」，
  /// 连带把整个偏好重写一遍，而重写又要重新序列化所有图。
  ///
  /// 为什么**要**看样式：样式是用户改出来的「这一项怎么画」。漏掉它会让
  /// 「改了逐图铺法」被判成「什么都没变」⇒ 不落盘（静默失效）。
  @override
  bool operator ==(Object other) =>
      other is BackgroundImage &&
      other.id == id &&
      other.opacity == opacity &&
      other.fit == fit &&
      other.align == align;

  @override
  int get hashCode => Object.hash('image', id, opacity, fit, align);

  @override
  String toString() {
    final String bytes = switch (state) {
      BackgroundImageState.ready => '${dataUrl!.length} chars',
      BackgroundImageState.pending => 'bytes pending',
      BackgroundImageState.corrupt => 'corrupt dataUrl',
    };
    final String style = (opacity == null && fit == null && align == null)
        ? 'global style'
        : 'opacity: $opacity, fit: $fit, align: $align';
    return 'BackgroundImage($id, $bytes, $style)';
  }
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
