/// 播放增益换算：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 单独成文件的理由与 `display_prefs.dart` 相同：这是一段会被测试反复钉住的
/// 算术，不该埋在 `package:web` 的导入后面（那样只能在浏览器里跑）。
library;

/// 实际输出增益：静音时恒为 0，否则取音量曲线。
///
/// 静音走「增益置 0」而不是「断开 destination」：音频图必须保持活着，
/// `AudioContext.currentTime` 与排片时刻才与真实播放一致，口型电平的到期释放
/// （`LevelTimeline`）才不会漂移。见 `audio_player.dart` 的不变量 3。
double playbackGain({required double volume, required bool muted}) =>
    muted ? 0.0 : gainForVolume(volume);

/// 音量滑杆位置（0..1）→ WebAudio 线性增益（0..1）。
///
/// **为什么不是恒等映射**：WebAudio 的 `gain` 是**线性振幅**倍数
/// （[MDN GainNode](https://developer.mozilla.org/en-US/docs/Web/API/GainNode)），
/// 而人耳对响度的感知接近对数。滑杆若线性映射到振幅，手感是「前半段几乎没变化、
/// 后半段突然变响」。
///
/// 这里取 `v²` 作为**温和的感知压缩**：半个滑杆 ≈ -12 dB，四分之一 ≈ -24 dB。
/// 更严格的做法是把滑杆按 dB 线性映射（即业界所谓 perceptual volume slider，
/// 见 <https://github.com/CyberFlameGO/perceptual>），但那条曲线会把**大部分
/// 行程压到极小音量**——对「把桌宠调小声、别盖过我干别的事」这个主用途过于陡峭。
///
/// **这是手感取舍，不是规范要求**：要换曲线只改这一个函数，回归在
/// `test/audio_gain_test.dart`。
double gainForVolume(double volume) {
  // 非有限值回落默认（满音量）——约定与 `DisplayPrefs.clampVolume` 一致。
  // 这是兜底：JSON 无法表示 NaN/Infinity，localStorage 里读不出来。
  final v = volume.isFinite ? volume.clamp(0.0, 1.0) : 1.0;
  if (v <= 0) return 0.0;
  if (v >= 1) return 1.0;
  return v * v;
}
