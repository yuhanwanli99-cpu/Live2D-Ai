/// DisplayPrefs 的**区间夹持与字节预算**（display_prefs.dart 的 part）。

///

/// 2026-10-06 按 B2 从 display_prefs.dart 搬出：方法体**逐字搬运**，只补了

/// extension 作用域必需的 DisplayPrefs. 类静态（默认值 / 区间端点）限定符。

/// _clampInt 与 _clampIntToRange 语义**刻意不同**（前者越界回落默认、后者夹到端点），别顺手合并。

part of 'display_prefs.dart';

extension DisplayPrefsLimits on DisplayPrefs {

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
    if (!value.isFinite) return DisplayPrefs.defaultBackgroundOpacity;
    return value.clamp(DisplayPrefs.minBackgroundOpacity, DisplayPrefs.maxBackgroundOpacity);
  }

  /// 界面透明度夹到 `[0, 1]`；非有限数回落默认。
  static double clampUiTransparency(double value) {
    if (!value.isFinite) return DisplayPrefs.defaultUiTransparency;
    return value.clamp(0.0, DisplayPrefs.maxUiTransparency);
  }

  /// 整数端点夹持（DEC-1）。`slideInterval` 用它：越界回落 0 会把轮播关掉。
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
      DisplayPrefs.minSlideIntervalSeconds,
      DisplayPrefs.maxSlideIntervalSeconds,
    );
  }

  /// 缩放夹到 `[DisplayPrefs.minScale, DisplayPrefs.maxScale]`；非有限值回落默认。
  static double clampScale(double value) {
    if (!value.isFinite) return DisplayPrefs.defaultScale;
    return value.clamp(DisplayPrefs.minScale, DisplayPrefs.maxScale);
  }

  /// 口型灵敏度夹到 `[DisplayPrefs.minMouthSensitivity, DisplayPrefs.maxMouthSensitivity]`；非有限值回落默认。
  static double clampMouthSensitivity(double value) {
    if (!value.isFinite) return DisplayPrefs.defaultMouthSensitivity;
    return value.clamp(DisplayPrefs.minMouthSensitivity, DisplayPrefs.maxMouthSensitivity);
  }

  /// 主音量夹到 `[DisplayPrefs.minVolume, DisplayPrefs.maxVolume]`；非有限值回落默认（满音量）。
  static double clampVolume(double value) {
    if (!value.isFinite) return DisplayPrefs.defaultVolume;
    return value.clamp(DisplayPrefs.minVolume, DisplayPrefs.maxVolume);
  }

}
