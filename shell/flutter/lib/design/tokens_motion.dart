part of 'tokens.dart';

/// **语义节拍**：不属于「UI 过渡时长」那 4 档的单点节奏。
///
/// 为什么要单独一个家族，而不是塞进 [AppDurations]：
/// [AppDurations] 的语义是「一次过渡有多快」（hover/pressed/内容切换），
/// 而这里是「一个持续状态按什么节拍呼吸」。两者混在一起会让
/// 「4 档」这个约束失去意义（规格 §2.7 的时长阶梯就是 4 档）。
///
/// 三者都是**跨组件共享**的：思考呼吸同时被 `StatePill` 与
/// `StreamingIndicator` 用，如果各自写一个 `Duration(milliseconds: 1400)`，
/// 改一处忘一处就会让两个指示器不同步（同类项目的经典病）。
abstract final class AppRhythms {
  /// 思考态的呼吸周期（规格 §6.3：1.4 s）。
  ///
  /// 取值理由：慢到不像「加载转圈」（那会制造焦虑），快到一眼看出是活的。
  static const Duration thinkingBreath = Duration(milliseconds: 1400);

  /// 「瞬时状态」的保持时长：`已打断`、`已复制` 这类**就地确认**共用一档。
  ///
  /// 长到人眼能读完那几个字，短到不会和下一轮重叠。
  ///
  /// **刻意只有一档**（2026-09-11，P2-3）：下面那条关于「流式三点周期」的
  /// 说明同样适用于这里——两个**同值**令牌只会制造「改一处忘一处」的机会。
  /// 「已复制」与「已打断」确实不是同一件事，但它们要的是**同一个时长**，
  /// 那就该是同一个令牌：令牌的粒度是**数值语义**，不是文案语义。
  static const Duration interruptedHold = Duration(milliseconds: 1200);

  /// 悬停提示出现前的等待（`Tooltip.waitDuration`）。
  ///
  /// 为什么是「节奏」而不是 `AppDurations` 里的一档：那 4 档的语义是
  /// **一次过渡有多快**，而这是**等多久才开始**。两者混在一起会让
  /// 「4 档」这个约束失去意义。
  ///
  /// 取值理由：400 ms 是桌面端悬停提示的常见量级——短到不觉得迟钝，
  /// 长到鼠标划过时不至于一路弹提示。
  static const Duration hintDelay = Duration(milliseconds: 400);

  // 刻意**没有**单独的「流式三点周期」：三点跑动与呼吸光表达的是同一件事
  // （思考中），共用 [thinkingBreath] 让两个指示器**同频**——看起来是有意为之，
  // 而不是各跑各的。两个同值令牌只会制造「改一处忘一处」的机会。

  /// 登记表（只服务测试）。
  static const Map<String, Duration> registry = <String, Duration>{
    'thinkingBreath': thinkingBreath,
    'interruptedHold': interruptedHold,
    'hintDelay': hintDelay,
  };
}

/// 动效时长：4 档，全部具名。
abstract final class AppDurations {
  /// hover / pressed / 开关 / 徽标切换。
  static const Duration fast = Duration(milliseconds: 120);

  /// 内容切换、消息渐入、横幅滑入。
  static const Duration base = Duration(milliseconds: 200);

  /// 模态 / sheet 入场。
  static const Duration slow = Duration(milliseconds: 320);

  /// **仅此一处**长动画：启动揭示。
  static const Duration reveal = Duration(milliseconds: 600);

  /// 登记表（只服务测试）。
  static const Map<String, Duration> registry = <String, Duration>{
    'fast': fast,
    'base': base,
    'slow': slow,
    'reveal': reveal,
  };
}

/// 动效曲线：**只允许两条**。
abstract final class Motion {
  /// 入场 / 出场（快出慢收）。AIRI 与 Nexus 独立收敛到同一条，属跨项目验证值。
  static const Cubic enter = Cubic(0.16, 1.0, 0.3, 1.0);

  /// 状态过渡、颜色/尺寸变化、开关。
  static const Curve state = Curves.easeInOutCubic;

  /// 登记表：曲线没有可枚举数值，登记名字。
  static const List<String> names = <String>['enter', 'state'];
}

/// 阈值令牌（转发 [Breakpoints]，让「引用侧枚举」能统一按 `Breakpoints.` 统计）。
abstract final class BreakpointTokens {
  static const double mediumMin = Breakpoints.mediumMin;
  static const double expandedMin = Breakpoints.expandedMin;
}

/// 任意颜色 → 舞台底要的 CSS 十六进制串（`#rrggbb`）。
///
/// # 为什么提到顶层（2026-09-11，P1-3）
///
/// 舞台底在**主题切换**时要跟着 UI 一起**插值**（否则界面在 200 ms 里渐变、
/// iframe 已经跳到终色，看起来像「舞台先闪了一下」）。插值意味着中途会产生
/// 一堆**中间色**，它们也要拼成同样的串格式——拼法一旦有第二个实现就会漂移，
/// 而渲染面是**严格校验**这个串的（格式不对就静默回落到默认色，
/// 表现为「主题切了但舞台没变」）。
///
/// 所以拼法只有这一处，[AppPalette.stageCss] 也走它。
///
/// 只取 RGB：舞台底是不透明纯色（有测试守着 `alpha == 1`）。
String stageColorCss(Color color) =>
    '#${(color.toARGB32() & 0xFFFFFF).toRadixString(16).padLeft(6, '0')}';
