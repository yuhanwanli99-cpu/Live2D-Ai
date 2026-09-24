/// 动作幅度即时预览的**唯一防抖下发器**（纯逻辑，无 Flutter widget 依赖）。
///
/// # 它解决什么
///
/// 外观三滑条改的是**设置草稿**，而渲染面只在「设置保存后」才收到值——
/// 用户的原话是「滑条要先保存才生效」。这里把草稿值防抖后直接下发给渲染面。
///
/// # 三层优先级（2026-09-23 W7 收口；这份规则的真源就在本类）
///
/// **临时覆盖（调试面板，显式、可一键取消） > 草稿（谁在拖谁优先） > 磁盘值**
///
/// - 草稿 / 磁盘值由宿主经 [schedule] / [syncNow] 送进来（`app/shell_prefs.dart`
///   的 `_effectiveActionScales`：草稿字段压过远端，都没加载就是 `null`）；
/// - 调试面板的「临时幅度覆盖」经 [pin] 成为本类的**显式状态**；
/// - [active] 是**给舞台的唯一取值口**：有临时覆盖给临时值，否则给产品/草稿值。
///   `main.dart` 把它交给 `Live2DStage.actionScales`，iframe 重建 / 重挂后
///   舞台自己就能补发（RESEARCH §2.3 的死参数由此接上）；
/// - 所有下发都只经本类：`main.dart` 里那条
///   `_stageKey.currentState?.sync(actionScales: …)` 直发路径**已删除**——
///   过去两个写者各自去重（产品值走本类、临时值直发且不更新 `_sent`），
///   同一操作序列的结果依赖历史（RESEARCH §2.4 的确定性缺陷 2）。
///
/// **关键不变式**：临时覆盖生效期间，任何产品/草稿值都不会被写下去——
/// [syncNow] 发的是 [active] 而不是入参，所以 `_applyPrefs` 的
/// `force: true` 补发（改主题 / 调音量等任意 `DisplayPrefs` 变更都会走）
/// 也冲不掉临时值（RESEARCH §2.4 的确定性缺陷 1）。
///
/// # 为什么拆成一个可实例化的类（而不是散在 State 里）
///
/// `part` 文件里的 `_ShellRootState` 私有字段在 VM 测试里拿不到实例；
/// 而「拖动 → 一帧下发」「值没变不重发」「重建后强发」「临时值不被冲掉」这几条
/// 恰恰是最容易悄悄坏掉、又最该被回归盯住的行为。抽成 `ActionScalesSyncer` 后，
/// 这些在 `flutter test` 里用假时钟 + 计数回调就能钉死。
library;

import 'dart:async';

/// 拖动三滑条时的**防抖**窗口。
///
/// 滑条一次拖动会连发几十次 `onChanged`（每次都是一帧 `sync`），而渲染面
/// 是为 60fps 渲染设计的、不是为「每秒几十条配置帧」。
/// 150ms：短到用户松手前就能看到反应（拖动中每 150ms 至少一帧），
/// 长到足以把一整套手势收成一两帧。
const Duration kActionScalesDebounce = Duration(milliseconds: 150);

class ActionScalesSyncer {
  /// 构造：`send` 是真正的下发回调（宿主传 `stage.applyActionScales`）。
  ActionScalesSyncer(this.send);

  /// 下发回调；`null` 值**不会**被调用。
  final void Function(Map<String, double> scales) send;

  /// 调试面板钉住的**临时覆盖**（`null` = 没有）。
  ///
  /// 按定义只对本会话有效、**不落盘**；唯一的清除口是 [clearPin]
  /// （面板上就是「恢复产品设置」）。草稿 / 磁盘值变化一律不清它。
  Map<String, double>? _pinned;

  String? _sent;
  Timer? _timer;
  bool _disposed = false;

  /// 上次已下发的快照（测试与排障用；`null` = 还没发过）。
  String? get sentSnapshot => _sent;

  /// 当前是否有临时覆盖。
  bool get hasPin => _pinned != null;

  /// 临时覆盖的三项倍率（没有时 `null`）。
  Map<String, double>? get pinned => _pinned;

  /// **给舞台的唯一取值口**：临时覆盖 > [product]（草稿优先的有效产品值）。
  Map<String, double>? active(Map<String, double>? product) => _pinned ?? product;

  /// 排一次下发（防抖）。草稿一变就调它——**不必先保存**。
  void schedule(Map<String, double>? payload) {
    if (_disposed) return;
    _timer?.cancel();
    _timer = Timer(kActionScalesDebounce, () => syncNow(payload));
  }

  /// 立刻下发**当前有效值**（`onReady` / iframe 重建后补发）。
  ///
  /// 真正发出去的是 [active]（临时覆盖优先），**不是**入参本身：
  /// 临时覆盖生效时，产品/草稿值经这里也写不下去（见类头注的关键不变式）。
  ///
  /// `payload == null` 且**没有临时覆盖** = 「舞台该用渲染面的出厂默认」：
  /// **不发**（发了会把渲染面自己的默认覆盖成前端猜的值），并把去重标记清空，
  /// 让下一次真值一定发得出去。
  ///
  /// `force` = 绕过「值没变就不发」——iframe 重建后旧帧随旧桥销毁，**必须重发**
  /// （与 `stage-bg` / `stageColor` 同一条纪律）。
  void syncNow(Map<String, double>? payload, {bool force = false}) {
    _timer?.cancel();
    _timer = null;
    if (_disposed) return;
    final Map<String, double>? effective = _pinned ?? payload;
    if (effective == null) {
      _sent = null;
      return;
    }
    final String key = _key(effective);
    if (!force && key == _sent) return;
    _sent = key;
    send(effective);
  }

  /// 调试面板「应用到渲染面（临时）」：钉住一组值并立刻下发。
  ///
  /// 钉住之后，草稿 / 磁盘值只会在 [_pinned] 被清掉之后才可能到达渲染面。
  void pin(Map<String, double> scales) {
    _pinned = Map<String, double>.unmodifiable(scales);
    syncNow(scales, force: true);
  }

  /// 调试面板「恢复产品设置」：清掉临时覆盖，并把产品值**强制**写下去。
  ///
  /// `force: true` 是必需的：临时值可能恰好等于产品值（用户把三个滑条拖回
  /// 同一组数），此时 `_sent` 去重会把这次恢复挡掉，舞台就停在「看着像恢复了、
  /// 其实没重发」上。
  void clearPin(Map<String, double>? product) {
    _pinned = null;
    syncNow(product, force: true);
  }

  /// 丢弃未到点的防抖窗口（宿主 `dispose`）。
  void dispose() {
    _disposed = true;
    _timer?.cancel();
    _timer = null;
  }

  static String _key(Map<String, double> payload) =>
      'head=${payload['head']},body=${payload['body']},'
      'expression=${payload['expression']}';
}
