/// 背景轮播控制器：**定时 + 索引**那一层，刻意不含任何 UI。
///
/// # 为什么要单独一个类（而不是在 `StatefulWidget` 里直接 `Timer.periodic`）
///
/// 1. **可测**：`flutter_test` 里推进假时钟要绕开整个 widget 树；
///    把定时与索引抽出来之后，「间隔 0 不进定时器」「只有一张不转」
///    「随机不连续」「stop/dispose 幂等」都能在纯 VM 里断言。
/// 2. **幂等**：`start` 连调两次、`stop` 后再 `dispose`、`dispose` 后再 `start`
///    都不许抛——这些都是真实会被触发的事件序列（用户狂点开关、页面重建）。
/// 3. **索引不落盘**：当前播到第几张**不是用户偏好**，刷新后从头开始符合直觉。
///
/// # 减少动画
///
/// `reducedMotion` 只关掉**过渡**（由渲染层读），**不关掉轮播本身**——
/// 「因为开了减少动画就悄悄不轮播」是静默失效，用户只会觉得功能坏了。
library;

import 'dart:async';
import 'dart:math' as math;

import 'background_logic.dart';

/// 轮播发生变化时回调（带新的索引）。
typedef BackgroundAdvance = void Function(int index);

class ShellSlideshow {
  ShellSlideshow({this.randomSource});

  /// 注入随机源（测试用固定种子，生产用默认 `Random()`）。
  ///
  /// 命名成 `randomSource` 而不是 `random`：方法里 `random:` 这个**名**要传给
  /// [nextBackgroundIndex] 表示「要不要随机顺序」，同名会让两件事糊在一起。
  final math.Random? randomSource;

  Timer? _timer;
  int _index = 0;
  int _length = 0;
  bool _random = false;
  bool _disposed = false;

  /// 当前索引（永远落在 `[0, length)`；`length == 0` 时恒为 0）。
  int get index => _length == 0 ? 0 : _index.clamp(0, _length - 1);

  /// 库里现在有几项。
  int get length => _length;

  /// 定时器是否在跑。
  bool get running => _timer != null;

  /// 库的**长度或顺序**变了（增删图片、改随机）时调用：
  /// 越界时把索引夹回范围，必要时重启定时器。
  void setLibrary({required int length, required bool randomOrder}) {
    _length = length;
    _random = randomOrder;
    if (_length <= 0) {
      _index = 0;
    } else if (_index >= _length) {
      _index = _length - 1;
    }
  }

  /// 直接跳到某一项（点缩略图时用）。
  void jumpTo(int next) {
    if (_length <= 0) return;
    _index = next.clamp(0, _length - 1);
  }

  /// 开始轮播。[intervalSeconds] 为 0 或库里不足 2 项 → **不进定时器**。
  void start(int intervalSeconds, {BackgroundAdvance? onAdvance}) {
    if (_disposed) return;
    stop();
    if (intervalSeconds <= 0 || _length < 2) return;
    _timer = Timer.periodic(Duration(seconds: intervalSeconds), (Timer _) {
      _index = nextBackgroundIndex(
        length: _length,
        current: _index,
        random: _random,
        randomSource: randomSource,
      );
      onAdvance?.call(_index);
    });
  }

  /// 停。**幂等**（没跑时再调一次什么也不做）。
  void stop() {
    _timer?.cancel();
    _timer = null;
  }

  /// 释放。**幂等**，且之后 [start] 也不再起定时器（对象已死）。
  void dispose() {
    stop();
    _disposed = true;
  }

  /// 重新启用一个已 [dispose] 的控制器（页面重建时复用同一个实例）。
  ///
  /// 为什么要留这个口子：`dispose` 之后 `start` 静默无效会很难查
  /// （表现是「轮播忽然不转了」），所以让调用方**显式**地复活它，
  /// 而不是偷偷复活。
  void revive() => _disposed = false;
}
