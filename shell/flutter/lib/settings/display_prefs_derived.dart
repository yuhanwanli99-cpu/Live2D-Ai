/// DisplayPrefs 的**派生判据**（当前画哪一项 / 逐图覆盖解析，display_prefs.dart 的 part）。

///

/// 2026-10-06 按 B2 从 display_prefs.dart **逐字搬出**（只补 3 处 DisplayPrefs. 类静态限定符）。

/// 判据仍然**只有这一处**：渲染层不再自己判来源（判据散到两处就会出现

/// 「设置说用背景库、画的不是背景库」）。

part of 'display_prefs.dart';

extension DisplayPrefsDerived on DisplayPrefs {

  /// 壳当前**实际会画**的那一项（判据只有这一处）。
  ///
  /// 渲染层不再自己判来源——判据散到两处就会出现「设置说用背景库、
  /// 画的不是背景库」这类不一致。
  BackgroundItem? get effectiveBackground {
    if (backgroundSource == DisplayPrefs.backgroundSourceStageImage) {
      final String? stage = stageImage;
      // 舞台那张的 id 是**固定**的：它不走字节库（走渲染面 `stage-bg` 协议），
      // 用一个常量 id 而不是内容哈希，免得每帧重算一张几 MB 的哈希。
      return stage == null
          ? null
          : BackgroundImage(id: DisplayPrefs.kStageImageItemId, dataUrl: stage);
    }
    if (backgrounds.isEmpty) return null;
    return backgrounds[_clampIndex(0, backgrounds.length - 1)];
  }

  /// 壳当前会画的**下标**（来源是背景库时才有意义）。
  int get effectiveBackgroundIndex =>
      backgroundSource == DisplayPrefs.backgroundSourceLibrary
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

}
