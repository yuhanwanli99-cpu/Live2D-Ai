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

// ─────────────────────────────────────────────────────────────────────────
// 心跳看门狗（2026-10-01，审计 F-0008-1）
//
// # 上一版判据为什么不够
//
// 上面那两个判据（`isLiveSocket` / `isZombieSocket`）都是**读 `readyState`**。
// 它们能挡住 2026-09-11 那个缺陷（close() 之后 onclose 尚未回调），但挡不住
// **半开连接**：笔记本休眠/唤醒、NAT 或代理静默丢包、服务端被 SIGKILL 而没有
// FIN/RST 送达。这时浏览器的 `readyState` **仍然是 OPEN**（规范只在「尝试收发
// 并失败」时才改状态，而本客户端**从不发送**任何帧——协议是单向事件流）。
// 于是 `ensureConnected()` 一直走「健康」分支不重连，`_status` 一直是
// `connected`，而 `turn_liveness.mustReleaseTurnOnWsLoss` 的前提
//（`status.isProblem`）永远不成立 ⇒ 「`POST /api/v1/chat` 成功但收不到回复」
// 的幽灵态可以永久挂着。
//
// 服务端每 10 s 主动发一条心跳（`web_api/ws/connection.rs`）——那是**已经送到
// 前端**的存活证据，前端解析了却从来没用过。下面把「静默多久算可疑 / 算死链」
// 抽成纯函数：`ws_client.dart` 的看门狗与 `ui_state_tracker.dart` 的相位判据
// 都读它，于是三个状态各有一个真实消费者（不是只算不用）。
// ─────────────────────────────────────────────────────────────────────────

/// 服务端主动心跳的间隔。**真源在 Rust**：
/// `crates/live2d-ai-desktop/src/web_api/ws/connection.rs`（10 s）。
/// 这里只作为「多久没动静算异常」的推导基准，前端**不**据此发 ping
/// （协议是单向的，客户端帧会被 tungstenite 静默丢弃）。
const Duration kWsHeartbeatInterval = Duration(seconds: 10);

/// 静默 ≥ 这个时长 ⇒ **可疑**（[HeartbeatLiveness.suspect]）。
///
/// 取值理由：≈3 个心跳周期。错过 1 次可能是调度抖动/GC 停顿（10 s 量级），
/// 连续 3 次收不到就不再是抖动。**必须**远小于「浏览器自己因 TCP 超时触发
/// close」——后者**没有上界**，正是幽灵窗口的来源。
const Duration kWsHeartbeatSuspectAfter = Duration(seconds: 30);

/// 静默 ≥ 这个时长 ⇒ **死链**（[HeartbeatLiveness.offline]）。
///
/// 取值理由：≈9 个心跳周期。到这一档时**连 `readyState == OPEN` 也不信**：
/// 这是「半开」最硬的判据——服务端就算还活着，它的话也已经 90 s 到不了我这里。
const Duration kWsHeartbeatOfflineAfter = Duration(seconds: 90);

/// 心跳看门狗的三态。
enum HeartbeatLiveness {
  /// 心跳按时到达：通道可信。
  online,

  /// 静默 ≥ [kWsHeartbeatSuspectAfter]（≈3 个周期）：**半开嫌疑**。
  ///
  /// 消费动作（`WsClient`）：主动摘掉这个「看着还 OPEN」的 socket 并立刻重开——
  /// 不等浏览器自己的 TCP 超时（那没有上界）。
  suspect,

  /// 静默 ≥ [kWsHeartbeatOfflineAfter]（≈9 个周期）：**已判定死链**。
  ///
  /// 消费动作（`UiStateTracker`）：相位不再认「已连接」，退回 `offline`，
  /// 免得界面继续显示「思考中」而实际一个字都收不到。
  offline,
}

/// 由「最后心跳时刻 + now」判三态。**纯函数、零依赖**（可 VM 单测）。
///
/// 边界（**含**，写进契约，测试逐点断言 ±1 ms）：
/// - `silent < suspectAfter` → [HeartbeatLiveness.online]
/// - `suspectAfter <= silent < offlineAfter` → [HeartbeatLiveness.suspect]
/// - `offlineAfter <= silent` → [HeartbeatLiveness.offline]
///
/// [lastHeartbeatAt] 为 null（还没收到过任何帧，例如刚建连、`subscribe_ack`
/// 尚未到达）按 **online** 处理：**「未知」不等于「已死」**——把未知判死会让
/// 每次刷新页面都先闪一下「后端未连接」。半开连接的前提是「曾经收到过」，
/// 所以这个取向不会削弱判据。
///
/// `now` 早于 [lastHeartbeatAt]（时钟回拨）按 online：`Duration` 为负，
/// 落进第一个分支。
HeartbeatLiveness heartbeatLiveness({
  required DateTime? lastHeartbeatAt,
  required DateTime now,
  Duration suspectAfter = kWsHeartbeatSuspectAfter,
  Duration offlineAfter = kWsHeartbeatOfflineAfter,
}) {
  if (lastHeartbeatAt == null) return HeartbeatLiveness.online;
  final Duration silent = now.difference(lastHeartbeatAt);
  if (silent < suspectAfter) return HeartbeatLiveness.online;
  if (silent < offlineAfter) return HeartbeatLiveness.suspect;
  return HeartbeatLiveness.offline;
}

/// 这个通道**还值不值得信**（`readyState` 之外的第二重判据）。
///
/// 与 [isLiveSocket] 的关系：那个判 socket 对象的状态机，这个判「话还到不到」。
/// 两者都成立才算可用——半开连接是前者成立、后者不成立的那种坏法。
///
/// # 「还到不到」只对已经连上的 socket 才问（复核 F-V1-1，2026-10-01）
///
/// `CONNECTING` 的静默**不是**「哑了」而是「还没开口」：它本来就在建连，
/// 而「最后听到动静」的时刻属于**上一条**连接。把时间判据扩到它身上，
/// `ensureConnected()`（用户每次发送前都会调）就会把一条**正在进行**的建连
/// 摘掉，并把指数退避 `_attempt` 重置回起点——后端持续不可达时表现为
/// 「每次发送都打断一次建连、退避永远涨不上去」。
/// 所以：
///
/// | readyState | 判据 |
/// | --- | --- |
/// | `CONNECTING` | **一律留着**（与 [isLiveSocket] 同取向；HEAD 旧判据也是这么早退的） |
/// | `OPEN` | 留着，除非静默到 [HeartbeatLiveness.offline]（≈90 s，半开正是这种坏法） |
/// | `CLOSING` / `CLOSED` | 僵尸，摘掉重开 |
///
/// 看门狗（`ws_client.dart`）读同一条边界：它同样**只对 `OPEN`** 判半开。
bool isChannelTrustworthy({
  required int readyState,
  required HeartbeatLiveness liveness,
}) {
  if (!isLiveSocket(readyState)) return false; // CLOSING/CLOSED：僵尸
  if (readyState != kSocketOpen) return true; // CONNECTING：留着，别打断建连
  return liveness != HeartbeatLiveness.offline;
}
