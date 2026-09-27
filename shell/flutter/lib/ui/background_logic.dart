/// 背景的**纯逻辑**：遮罩强度推导、轮播索引、图案配色。
///
/// # 为什么要单独成文件
///
/// 三件事都是「能用眼睛看、但只能用数字钉」的东西：
///
/// 1. **遮罩强度**要同时吃「图片不透明度」和「图片有多亮」——
///    两者的关系写错了就会出现「亮图压不住字 / 暗图被压太狠」。
/// 2. **轮播索引**（顺序回绕 / 随机不连续）是最容易写出 off-by-one 的地方。
/// 3. **图案配色**是「四套主题 × 四种图案」的一张小表。
///
/// 全部**不 import 渲染层**（`package:flutter/material.dart` 里的 `BoxFit` 等），
/// 因此可以在 VM 上直接单测；渲染层按这些值去画。
library;

import 'dart:math' as math;

import 'package:flutter/foundation.dart' show immutable;
import 'package:flutter/material.dart' show Color;

import '../design/background_item.dart';
import '../design/tokens.dart';

/// 遮罩档位（与 `DisplayPrefs.backgroundScrim` 同一套取值）。
abstract final class ScrimLevel {
  /// **自动**：按图片不透明度与亮度推导。默认。
  static const int auto = 0;

  /// 不画遮罩。
  static const int none = 1;

  /// 轻（固定一档）。
  static const int light = 2;

  /// 重（固定一档）。
  static const int heavy = 3;
}

/// 遮罩的**强度上界**。
///
/// 为什么要上界：遮罩是为了「让字可读」，不是为了「把背景盖住」。
/// 超过这个值用户会看到「我设的背景不见了」——那还不如让字不可读。
const double kMaxScrimAlpha = 0.45;

/// `auto` 遮罩的增益（与 [scrimAlphaFor] 的公式配套）。
///
/// 0.45 配 [kMaxScrimAlpha] 0.45：**满不透明度 + 界面最透时正好撞上界**，
/// 于是 `auto` 这一档把 `[0, 0.45]` 整段用满，而「重」档恰好等于 auto 的
/// 最坏情况——用户手动按「重」不会比自动更狠。
const double kAutoScrimGain = 0.45;

/// 由**固定档**直接给出的强度；`auto` 返回 `null`（走推导）。
///
/// 为什么「无」不是 0 而是 `null`：`auto` 在不透明度为 0 时也要返回 0
/// （不画图就没有可读性问题），所以两者不能共用一个 0。
double? fixedScrimAlpha(int level) => switch (level) {
  ScrimLevel.none => 0.0,
  ScrimLevel.light => 0.22,
  ScrimLevel.heavy => kMaxScrimAlpha,
  _ => null,
};

/// **`auto` 下的遮罩强度**：**纯函数，可 VM 单测**。
///
/// # 一条公式
///
/// `scrim = kAutoScrimGain * opacity * (0.5 + 0.5 * uiTransparency)`，
/// 上界 [kMaxScrimAlpha]。
///
/// 选它的理由只有一条：**遮罩与图片的「贡献量」成正比**。
/// 一张 0.15 的背景对画面的影响本来就很小，遮罩就不该按「有背景」的标准去压；
/// 反过来一张满不透明的图几乎把底色顶掉了，遮罩就该接近满格。
/// 写成 `opacity` 的正因子，「弱图弱压、强图强压」自动成立，
/// 不需要额外的分段逻辑。
///
/// 由此得到三条可测的设计线：
///
/// 1. **opacity = 0 → 0**：没画东西就没有可读性问题；
/// 2. **随 opacity 单调不减**；
/// 3. **随界面透明度单调不减**（面板越透，背景越要压）。
///
/// 默认档（opacity 1.0、界面透明 0.5）→ **0.34**；
/// 界面透明拉满 → 0.45，正好撞上界。
///
/// # 「按图片亮度自动加压」被删掉了（2026-09-27）
///
/// 原来公式里有一项 `0.65 * luma`，`luma` 来自 [ShellBackdrop.imageLuma]。
/// 但**没有任何调用方传过它** —— 采样一张图的主色需要异步解码，
/// 一直没有接上，于是那一项恒为中性 0.5：
///
/// - 公式写着「亮图压得更狠」，实际是**所有图一视同仁**；
/// 界面给不出任何提示，用户只能自己发现「白底那张图好像没被额外压暗」。
///
/// 与其留一个**名义上存在、实际上不生效**的自变量，不如删掉：
/// 想按亮度自适应，正确做法是走「重」档手动指定，或者将来真的做采样再把它
/// 作为**独立**的档位加回来。
///
/// 上界永远是 [kMaxScrimAlpha]。
double scrimAlphaFor({
  required double imageOpacity,
  required int level,
  double uiTransparency = 0.0,
}) {
  final double? fixed = fixedScrimAlpha(level);
  if (fixed != null) return fixed;
  final double opacity = imageOpacity.clamp(0.0, 1.0);
  if (opacity <= 0) return 0;
  // 界面越透，面板越挡不住字 → 背景要压得更多。
  //
  // 这一项让「界面透明程度」在**亮图**上仍然安全：
  // 没有它的话，面板 alpha 0.55 + 白墙 = 白底上的浅灰字。
  final double t = uiTransparency.clamp(0.0, 1.0);
  return (kAutoScrimGain * opacity * (0.5 + 0.5 * t)).clamp(
    0.0,
    kMaxScrimAlpha,
  );
}

