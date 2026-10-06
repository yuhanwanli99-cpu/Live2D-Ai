/// 舞台显示偏好：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 单独成文件的理由：这些值会被持久化（localStorage）并在启动时反序列化，
/// 「非法/缺失/越界输入回落到什么」是必须能回归的算术，不该埋在 UI 里。
/// 持久化本身在 app/browser_io.dart 做（那里才允许 package:web）。
///
/// # 行数（2026-10-06 拆 part 后）
///
/// 本文件拆分前是 1169 行（rc.5 起就超过「源码 ≤500 行、豁免 ≤1000 行」那条线，
/// 登记为 Stage C3）。2026-10-06 按 docs/plans/DECISION-display-prefs-2026-10-06.md
/// 的 **B2** 方案拆成「同库 part + extension 外搬方法」（与 E1-b 拆 main.dart 同款配方）：
///
/// | part | 内容 |
/// | --- | --- |
/// | display_prefs_playlist.dart | 顶层常量 / 字节算术 / 轮播列表增删排序 |
/// | display_prefs_codec.dart | toJson / fromJson 与读入宽容（坏值也永不抛） |
/// | display_prefs_derived.dart | effective* / hasBackground* 派生判据 |
/// | display_prefs_limits.dart | clamp* 区间夹持 + 字节预算 |
/// | display_prefs_copy.dart | copyWith |
///
/// 搬迁是**逐字**的（剪贴 + extension 头），只补了 extension 作用域必需的
/// DisplayPrefs.xxx 类静态限定符。下列成员**刻意留在类里**：
///
/// - 24 个字段、const 构造、全部 static const（Dart 规定**类体不能跨 part**；
///   且 198 处调用点读的是 DisplayPrefs.defaultXxx 这类类静态）；
/// - == / hashCode / toString：它们是 Object 成员，写进 extension 会
///   **静默失效**（extension 只在静态类型没有该成员时才生效）；
/// - 一行转发包装：保住既有静态调用点的公共名字。
///
/// 落盘 24 键的全集守卫见 test/display_prefs_persist_keys_test.dart。
library;

import '../design/background_item.dart';
import '../design/theme_id.dart';

part 'display_prefs_playlist.dart';
part 'display_prefs_codec.dart';
part 'display_prefs_derived.dart';
part 'display_prefs_limits.dart';
part 'display_prefs_copy.dart';

