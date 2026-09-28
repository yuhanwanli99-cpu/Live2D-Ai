/// 舞台显示偏好：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 单独成文件的理由：这些值会被持久化（localStorage）并在启动时反序列化，
/// 「非法/缺失/越界输入回落到什么」是必须能回归的算术，不该埋在 UI 里。
/// 持久化本身在 `app/browser_io.dart` 做（那里才允许 `package:web`）。
///
/// # 行数（**超出豁免带**，拆分归 Stage C3）
///
/// 本文件在本轮（Stage B · B-a）之后约 1160 行，**超过「源码 ≤500 行、
/// 豁免 ≤1000 行」那条线**。这不是本轮才超的：rc.5 时已是 1002 行
/// （`docs/releases/v0.2.0-rc.5.md` §9.4 已把拆分登记为 Stage C3），
/// 本轮只加不减。写在这里是**如实**，不是豁免申请。
library;

import '../design/background_item.dart';
import '../design/theme_id.dart';

/// 背景库最多几项。
///
/// 8 是内存与观感的折中：每一项的字节都常驻内存（base64 字符串 + 轮播时
/// 解码出的位图），8 张 4 MB 的图约 64 MB，再多就开始影响启动与切图。
const int kBackgroundMaxCount = 8;

/// **单张**背景图的字节上限（24 MB，按**解码后**的字节算，不是 base64 字符）。
///
/// # 为什么还有上限，而 2026-09-27 之前是 390 KB
///
/// 之前那个数（`kBackgroundImageMaxChars = 400000`）不是安全边际，
/// 是**事故**：它让「选一张 1080p 照片」永远失败——实测 3 张图一张都存不进去。
///
/// 现在的 24 MB 不是配额保护（配额在 IndexedDB 那里，量大得多），
/// 而是**防手抖**：有人把一个 200 MB 的 TIFF 拖进来，base64 之后
/// 字符串本身就能把标签页的内存吃穿，那会拖垮整个应用而不只是背景。
/// 24 MB 已经远大于任何真实壁纸。
const int kBackgroundImageMaxBytes = 24 * 1024 * 1024;

/// 背景库字节占用（**解码后**的字节；图案零成本）。
int backgroundBytesOf(BackgroundItem item) {
  if (item is! BackgroundImage) return 0;
  final String? url = item.dataUrl;
  if (url == null) return 0;
  // base64：4 个字符 = 3 个字节。整数运算（不 import dart:convert），
  // 免得这个纯逻辑文件多一条依赖。
  final int comma = url.indexOf(',');
  final int payload = comma >= 0 ? url.length - comma - 1 : url.length;
  return (payload / 4 * 3).floor();
}