/// 遮罩颜色：`palette.stage` 压上来（暗主题是黑幕、亮主题是白幕）。
///
/// 为什么用 `stage` 而不是写死黑/白：与 `AppColors.glassBarrier` 的
/// 「遮罩必须与背景反向」同一条纪律——亮色主题下黑幕会让画面发脏。
Color scrimColorFor(AppPalette palette) => palette.stage;

/// **轮播的下一个索引**：**纯函数**。
///
/// 三种情形都要对：
/// - `len <= 1` → 永远返回 [current]（**不轮播**，否则会在同一张上打转）；
/// - 顺序 → 回绕；
/// - 随机 → **绝不返回当前这张**（`len >= 2` 时才有「另一张」可选）。
///
/// 随机用 [random] 注入而不是自建 `Random()`：测试要能**确定**地断言。
int nextBackgroundIndex({
  required int length,
  required int current,
  required bool random,
  math.Random? randomSource,
}) {
  if (length <= 1) return 0;
  final int start = current.clamp(0, length - 1);
  if (!random) return (start + 1) % length;
  final math.Random rng = randomSource ?? math.Random();
  // 候选集是「除当前以外的全部」，所以在 [1, length) 上取模再错开。
  final int step = 1 + rng.nextInt(length - 1);
  return (start + step) % length;
}

// ──────────────────────────────────────────────────── 图案配色

/// 一个图案用到的两三个颜色（**由主题算出来**，不写死）。
@immutable
class PatternColors {
  const PatternColors({required this.a, required this.b, required this.ink});

  /// 主色（渐变起点 / 光晕 / 线条）。
  final Color a;

  /// 副色（渐变终点 / 第二团光晕）。
  final Color b;

  /// 线条/点阵用的墨色（**已经乘好透明度**）。
  final Color ink;
}

/// 某个内置图案在当前主题下的配色。
///
/// 取值原则：图案是**背景**，所以它必须比 UI 本身更安静——
/// `a`/`b` 都由 `accent` 与面色混合而来，混合比例 ≤ 0.35，
/// 墨色则压到 [kBaseScrimAlpha] 的一小部分。
PatternColors patternColorsFor(int id, AppPalette palette) {
  final Color accent = palette.accent;
  final Color stage = palette.stage;
  return switch (id) {
    BackgroundPatternId.gradient => PatternColors(
      a: Color.lerp(stage, accent, 0.30)!,
      b: Color.lerp(stage, palette.surfaceAlt, 0.55)!,
      ink: palette.ink.withValues(alpha: palette.dark ? 0.10 : 0.08),
    ),
    BackgroundPatternId.glow => PatternColors(
      a: Color.lerp(stage, accent, 0.26)!,
      b: Color.lerp(stage, palette.raised, 0.45)!,
      ink: palette.ink.withValues(alpha: 0.06),
    ),
    BackgroundPatternId.grid => PatternColors(
      a: Color.lerp(stage, accent, 0.12)!,
      b: stage,
      ink: palette.ink.withValues(alpha: palette.dark ? 0.08 : 0.10),
    ),
    BackgroundPatternId.stripes => PatternColors(
      a: Color.lerp(stage, accent, 0.16)!,
      b: stage,
      ink: palette.ink.withValues(alpha: palette.dark ? 0.07 : 0.09),
    ),
    _ => PatternColors(a: stage, b: stage, ink: palette.ink),
  };
}

/// 图案在设置里的小预览尺寸（px）——**渲染层与测试共用一个真源**。
const double kPatternPreviewSize = 96;

/// 网格点阵的间距与半径（渲染层画；放这里是为了能被测试读到）。
const double kGridSpacing = 7;
const double kGridDotMin = 0.45;
const double kGridDotMax = 0.70;

/// 网格的**固定**随机种子。
///
/// 为什么固定：不固定的话每帧重排 → 点阵会「沸腾」，是典型的装饰性抖动。
const int kGridSeed = 19;