/// 舞台显示偏好。
class DisplayPrefs {
  const DisplayPrefs({
    this.theme = AppThemeId.fallback,
    this.stageImage,
    this.backgrounds = const <BackgroundItem>[],
    this.backgroundSource = backgroundSourceLibrary,
    this.backgroundOpacity = defaultBackgroundOpacity,
    this.backgroundBlur = defaultBackgroundBlur,
    this.backgroundScrim = defaultBackgroundScrim,
    this.backgroundEnabled = defaultBackgroundEnabled,
    this.imageFit = defaultImageFit,
    this.imageAlign = defaultImageAlign,
    this.tileSize = defaultTileSize,
    this.slideInterval = defaultSlideInterval,
    this.slideRandom = false,
    this.uiTransparency = defaultUiTransparency,
    this.stagePlaylist = const <String>[],
    this.scale = defaultScale,
    this.mouthSensitivity = defaultMouthSensitivity,
    this.lipSync = true,
    this.idleEnabled = true,
    this.muted = false,
    this.volume = defaultVolume,
    this.allowDragZoom = true,
    this.tier = defaultTier,
    this.edgeStrength = defaultEdgeStrength,
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

  /// 舞台背景图 dataURL（单张，走渲染面协议 `stage-bg`）。
  ///
  /// `null` = 不用背景图，只有纯色底。**只影响舞台**，与 [backgrounds] 无关。
  ///
  /// 超过 [kStageImageMaxChars] 的图**不进这里**（会毁掉整份偏好的写入），
  /// 那种情况只在会话内生效。
  final String? stageImage;

  /// **背景库**：有序的一串「图 / 内置图案」，壳根铺它，轮播按它走。
  ///
  /// 2026-09-27 从「单张 `shellImage`」升级而来。判别联合
  /// （`BackgroundImage` / `BackgroundPattern`）而不是两个平行数组——
  /// 见 `design/background_item.dart` 的头注。
  ///
  /// 空列表 = 没有背景，壳就是纯色面（与本轮之前「没选图」的观感一致）。
  final List<BackgroundItem> backgrounds;

  /// **壳画哪一张**：背景库，还是舞台那张单图。
  ///
  /// # 为什么换掉 `syncShellStageBg`（2026-09-27 修一个真缺陷）
  ///
  /// 原来的形态是「壳跟随舞台」，默认 `true`。于是：
  ///
  /// - 用户去**背景库**加图 → 壳却去读 [stageImage]（从没设过 → `null`）
  ///   → **加了 3 张图，界面一点变化都没有**；
  /// - 而 UI 在那个状态下**把图库藏起来、只留「添加图片」按钮**，
  ///   点下去还会提示「已加进背景库」——**一个必然无效的按钮**。
  ///
  /// 那是本项目 P4 明令禁止的「静默失效」。修法不是加提示，而是**把默认
  /// 换过来**：背景库是唯一真相，舞台那张图降级成一个**可选来源**。
  ///
  /// 铺法 / 位置 / 透明度 / 模糊 / 遮罩 / 过渡在**两种来源下都生效**——
  /// 它们是渲染参数，不是图片来源。
  final int backgroundSource;

  /// 背景不透明度。
  ///
  /// # 默认为什么是 `1.0` 而不是过去的 `0.15`（2026-09-27 改）
  ///
  /// 0.15 是 rc.5 那个「壳全局背景固定 0.15」的遗留值，它服务的是
  /// **一张装饰性底纹**，不是「用户亲手挑的一张照片」。
  /// 用户加了图却只看到 15% 的淡影 —— 那正是「做了和没做一样」。
  /// 现在的默认是「你选的图，就是你看到的图」；**需要压暗时拉「遮罩」**，
  /// 那才是管可读性的那个旋钮。
  ///
  /// 区间到 `1.0`：0 = 完全不画背景（回到纯色面）——**除非**某一项自己
  /// 覆盖过不透明度（`BackgroundImage.opacity`）：那是对「这一项」的明确
  /// 指令，优先于全局（见 [effectiveImageOpacity]）。
  final double backgroundOpacity;

  /// 背景模糊半径（px）。
  ///
  /// 只作用在**壳自己画的背景**上（舞台是 `<iframe>` 平台视图，模糊不到）。
  /// 0–8：这个量级上背景仍然认得出是「一张图」，再大就只是色块。
  final double backgroundBlur;

  /// 可读性遮罩档位（`0=auto` / `1=无` / `2=轻` / `3=重`）。
  final int backgroundScrim;

  /// **全局背景开关**（DEC-4）：`false` = 不画图，只留底色。
  ///
  /// 与 [backgroundOpacity] = 0 的区别是**可逆**：关掉开关不动用户的透明度
  /// 设置，重新打开就是他原来看到的样子。
  final bool backgroundEnabled;

  /// 图片铺法（`0=cover` / `1=contain` / `2=stretch` / `3=tile`）。
  final int imageFit;

  /// 图片位置（3×3 九宫格索引，`0=左上` … `4=居中` … `8=右下`）。
  final int imageAlign;

  /// 平铺的贴片边长（逻辑像素；**只对 [imageFit] = 3 有效**）。
  ///
  /// 为什么放在全局而不是逐图：逐图覆盖只做 opacity / fit / align 三项
  /// （parity §6 的短期目标），贴片大小是「铺法参数」，跟着 [imageFit] 走。
  final double tileSize;

  /// 轮播间隔（秒；`0` = 不轮播）。
  final int slideInterval;

  /// 轮播是否随机顺序（**不会连续两张相同**）。
  final bool slideRandom;

  /// **界面本身的透明程度**（0 = 完全不透明，1 = 尽量透）。
  ///
  /// 与 [backgroundOpacity] 是**两个不同的轴**，别混：
  ///
  /// | | 管的是 | 0 时 | 1 时 |
  /// | --- | --- | --- | --- |
  /// | `backgroundOpacity` | **图**本身有多实 | 看不见图 | 压满 |
  /// | `uiTransparency` | **面板**有多透 | 面板不透明 | 面板半透（露出背后的图） |
  ///
  /// # 默认为什么是 `0.5` 而不是 `0`（2026-09-27 改）
  ///
  /// 这一条是「背景加了等于没加」的**真正原因**，不是背景自己的问题：
  ///
  /// 背景铺在**壳根**，而它上面压着 AppBar、聊天面板、设置面板 ——
  /// 全部是面。`uiTransparency = 0` 时那些面**完全不透明**，
  /// 于是背景图只在「面之间的缝隙」里露出几个像素：
  /// **用户加了一张壁纸，整个界面一个像素都没变。**
  ///
  /// 默认 0.5 → 面板 alpha 0.775，于是图在聊天面板下透出约 15% ——
  /// 刚好是「认得出有张图、又不影响读字」的那一档。
  ///
  /// **没有背景时这一项只影响设置面板**（聊天面板不透明，见
  /// `ChatPanel.backdropVisible`）：背后是纯色底，把面板做半透
  /// 只会让整块界面发灰，而不是「透出背景」。
  final double uiTransparency;

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

  /// **描边强度**（2026-09-27）：`AppColors.hairline` 透明度的缩放系数。
  ///
  /// 默认 `1.0`。调低会让界面更「轻」（发丝线几乎消失，靠三级面差分层）；
  /// 调高适合大屏 / 亮度高的显示器。
  ///
  /// **不参与**的：焦点环（`focusRing`）与危险描边（`dangerBorder`）直出
  /// `accent` / `danger`，不走 `hairline`——它们是可用性下限，
  /// 不是审美旋钮（见 `theme_palette_test` 的对比度断言）。
  final double edgeStrength;

  /// 默认档位。
  static const int defaultTier = 8192;

  /// 合法档位（**唯一真源**，下拉/分段与 clamp 都读它）。
  static const List<int> tiers = <int>[4096, 8192, 16384];

  /// 描边强度：默认值与区间。
  ///
  /// 下限不是 0：全 0 时发丝线**完全消失**，界面只剩色块没有边界——
  /// 那不是「更轻的界面」，是坏掉的界面。0.4 保留刚好可辨的一线。
  static const double defaultEdgeStrength = 1.0;
  static const double minEdgeStrength = 0.4;
  static const double maxEdgeStrength = 1.5;

  // ── 背景系统（2026-09-27）──

  /// 背景不透明度：默认值与区间。
  ///
  /// 区间真源是 [BackgroundStyleRange]（全局与逐图覆盖共用一套）。
  static const double defaultBackgroundOpacity =
      BackgroundStyleRange.maxOpacity;
  static const double minBackgroundOpacity = BackgroundStyleRange.minOpacity;
  static const double maxBackgroundOpacity = BackgroundStyleRange.maxOpacity;

  /// 全局背景开关（DEC-4；默认**开**）。
  ///
  /// 关掉 = **不画图**（底色照旧），而不是「把不透明度调成 0」：
  /// 前者是「这一阵子先不显示背景」，后者会把用户调好的透明度抹掉。
  /// 旧档没有这个键 → 迁移成 `true`（老用户界面一个像素都不变）。
  static const bool defaultBackgroundEnabled = true;

  /// 平铺（`imageFit = 3`）的**贴片边长**（逻辑像素）。
  ///
  /// 只对 `tile` 有效：cover / contain / stretch 下它不参与渲染。
  ///
  /// 区间 16–256 的理由：小于 16 照片变成噪点（认不出是那张图）；
  /// 大于 256 一屏只剩几块，那已经是 `cover` 的观感，不再是「平铺」。
  static const double defaultTileSize = 64.0;
  static const double minTileSize = 16.0;
  static const double maxTileSize = 256.0;

  /// 背景模糊：默认值与区间（px）。
  static const double defaultBackgroundBlur = 0.0;
  static const double maxBackgroundBlur = 8.0;

  /// 遮罩档位（**0 必须是 auto**——存/不存的语义不同）。
  static const int defaultBackgroundScrim = 0;
  static const int minBackgroundScrim = 0;
  static const int maxBackgroundScrim = 3;

  /// 铺法（**0 必须是 cover**）。
  ///
  /// # 上界为什么从 1 放到 3（2026-09-28 · Stage B · B-a）
  ///
  /// 旧注释写的「渲染层只实现了两档」在 `shell_backdrop.dart` 补齐
  /// `stretch` / `tile` 之后**已失效**，那个假上限连同它的理由一起删掉：
  ///
  /// | 值 | 名字 | 渲染 |
  /// | --- | --- | --- |
  /// | 0 | `cover` | `Image(fit: BoxFit.cover)` |
  /// | 1 | `contain` | `Image(fit: BoxFit.contain)` |
  /// | 2 | `stretch` | `FittedBox(fit: BoxFit.fill)` 包一层（拉伸铺满） |
  /// | 3 | `tile` | `ImageRepeat.repeat`，贴片边长见 [tileSize] |
  ///
  /// **迁移语义**：旧档里出现的 `2` / `3` 由「坏值回落 0」变成**合法档**——
  /// 以前会被静默改成「铺满」的两档，现在按用户当初的意图生效；
  /// 区间外的值（`4` / `42` / 负数 / 非 int）仍走 [_clampInt] 回落默认 0。
  ///
  /// 区间常量住在 [BackgroundStyleRange]：全局与逐图覆盖共用一套，不会各自
  /// 长一个上界。
  static const int defaultImageFit = fitCover;
  static const int maxImageFit = BackgroundStyleRange.maxFit;

  /// 铺法取值（**唯一真源**：UI 的选项、渲染层的分支、存储的 clamp 都读它）。
  static const int fitCover = 0;
  static const int fitContain = 1;
  static const int fitStretch = 2;
  static const int fitTile = 3;

  /// 九宫格位置（**4 必须是居中**——默认就该是什么都不改的样子）。
  static const int defaultImageAlign = 4;
  static const int maxImageAlign = BackgroundStyleRange.maxAlign;

  /// 轮播间隔（秒；**0 = 不轮播**）。
  static const int defaultSlideInterval = 0;

  /// 轮播开启时，间隔滑杆的两端（秒）。
  ///
  /// `0` 与它们不是一回事：`0` = **关掉轮播**（定时器不启动），
  /// 而 5 秒是「开着轮播」时能选的最小间隔——再小会让定时器疯狂重入。
  ///
  /// **上界同时是存储 clamp 的上界**（`fromJson` 用它夹 `slideInterval`）。
  /// 2026-09-27 之前这里另有一个 `maxSlideInterval = 3600`：滑杆最多只到
  /// 300，而存储允许 3600 ⇒ 一份 3600 的存量偏好会让滑块贴在最右、读数
  /// 却写 3600，**控件与读数说两套**。统一成一个上界之后
  /// 「能存的」= 「能选的」。
  static const int minSlideIntervalSeconds = 5;
  static const int maxSlideIntervalSeconds = 300;

  /// 壳画**背景库**（默认，唯一真相）。
  static const int backgroundSourceLibrary = 0;

  /// 壳画**舞台那张单图**（rc.5 兼容形态：一张图，壳与舞台共用）。
  static const int backgroundSourceStageImage = 1;

  /// 「舞台那张」在背景库语义下的固定 id。
  ///
  /// 那一项**不存字节库**（它走渲染面 `stage-bg` 协议，且随偏好落盘），
  /// 所以 id 不参与任何字节库操作——固定值就够，且省掉每帧对一张
  /// 几 MB 的 dataURL 做哈希。
  static const String kStageImageItemId = 'bgstage';

  /// 界面透明度：默认值与区间。
  static const double defaultUiTransparency = 0.5;
  static const double minUiTransparency = 0.0;
  static const double maxUiTransparency = 1.0;

  /// 面板不透明度的**下界**（`uiTransparency = 1` 时取到它）。
  ///
  /// 为什么要下界而不是 0：0 意味着面板完全消失，聊天文字直接落在图上——
  /// 那不是「更透的界面」，是坏掉的界面。这条与
  /// `minEdgeStrength`（发丝线不许归零）是同一条纪律。
  static const double minPanelAlpha = 0.55;

  /// 九宫格位置 → 对齐（**纯函数，可 VM 单测**）。
  ///
  /// 索引 0..8 按行优先：0 左上 · 1 上 · 2 右上 · 3 左 · **4 中** ·
  /// 5 右 · 6 左下 · 7 下 · 8 右下。
  static const List<({double x, double y})> alignTable =
      <({double x, double y})>[
        (x: -1, y: -1),
        (x: 0, y: -1),
        (x: 1, y: -1),
        (x: -1, y: 0),
        (x: 0, y: 0),
        (x: 1, y: 0),
        (x: -1, y: 1),
        (x: 0, y: 1),
        (x: 1, y: 1),
      ];

  /// 铺法 → `BoxFit` 的**名字**（不在纯逻辑层 import material）。
  ///
  /// 为什么要转一手：`display_prefs.dart` 刻意**不 import Flutter**，
  /// 这样它能在纯 Dart 的 VM 测试里跑（见文件头注）。渲染层按名字翻译回去。
  static String fitName(int fit) => switch (fit) {
    fitContain => 'contain',
    fitStretch => 'stretch',
    fitTile => 'tile',
    _ => 'cover',
  };

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

  /// 读**旧**单张字段时的字符上限。
  ///
  /// 为什么还有它：旧档里的 base64 已经在那里了，判据只能是它的长度。
  /// 这个数取旧的 `kBackgroundImageMaxChars`（390 KB）——**只**用于读旧档，
  /// 新的导入路径完全不读它（见 [kBackgroundImageMaxBytes]）。
  static const int kLegacyImageMaxChars = 400000;

  // ── 静态 API 面：本体在 part 的 extension 里；这些一行转发只为保住既有
  //    调用点与公共名字（lib 既有静态调用 + test 多处 + 常量读取都不动）。 ──

  /// 反序列化：**永不抛异常**。本体在 display_prefs_codec.dart 的
  /// [DisplayPrefsCodec.decode]（保留 factory 形状 ⇒ 既有调用点不变）。
  factory DisplayPrefs.fromJson(Map<String, Object?>? json) =>
      DisplayPrefsCodec.decode(json);

  /// 背景库读取。本体：[DisplayPrefsCodec.readBackgrounds]。
  static List<BackgroundItem> readBackgrounds(Map<String, Object?> json) =>
      DisplayPrefsCodec.readBackgrounds(json);

  /// 轮播列表读取。本体：[DisplayPrefsCodec.readStagePlaylist]。
  static List<String> readStagePlaylist(Object? raw) =>
      DisplayPrefsCodec.readStagePlaylist(raw);

  /// 逐图覆盖 ?? 全局：不透明度。本体：[DisplayPrefsDerived.effectiveImageOpacity]。
  static double effectiveImageOpacity(BackgroundItem? item, double global) =>
      DisplayPrefsDerived.effectiveImageOpacity(item, global);

  /// 逐图覆盖 ?? 全局：铺法。本体：[DisplayPrefsDerived.effectiveImageFit]。
  static int effectiveImageFit(BackgroundItem? item, int global) =>
      DisplayPrefsDerived.effectiveImageFit(item, global);

  /// 逐图覆盖 ?? 全局：位置。本体：[DisplayPrefsDerived.effectiveImageAlign]。
  static int effectiveImageAlign(BackgroundItem? item, int global) =>
      DisplayPrefsDerived.effectiveImageAlign(item, global);

  /// 背景库字节占用。本体：[DisplayPrefsLimits.backgroundsBytes]。
  static int backgroundsBytes(List<BackgroundItem> items) =>
      DisplayPrefsLimits.backgroundsBytes(items);

  /// 这一项能不能加进来。本体：[DisplayPrefsLimits.canAddBackground]。
  static bool canAddBackground(
    List<BackgroundItem> current,
    BackgroundItem item,
  ) => DisplayPrefsLimits.canAddBackground(current, item);

  /// 区间夹持与读入校验的九个 clamp。本体都在 display_prefs_limits.dart。
  static double clampBackgroundOpacity(double value) =>
      DisplayPrefsLimits.clampBackgroundOpacity(value);
  static double clampUiTransparency(double value) =>
      DisplayPrefsLimits.clampUiTransparency(value);
  static double clampBackgroundBlur(double value) =>
      DisplayPrefsLimits.clampBackgroundBlur(value);
  static double clampTileSize(double value) =>
      DisplayPrefsLimits.clampTileSize(value);
  static int clampSlideInterval(int value) =>
      DisplayPrefsLimits.clampSlideInterval(value);
  static double clampScale(double value) => DisplayPrefsLimits.clampScale(value);
  static double clampMouthSensitivity(double value) =>
      DisplayPrefsLimits.clampMouthSensitivity(value);
  static double clampVolume(double value) =>
      DisplayPrefsLimits.clampVolume(value);
  static double clampEdgeStrength(double value) =>
      DisplayPrefsLimits.clampEdgeStrength(value);

  @override
  bool operator ==(Object other) {
    if (other is! DisplayPrefs) return false;
    if (other.theme != theme ||
        other.stageImage != stageImage ||
        other.backgroundSource != backgroundSource ||
        other.backgroundEnabled != backgroundEnabled ||
        other.backgroundOpacity != backgroundOpacity ||
        other.backgroundBlur != backgroundBlur ||
        other.backgroundScrim != backgroundScrim ||
        other.imageFit != imageFit ||
        other.imageAlign != imageAlign ||
        other.tileSize != tileSize ||
        other.slideInterval != slideInterval ||
        other.slideRandom != slideRandom ||
        other.uiTransparency != uiTransparency ||
        other.scale != scale ||
        other.mouthSensitivity != mouthSensitivity ||
        other.lipSync != lipSync ||
        other.idleEnabled != idleEnabled ||
        other.muted != muted ||
        other.volume != volume ||
        other.allowDragZoom != allowDragZoom ||
        other.tier != tier ||
        other.edgeStrength != edgeStrength) {
      return false;
    }
    if (backgrounds.length != other.backgrounds.length) return false;
    for (int i = 0; i < backgrounds.length; i++) {
      // **用 `!=`（含逐图样式），不要用 id-only 的 `sameAs`**（F-0034-01，
      // 审计 2026-09-28；P2 在线缺陷）：
      //
      // `sameAs` 的语义是「**字节读回前后算同一项**」（只看 id）——那是给
      // 水合去重用的，不是给「变了没有」用的。把它当通用判据会让「只改了
      // 逐图铺法 / 不透明度 / 位置」被判成「什么都没变」⇒ `_updatePrefs` 的
      // `if (next == widget.prefs) return;`（以及 `main.dart` 的 `_update`）
      // **静默丢掉**这次编辑：界面不动、刷新还原，且没有任何错误。
      //
      // `BackgroundImage.==` 与 `hashCode` 都含 `id + opacity + fit + align`
      // 且都不看 `dataUrl`，所以换用它既补上样式轴、又保持「字节没回来也
      // 算同一项」，还让 `==` 与 `hashCode` 重新对称。
      if (backgrounds[i] != other.backgrounds[i]) return false;
    }
    return _sameList(other.stagePlaylist, stagePlaylist);
  }

  @override
  int get hashCode => Object.hash(
    Object.hashAll(<Object?>[
      theme,
      stageImage,
      backgroundSource,
      backgroundOpacity,
      backgroundBlur,
      backgroundScrim,
      imageFit,
      imageAlign,
      Object.hashAll(stagePlaylist),
    ]),
    Object.hashAll(<Object?>[
      slideInterval,
      slideRandom,
      tileSize,
      backgroundEnabled,
      uiTransparency,
      scale,
      mouthSensitivity,
      lipSync,
      idleEnabled,
      muted,
      volume,
      allowDragZoom,
      tier,
      edgeStrength,
      ...backgrounds,
    ]),
  );

  @override
  String toString() =>
      'DisplayPrefs(theme: ${theme.wire}, stageImage: ${stageImage?.length ?? 0} chars, '
      'playlist: ${stagePlaylist.length} items, '
      'backgrounds: ${backgrounds.length} 项 / ${backgroundsBytes(backgrounds)} bytes, '
      'source: $backgroundSource, enabled: $backgroundEnabled, '
      'opacity: $backgroundOpacity, blur: $backgroundBlur, '
      'scrim: $backgroundScrim, fit: $imageFit, align: $imageAlign, '
      'tileSize: $tileSize, '
      'slide: ${slideInterval}s/${slideRandom ? 'random' : 'order'}, '
      'scale: $scale, mouth: $mouthSensitivity, '
      'lipSync: $lipSync, idle: $idleEnabled, muted: $muted, '
      'volume: $volume, allowDragZoom: $allowDragZoom, tier: $tier, '
      'edgeStrength: $edgeStrength)';

}