/// dataURL 大致折成多少字节（**纯算术，可 VM 单测**）。
int dataUrlBytes(String dataUrl) {
  final int comma = dataUrl.indexOf(',');
  final int payload = comma >= 0 ? dataUrl.length - comma - 1 : dataUrl.length;
  return (payload / 4 * 3).floor();
}


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
/// 这一轮刻意不引入（见 `docs/verification/flutter-shell-manifest-checklist.md`）。
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

  DisplayPrefs copyWith({
    AppThemeId? theme,
    // 刻意**不能**用 `copyWith()` 把背景图设成「还是原值」——清除走
    // `clearStageImage: true`，这样 `copyWith()` 不会因为「参数缺省 = null」
    // 而把「设成 null（清除）」和「不改」混成同一件事。
    String? stageImage,
    bool clearStageImage = false,
    List<BackgroundItem>? backgrounds,
    int? backgroundSource,
    double? backgroundOpacity,
    double? backgroundBlur,
    int? backgroundScrim,
    bool? backgroundEnabled,
    int? imageFit,
    int? imageAlign,
    double? tileSize,
    int? slideInterval,
    bool? slideRandom,
    double? uiTransparency,
    List<String>? stagePlaylist,
    double? scale,
    double? mouthSensitivity,
    bool? lipSync,
    bool? idleEnabled,
    bool? muted,
    double? volume,
    bool? allowDragZoom,
    int? tier,
    double? edgeStrength,
  }) {
    return DisplayPrefs(
      theme: theme ?? this.theme,
      stageImage: clearStageImage ? null : (stageImage ?? this.stageImage),
      backgrounds: backgrounds ?? this.backgrounds,
      backgroundSource: backgroundSource ?? this.backgroundSource,
      backgroundOpacity: backgroundOpacity ?? this.backgroundOpacity,
      backgroundBlur: backgroundBlur ?? this.backgroundBlur,
      backgroundScrim: backgroundScrim ?? this.backgroundScrim,
      backgroundEnabled: backgroundEnabled ?? this.backgroundEnabled,
      imageFit: imageFit ?? this.imageFit,
      imageAlign: imageAlign ?? this.imageAlign,
      tileSize: tileSize ?? this.tileSize,
      slideInterval: slideInterval ?? this.slideInterval,
      slideRandom: slideRandom ?? this.slideRandom,
      uiTransparency: uiTransparency ?? this.uiTransparency,
      stagePlaylist: stagePlaylist ?? this.stagePlaylist,
      scale: scale ?? this.scale,
      mouthSensitivity: mouthSensitivity ?? this.mouthSensitivity,
      lipSync: lipSync ?? this.lipSync,
      idleEnabled: idleEnabled ?? this.idleEnabled,
      muted: muted ?? this.muted,
      volume: volume ?? this.volume,
      allowDragZoom: allowDragZoom ?? this.allowDragZoom,
      tier: tier ?? this.tier,
      edgeStrength: edgeStrength ?? this.edgeStrength,
    );
  }

  /// 序列化（写入 localStorage）。
  Map<String, Object?> toJson() => <String, Object?>{
    'theme': theme.wire,
    'stageImage': stageImage,
    'backgrounds': <Object?>[
      for (final BackgroundItem b in backgrounds) b.toJson(),
    ],
    'backgroundSource': backgroundSource,
    'backgroundOpacity': backgroundOpacity,
    'backgroundBlur': backgroundBlur,
    'backgroundScrim': backgroundScrim,
    'backgroundEnabled': backgroundEnabled,
    'imageFit': imageFit,
    'imageAlign': imageAlign,
    'tileSize': tileSize,
    'slideInterval': slideInterval,
    'slideRandom': slideRandom,
    'uiTransparency': uiTransparency,
    'stagePlaylist': stagePlaylist,
    'scale': scale,
    'mouthSensitivity': mouthSensitivity,
    'lipSync': lipSync,
    'idleEnabled': idleEnabled,
    'muted': muted,
    'volume': volume,
    'allowDragZoom': allowDragZoom,
    'tier': tier,
    'edgeStrength': edgeStrength,
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
      backgrounds: readBackgrounds(json),
      backgroundSource: _readBackgroundSource(json),
      backgroundOpacity: clampBackgroundOpacity(
        _readDouble(json['backgroundOpacity'], defaultBackgroundOpacity),
      ),
      backgroundBlur: clampBackgroundBlur(
        _readDouble(json['backgroundBlur'], defaultBackgroundBlur),
      ),
      backgroundScrim: _clampInt(
        _readInt(json['backgroundScrim'], defaultBackgroundScrim),
        minBackgroundScrim,
        maxBackgroundScrim,
        defaultBackgroundScrim,
      ),
      // 旧档没有这个键 → `true`（迁移默认：老用户界面不变）。
      backgroundEnabled: _readBool(json['backgroundEnabled'], true),
      // 上界 3（Stage B 扩档）：旧档里的 2/3 现在是**合法档**，不再是坏值。
      imageFit: _clampInt(
        _readInt(json['imageFit'], defaultImageFit),
        0,
        maxImageFit,
        defaultImageFit,
      ),
      imageAlign: _clampInt(
        _readInt(json['imageAlign'], defaultImageAlign),
        0,
        maxImageAlign,
        defaultImageAlign,
      ),
      tileSize: clampTileSize(_readDouble(json['tileSize'], defaultTileSize)),
      // DEC-1：**端点夹持**（0 仍＝关），不是「越界回落默认」——
      // 回落 0 会把用户开着的轮播静默关掉（详见 [clampSlideInterval]）。
      slideInterval: clampSlideInterval(
        _readInt(json['slideInterval'], defaultSlideInterval),
      ),
      slideRandom: _readBool(json['slideRandom'], false),
      uiTransparency: clampUiTransparency(
        _readDouble(json['uiTransparency'], defaultUiTransparency),
      ),
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
      // 旧存档没有这两个字段 → 默认 1.0（不是「假装用户调过」）。
      edgeStrength: clampEdgeStrength(
        _readDouble(json['edgeStrength'], defaultEdgeStrength),
      ),
    );
  }

  /// 读「壳画哪一张」，**含旧字段迁移**。
  ///
  /// 旧形态的 `syncShellStageBg` 是「壳跟随舞台」，默认 `true`。迁移规则：
  ///
  /// - **确实有一张舞台图**（`stageImage != null`）→ 迁到「舞台那张」，
  ///   老用户**看到的界面一个像素都不变**；
  /// - **没有舞台图**（绝大多数用户，包括把图加进背景库的那些）→ 迁到
  ///   「背景库」，也就是新默认。
  ///
  /// 写成「读旧键」而不是「改旧键的默认值」：改默认值会让「显式设过 true」
  /// 和「从没设过」分不开，而那正是本缺陷的成因。
  static int _readBackgroundSource(Map<String, Object?> json) {
    final Object? stored = json['backgroundSource'];
    if (stored is int) {
      return stored == backgroundSourceStageImage
          ? backgroundSourceStageImage
          : backgroundSourceLibrary;
    }
    final bool legacySync = _readBool(json['syncShellStageBg'], false);
    final String? stage = _readImage(json['stageImage'], kStageImageMaxChars);
    return legacySync && stage != null
        ? backgroundSourceStageImage
        : backgroundSourceLibrary;
  }

  /// 壳当前**实际会画**的那一项（判据只有这一处）。
  ///
  /// 渲染层不再自己判来源——判据散到两处就会出现「设置说用背景库、
  /// 画的不是背景库」这类不一致。
  BackgroundItem? get effectiveBackground {
    if (backgroundSource == backgroundSourceStageImage) {
      final String? stage = stageImage;
      // 舞台那张的 id 是**固定**的：它不走字节库（走渲染面 `stage-bg` 协议），
      // 用一个常量 id 而不是内容哈希，免得每帧重算一张几 MB 的哈希。
      return stage == null
          ? null
          : BackgroundImage(id: kStageImageItemId, dataUrl: stage);
    }
    if (backgrounds.isEmpty) return null;
    return backgrounds[_clampIndex(0, backgrounds.length - 1)];
  }

  /// 壳当前会画的**下标**（来源是背景库时才有意义）。
  int get effectiveBackgroundIndex =>
      backgroundSource == backgroundSourceLibrary
      ? _clampIndex(0, backgrounds.length - 1)
      : 0;

  /// 当前项**是不是一张图片**（图案不算）。
  ///
  /// 「铺法 / 位置」这两个控件只对图片有意义：图案是 `CustomPainter`
  /// 画满整块，没有「原图尺寸」可裁可留边。UI 按它决定显不显示那两项。
  bool get currentItemIsImage => effectiveBackground is BackgroundImage;

  /// 有没有东西**真的画得出来**（**唯一判据**，全仓库只此一处）。
  ///
  /// # 为什么判据是这三个而不是两个条件
  ///
  /// 2026-09-27 之前这里只有 `opacity > 0 && effectiveBackground != null`，
  /// 而渲染层（`AppShell`）**另写了一份**判据，两份还不一样：
  /// 那边多了一条 `opacity < 1`。于是：
  ///
  /// - 滑杆推到 100% 的那一刻，设置页的说明还写着「聊天面板会跟着透」，
  ///   实际面板已经退回不透明 —— **界面在骗人**（P4）；
  /// - 任何新写的代码都得先回答「我该信哪一个」。
  ///
  /// 现在只剩这一个函数，渲染层传自己的当前项进来（它知道轮播索引）。
  /// 加上 [backgroundEnabled]：DEC-4 的全局开关在这里收口——
  /// 关掉开关后，所有「有没有背景」的判断（脚手架底让不让、聊天面板透不透）
  /// 都跟着回到「没有背景」那一态，不必每个调用点各写一遍。
  ///
  /// 不透明度读的是 [effectiveImageOpacity]（逐图 ?? 全局）：某一项显式覆盖过
  /// 不透明度时，判据必须与渲染层**看同一个值**，否则又会出现「设置说有背景、
  /// 画面按没有」。全局 0 而这一项覆盖成 > 0 ⇒ 照画（用户的明确指令优先）。
  bool hasBackgroundAt(BackgroundItem? item) =>
      backgroundEnabled &&
      item != null &&
      item.isRenderable &&
      effectiveImageOpacity(item, backgroundOpacity) > 0;

  /// 逐图覆盖 ?? 全局：**不透明度**（唯一解析点）。
  ///
  /// 渲染层（`ShellBackdrop`）与 [hasBackgroundAt] 都读它——判据与画面共用
  /// 一个解析，才不会出现两套。图案没有逐图样式 ⇒ 一律回落全局。
  static double effectiveImageOpacity(BackgroundItem? item, double global) =>
      (item is BackgroundImage ? item.opacity : null) ?? global;

  /// 逐图覆盖 ?? 全局：**铺法**（唯一解析点）。
  static int effectiveImageFit(BackgroundItem? item, int global) =>
      (item is BackgroundImage ? item.fit : null) ?? global;

  /// 逐图覆盖 ?? 全局：**位置**（唯一解析点）。
  static int effectiveImageAlign(BackgroundItem? item, int global) =>
      (item is BackgroundImage ? item.align : null) ?? global;

  /// 本偏好自己那一项有没有背景（[effectiveBackground] 版本）。
  bool get hasBackground => hasBackgroundAt(effectiveBackground);

  static int _clampIndex(int index, int max) =>
      max < 0 ? 0 : index.clamp(0, max);

  /// 读背景库：数量超了截断、坏项直接丢、**永不抛**。
  ///
  /// **逐项的大小上限已经不在这里**（2026-09-27）：字节住 IndexedDB，
  /// 偏好里只剩 id，本地已经没什么可超的了。留下的只有「最多几项」。
  ///
  /// 顺带做**旧字段迁移**：`shellImage`（2026-09-14 rc.5 的单张 `String?`）
  /// → 单元素列表。写回时**不再**写 `shellImage`，所以这是一次性迁移。
  static List<BackgroundItem> readBackgrounds(Map<String, Object?> json) {
    final List<BackgroundItem> out = <BackgroundItem>[];
    final Object? raw = json['backgrounds'];
    if (raw is List) {
      for (final Object? entry in raw) {
        if (out.length >= kBackgroundMaxCount) break;
        final BackgroundItem? item = BackgroundItem.fromJson(entry);
        if (item == null) continue;
        out.add(item);
      }
      return List<BackgroundItem>.unmodifiable(out);
    }
    final String? legacy = _readImage(json['shellImage'], kLegacyImageMaxChars);
    return legacy == null
        ? const <BackgroundItem>[]
        : <BackgroundItem>[
            // id 同样由内容算 ⇒ 这张图与背景库里同一张图会被认成同一项。
            BackgroundImage(id: backgroundIdOf(legacy), dataUrl: legacy),
          ];
  }

  /// 读**旧**单张字段时的字符上限。
  ///
  /// 为什么还有它：旧档里的 base64 已经在那里了，判据只能是它的长度。
  /// 这个数取旧的 `kBackgroundImageMaxChars`（390 KB）——**只**用于读旧档，
  /// 新的导入路径完全不读它（见 [kBackgroundImageMaxBytes]）。
  static const int kLegacyImageMaxChars = 400000;

  /// 背景库当前的**字节占用**（只有画得出来的图片算，图案零成本）。
  ///
  /// 2026-09-27 之前这里叫 `backgroundsChars`，算的是 base64 字符数，
  /// 用来对照 localStorage 的配额。现在它**不控制任何上限**——
  /// 配额在 IndexedDB 那里，浏览器说了算。它现在只用来**显示**占用。
  static int backgroundsBytes(List<BackgroundItem> items) {
    int total = 0;
    for (final BackgroundItem item in items) {
      total += backgroundBytesOf(item);
    }
    return total;
  }

  /// 这一项**能不能加进来**（数量上限 + 单张体积上限）。**纯函数**。
  ///
  /// 刻意**没有**「总预算」这一关：字节不再与偏好共用配额，
  /// 总量由浏览器按磁盘剩余空间判（真满了 [BackgroundStore.put] 会返回
  /// `false`，那时才如实告诉用户）。在前端**猜**一个总上限，
  /// 只会把「还早得很」误报成「加不进去了」。
  static bool canAddBackground(
    List<BackgroundItem> current,
    BackgroundItem item,
  ) {
    if (current.length >= kBackgroundMaxCount) return false;
    if (item is! BackgroundImage) return true;
    return backgroundBytesOf(item) <= kBackgroundImageMaxBytes;
  }

  /// 背景不透明度夹到区间；非有限数回落默认。
  static double clampBackgroundOpacity(double value) {
    if (!value.isFinite) return defaultBackgroundOpacity;
    return value.clamp(minBackgroundOpacity, maxBackgroundOpacity);
  }

  /// 界面透明度夹到 `[0, 1]`；非有限数回落默认（不透明）。
  static double clampUiTransparency(double value) {
    if (!value.isFinite) return defaultUiTransparency;
    return value.clamp(0.0, maxUiTransparency);
  }

  /// 背景模糊夹到 `[0, maxBackgroundBlur]`；非有限数回落默认（不模糊）。
  static double clampBackgroundBlur(double value) {
    if (!value.isFinite) return defaultBackgroundBlur;
    return value.clamp(0.0, maxBackgroundBlur);
  }

  /// 平铺贴片边长夹到 `[minTileSize, maxTileSize]`；非有限数回落默认。
  ///
  /// 这里用**端点夹持**（与 scale / volume 同一条纪律）：它是数值区间，
  /// 端点没有枚举语义，夹到端点永远比「静默换一个值」更接近用户意图。
  static double clampTileSize(double value) {
    if (!value.isFinite) return defaultTileSize;
    return value.clamp(minTileSize, maxTileSize);
  }

  /// 整数夹到区间；越界回落**默认值**（而不是区间端点）。
  ///
  /// 为什么不是端点：`scrim` 的 0 是 `auto`、1 是「无」，把一个坏值夹到 1
  /// 会把用户的背景变得不可读；回落默认才是「不知道就按默认来」。
  ///
  /// **本轮不动它**（DEC-1 明确）：要端点夹持的字段用 [_clampIntToRange]。
  static int _clampInt(int value, int min, int max, int fallback) {
    if (value < min || value > max) return fallback;
    return value;
  }

  /// 整数**端点夹持**（DEC-1 新增；与 [_clampInt] 并列、语义不同）。
  ///
  /// 为什么另开一个而不是改 [_clampInt]：那个函数的「越界回落默认」是
  /// `scrim` 的语义，改掉会把用户的背景变得不可读。
  static int _clampIntToRange(int value, int min, int max) {
    if (value < min) return min;
    if (value > max) return max;
    return value;
  }

  /// 轮播间隔的**端点夹持**（DEC-1；rc.5 §9.1 那条未决项的落地）。
  ///
  /// 规则：`0` 仍＝**关**（定时器不启动）；其余一律夹进 `[5, 300]`。
  ///
  /// | 存 | 0 | 1 | 2 | 4 | 5 | 30 | 300 | 301 | 3600 | 99999 | 负数 |
  /// | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
  /// | 读 | 0 | 5 | 5 | 5 | 5 | 30 | 300 | 300 | 300 | 300 | 5 |
  ///
  /// # 为什么不沿用「越界回落默认」
  ///
  /// `slideInterval` 是**数值区间**（像 scale / volume），而它的默认值恰好是
  /// `0 = 关`：越界回落默认 = **把用户开着的轮播静默关掉**（rc.5 §9.1）。
  /// 夹到端点则是「继续开着，只是间隔落回合法区间」，符合用户意图。
  ///
  /// 负数也夹到 5（而不是 0）：存储里出现负数只可能是坏值，把它解释成
  /// 「关掉用户开着的东西」比「用一个合法间隔继续开着」更武断。这一条是
  /// DEC-1「<5 → 5」的**字面执行**，不是自行裁决。
  static int clampSlideInterval(int value) {
    if (value == 0) return 0;
    return _clampIntToRange(
      value,
      minSlideIntervalSeconds,
      maxSlideIntervalSeconds,
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

  /// 描边强度夹到区间；非有限值回落默认（不缩放）。
  static double clampEdgeStrength(double value) {
    if (!value.isFinite) return defaultEdgeStrength;
    return value.clamp(minEdgeStrength, maxEdgeStrength);
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
      if (!backgrounds[i].sameAs(other.backgrounds[i])) return false;
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
