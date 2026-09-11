/// Live2D 渲染面传输抽象。
///
/// 职责边界：只负责「把 JSON 字符串送进 iframe / 把 iframe 的 JSON 字符串
/// 送出来」，不理解协议语义。协议解析与状态机在 `live2d_bridge.dart`。
///
/// 实现：
/// - `live2d_host_web.dart` → `WebIframeTransport`（真实 iframe + postMessage）；
/// - 测试用内存实现（见 `test/live2d_bridge_test.dart`）。
abstract interface class Live2DTransport {
  /// iframe → Flutter 的原始 JSON 字符串流（可能含历史遗留帧）。
  Stream<String> get events;

  /// 发送一条 JSON 字符串（调用方保证已序列化）。
  Future<void> send(String json);

  /// 释放消息监听与内部流。
  Future<void> dispose();
}
