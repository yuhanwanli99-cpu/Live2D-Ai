/// WebSocket 存活判据：**纯逻辑，零 `package:web` 依赖**。
///
/// `ws_client.dart` 必须 `import 'package:web'`，于是它的任何分支在
/// `flutter test` 里都跑不到。而下面这个判据正是 2026-09-11 查出来的缺陷所在，
/// 必须有回归——所以把「readyState → 还值不值得留着」这条表抽成纯函数。
///
/// # 缺陷是什么（2026-09-11）
///
/// `ensureConnected()` 曾写成 `if (_disposed || _socket != null) return;`——
/// 用「有没有 socket 对象」当存活判据。但 `close()` 之后 `onclose` 是
/// **异步**回调，这中间 `_socket` 仍然非 null 而连接已经废了；
/// `_handleText` 里 `shutdown_ready` 走的正是「先 close 再退避重连」这条路。
/// 于是这份**专门为了「POST 成功但收不到回复」而写的保险**，
/// 恰好在最需要它的那一刻变成空操作。
library;

/// DOM `WebSocket.readyState` 的四个取值。
///
/// 与 `package:web` 的 `WebSocket.CONNECTING/OPEN/CLOSING/CLOSED` 一一对应；
/// 这里重新声明是为了让本文件不依赖 `package:web`（值本身是 W3C 标准的一部分，
/// 不会变）。
const int kSocketConnecting = 0;
const int kSocketOpen = 1;
const int kSocketClosing = 2;
const int kSocketClosed = 3;

/// 这个 socket 对象**还值得留着**吗（= 不需要立刻重开）。
///
/// - `CONNECTING`：**留着**。它正在建连，此刻重开只会多留一条废连接，
///   而且会让 `send()` 之后本应到达的回复彻底失去落点。
/// - `OPEN`：留着，可用。
/// - `CLOSING` / `CLOSED`：**僵尸**——必须摘掉重开。这正是上面那个缺陷。
bool isLiveSocket(int readyState) =>
    readyState == kSocketConnecting || readyState == kSocketOpen;

/// 僵尸 socket：对象还在，但连接已经废了，`onclose` 尚未（或不会）回调。
bool isZombieSocket(int readyState) =>
    readyState == kSocketClosing || readyState == kSocketClosed;
