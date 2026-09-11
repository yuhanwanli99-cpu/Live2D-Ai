/// 播放增益换算：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// 单独成文件的理由与 `display_prefs.dart` 相同：这是一段会被测试反复钉住的
/// 算术，不该埋在 `package:web` 的导入后面（那样只能在浏览器里跑）。
///
/// 2026-09-11：这里原先还有一个 `playbackGain({volume, muted})`（把「静音」也
/// 折算成增益 0，服务于 Web Audio 的 `GainNode`）。播放路径改成 `<audio>` 媒体
/// 元素后，静音由 `element.muted` 表达、音量由 `element.volume` 表达——两者
/// 直接对应浏览器自己的两个开关，**再折叠成增益反而是多余的**（而且会掩盖
/// 「站点级静音」这条我们特意换过来的权限路径），所以那个函数连同它的 4 条
/// 测试一起删了。音量曲线本身（[gainForVolume]）不变，手感不变。
library;

/// 音量滑杆位置（0..1）→ 线性振幅（0..1），写到 `HTMLMediaElement.volume` 上。
///
/// **为什么不是恒等映射**：`HTMLMediaElement.volume` 与 Web Audio 的
/// `gain` 一样是**线性振幅**倍数（[MDN volume]），而人耳对响度的感知接近对数。
/// 滑杆若线性映射到振幅，手感是「前半段几乎没变化、后半段突然变响」。
///
/// 这里取 `v²` 作为**温和的感知压缩**：半个滑杆 ≈ -12 dB，四分之一 ≈ -24 dB。
/// 更严格的做法是把滑杆按 dB 线性映射（即业界所谓 perceptual volume slider，
/// 见 <https://github.com/CyberFlameGO/perceptual>），但那条曲线会把**大部分
/// 行程压到极小音量**——对「把桌宠调小声、别盖过我干别的事」这个主用途过于陡峭。
///
/// **这是手感取舍，不是规范要求**：要换曲线只改这一个函数，回归在
/// `test/audio_gain_test.dart`。
///
/// [MDN volume]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement/volume
double gainForVolume(double volume) {
  // 非有限值回落默认（满音量）——约定与 `DisplayPrefs.clampVolume` 一致。
  // 这是兜底：JSON 无法表示 NaN/Infinity，localStorage 里读不出来。
  final v = volume.isFinite ? volume.clamp(0.0, 1.0) : 1.0;
  if (v <= 0) return 0.0;
  if (v >= 1) return 1.0;
  return v * v;
}
