# GROUNDING · W-VERIFY-1（独立复核 W1-a `93416f0c`）

> 复核者：`verifier`（非实施者，**只读仓库**；唯一写入处 = `docs/audit/2026-10-01-debloat/**`）
> 复核对象：`93416f0c fix(frontend): W1-a/R8a-1 幽灵态两半 —— 心跳看门狗 + 本地失败回落相位`
> **工作树纪律**：本波复核**不在仓库工作树跑任何 flutter/cargo**（当时有 3 个 Dart worker 在飞）。
> 全部命令在 **`git archive 93416f0c` 解出的副本**里跑：`/tmp/w1a_verify`（门禁）、
> `/tmp/w1a_probe`（对抗性探针）、`/tmp/w1a_t1..t3`（判别力篡改）。
> 副本完整性已核（8 个文件 md5 与 `git show 93416f0c:<path>` **逐个 SAME**）：`raw/w1a_copy_integrity.txt`。
> 本轮**未改任何源码/已有文档、未 commit、未碰 `/home/skystar/Live2D-Ai`**。

---

## 0. 起始态与范围

```console
$ git log --oneline -1
93416f0c fix(frontend): W1-a/R8a-1 幽灵态两半 —— 心跳看门狗 + 本地失败回落相位
$ git show --name-status --format="" 93416f0c
M  shell/flutter/lib/api/ws_client.dart
M  shell/flutter/lib/api/ws_liveness.dart
M  shell/flutter/lib/app/shell_chat.dart
M  shell/flutter/lib/chat/chat_controller.dart
M  shell/flutter/lib/state/ui_state_tracker.dart
A  shell/flutter/test/ui_state_tracker_half_open_test.dart
A  shell/flutter/test/ui_state_tracker_local_failure_test.dart
A  shell/flutter/test/ws_heartbeat_liveness_test.dart
$ git diff --numstat 93416f0c^ 93416f0c
shell/flutter/lib/api/ws_client.dart               |  99 +++-
shell/flutter/lib/api/ws_liveness.dart             |  92 +++
shell/flutter/lib/app/shell_chat.dart              |   9 ++     ← 8 行注释 + 1 行代码（`:53`）
shell/flutter/lib/chat/chat_controller.dart        |  22 +++
shell/flutter/lib/state/ui_state_tracker.dart      |  73 +++-
shell/flutter/test/ui_state_tracker_half_open_test.dart     | 133 ++  (0 删)
shell/flutter/test/ui_state_tracker_local_failure_test.dart | 169 ++  (0 删)
shell/flutter/test/ws_heartbeat_liveness_test.dart          | 174 ++  (0 删)
8 files changed, 767 insertions(+), 4 deletions(-)
```

**范围判定 ✅**：改动 = 实施者自报的 5 个 `lib` 文件 + 3 个新测试文件，**无第 9 个文件**。
`shell_chat.dart` 的 9 行 = 8 行注释 + **1 行代码**（`:53 if (_chat.sendFailedLocally) _ui.markTurnFailed();`），
与 leader「明确授权的孤儿文件 1 行接线」一致 —— **批准成立，不是偷偷扩范围**。

## 1. 归档副本上的门禁（`/tmp/w1a_verify`，工作树零触碰）

```console
$ git archive 93416f0c | tar -x -C /tmp/w1a_verify
$ cd /tmp/w1a_verify/shell/flutter && flutter pub get && flutter analyze && flutter test
PUBGET_EXIT=0
No issues found! (ran in 3.1s)
ANALYZE_EXIT=0
00:26 +1407: All tests passed!
TEST_EXIT=0
```
（全文 250,363 B：`raw/w1a_flutter.txt`）

| 项 | 声称 | 实测 | 判定 |
|---|---|---|---|
| `flutter pub get`（离线/缓存） | — | exit 0 | ✅ 前提成立（无需网络） |
| `flutter analyze` | 0 issue | `No issues found!` | ✅ |
| `flutter test` | **1407 passed / 0 failed** | **`+1407 All tests passed!`** / exit 0 | ✅ |
| 测试条数不降 | 1378 → 1407（+29） | 新测试文件新增 **29** 条 `test(`/`testWidgets(`（133/169/174 行，**0 行删除**） | ✅ 只增不减 |

## 2. W1-a 完整 diff 逐文件复核（读码，对照 `raw/w1a_full_diff.txt`）

### 2.1 F-0008-1 心跳看门狗 —— 逐跳读链

**判据（纯函数，`lib/api/ws_liveness.dart:108-121`）**

```dart
HeartbeatLiveness heartbeatLiveness({required DateTime? lastHeartbeatAt, required DateTime now,
    Duration suspectAfter = kWsHeartbeatSuspectAfter,   // 30s
    Duration offlineAfter = kWsHeartbeatOfflineAfter})  // 90s
{
  if (lastHeartbeatAt == null) return HeartbeatLiveness.online;   // 「未知」≠「已死」
  final Duration silent = now.difference(lastHeartbeatAt);
  if (silent < suspectAfter) return HeartbeatLiveness.online;     // 边界含在 suspect 侧
  if (silent < offlineAfter) return HeartbeatLiveness.suspect;
  return HeartbeatLiveness.offline;
}
```
`isChannelTrustworthy(readyState, liveness) = isLiveSocket(readyState) && liveness != offline`（`:125`）。
常量与真源一致：`kWsHeartbeatInterval = 10s`（Rust `web_api/ws/connection.rs`）⇒ 3×/9×。

**消费者（不是「只算不用」——`grep` 实证）**

```console
$ grep -rn "heartbeatLiveness(" lib/ | grep -v "^lib/api/ws_liveness.dart"
lib/api/ws_client.dart:143          # ensureConnected 的第二重判据
lib/api/ws_client.dart:202          # 看门狗巡检 _checkHeartbeatLiveness
lib/state/ui_state_tracker.dart:105 # channelLiveness getter
$ grep -rn "isChannelTrustworthy" lib/ | grep -v ws_liveness.dart
lib/api/ws_client.dart:141
```
⇒ 纯函数 **3 个调用点 / 2 个文件**；`signals.wsConnected = _wsStatus.isUsable && channelLiveness != offline`（`ui_state_tracker.dart:94`）。

**「Timer → `_setStatus(disconnected)` → `mustReleaseTurnOnWsLoss`」逐跳核（leader 点名项）**

| 跳 | 位置 | 行为 | 核验 |
|---|---|---|---|
| 1 | `ws_client.dart:188-190` | `onopen` 里 `_startWatchdog()`：`Timer.periodic(5s, _checkHeartbeatLiveness)` | ✅ 只在 `onopen` 启动（`onopen` 同时把 `_lastHeartbeatAt` 置 now，`:100`） |
| 2 | `:194-207` | 巡检：`_disposed`/`socket==null`/`!isLiveSocket(readyState)` 三个早退；`online` 早退；否则 `_recoverFromHalfOpen(socket)` | ✅ **suspect（≥30s）就处置**，早于 offline（90s）⇒ 看门狗是主路径，liveness→`wsConnected` 只是显示层兜底 |
| 3 | `:219-227` | `_setStatus(WsStatus.disconnected)`（**:220**）→ 取消重连计时器 → 取消看门狗 → `identical` 判定后 `_socket=null` → `_detach(socket)`（**:224**，定义在 `:161`；清空 4 个回调并 `close()`）→ `_attempt=0` → `_open()`（**不走退避**） | ✅ 顺序正确：**先宣布通道有问题**（`disconnected` 是 `isProblem`）再摘；`_detach` 清 `onclose` ⇒ **不会与旧 socket 的 `onclose` 抢着重连**（无重复连接竞态） |
| 4 | `:238-241` | `_setStatus` **不去重**：`_status = status; _statuses.add(status);` | ✅ 关键：即使状态已是 `disconnected` 也会再发一次事件，消费者不会漏 |
| 5 | `chat_controller.dart:28-40` | `ws.statuses.listen(...)` → `mustReleaseTurnOnWsLoss(turnInFlight: _streaming, status: status)` → `_releaseTurn(...)` | ✅ `turn_liveness.dart:100-103`：`turnInFlight && status.isProblem`；`ws_status.dart`：`isProblem = disconnected \|\| closed` ⇒ **链成立** |

**结论：这条链完整、无断点、无重复处置。** 并且 `_lastHeartbeatAt` 的刷新面是**每一帧**
（`ws_client.dart:250` 在解析之前；`ui_state_tracker.dart:191` 同样在 `switch` 之前），
这正是「流式聊天中 `text_delta` 也是存活证据」的正确取向。

### 2.2 F-0007-1 本地失败回落相位

- `ui_state_tracker.dart:178-183 markTurnFailed()`：清 `_turnActive`/`_voiceActive`，**不置 `_interrupted`、不置 `_errorActive`**；
  `deriveUiPhase`（`ui_phase.dart`）里 `!wsConnected → offline` 优先于 `turnActive`、`voiceActive` 优先于 `turnActive` —— 语义自洽。
- `chat_controller.dart:80/83/119/168`：`_sendFailedLocally` **只由 `_failLocalTurn`（本地失败三条路径）置位**，每轮 `send()` 开头清零。
- `shell_chat.dart:43-53`：`markTurnAccepted()` → `await _chat.send(text)` → `if (_chat.sendFailedLocally) _ui.markTurnFailed()`。

**「为什么不用 `_chat.error != null`」——实施者的理由我独立证实（不是凭空）**：
`chat_controller.dart:331-350`：收到**非致命** `WsErrorEvent` 时 `_error = formatWsError(...)`（**:341**）后，
只有 `mustSettleTurnOnError(fatal: true)` 才收口；非致命**不收口**（注释写明「已生成的语音还要播完，交给紧随的 `turn_state`」）。
⇒ 存在「`error != null` 且本轮仍在飞」的窗口（且 `send()` 是「先建气泡再发请求」，服务端帧**可能早于 HTTP 应答到达**，这一点由 `:120-130` 的注释自证）。
**因此用 `_chat.error != null` 会误判——实施者的取舍正确，我找不到反例。**

## 3. 反例尝试（**我自己构造并执行**，全在 /tmp 副本）

探针源码：`raw/zz_verifier_probe_test.dart`　原始输出：`raw/w1a_probe.txt`

```console
$ cd /tmp/w1a_probe/shell/flutter && flutter test test/zz_verifier_probe_test.dart
00:00 +5: All tests passed!
PROBE_EXIT=0
```

| 探针 | 断言 | 结果 | 意义 |
|---|---|---|---|
| **A** | `isChannelTrustworthy(CONNECTING, offline) == false`；对照 `(OPEN, suspect) == true` | 通过 | **发现空白格**（见 F-V1-1） |
| **B** | `onWsStatus(connected)` + `markTurnAccepted()` + 静默 90s ⇒ `phase == offline`、`signals.wsConnected == false`，但 **`signals.turnActive` 仍为 true**，且 `mustReleaseTurnOnWsLoss(turnInFlight:true, status: connected) == false` | 通过 | **证明了链的主从关系**：解「在飞轮次/`_streaming`」的只有看门狗置 `disconnected`；`liveness→wsConnected` 只是显示层兜底（这也解释了为什么断言必须同时钉住两条） |
| **C** | `voice_started`（外部轮次）+ `markTurnAccepted()` 后调一次 `markTurnFailed()` ⇒ **`voiceActive` 也被清掉**，相位回 `idle` | 通过 | 实施者自报的「残余窄竞态」**实测成立**（共享同一对标志，无轮次令牌） |
| **D** | `markTurnAccepted()` → `markTurnFailed()` 的净效果 = `idle` | 通过 | 与 F-V1-2 的「早退 + 陈旧标志」叠加时就是**误清** |

### 3.1 判别力自证（**独立篡改**，不改仓库；`raw/w1a_tamper.txt`）

| 篡改 | 文件/行 | 结果 |
|---|---|---|
| **T1** | `ws_liveness.dart` 把 `silent < suspectAfter` 写成 `<=` | `+16 -2`，`T1_EXIT=1` ✅ 边界测试真的会红 |
| **T2** | `ui_state_tracker.dart` 删掉 `consume()` 里 `_lastHeartbeatAt = _now();` | `+3 -1`，`T2_EXIT=1` ✅ 状态层消费者测试会红（`Expected online / Actual suspect`） |
| **T3** | `markTurnFailed()` 里加 `_interrupted = true`（冒充 `markStopped`） | `+1 -4`，`T3_EXIT=1` ✅ F-0007-1 的「不许伪造成已打断」测试会红 |

⇒ 三份新测试**有判别力**（把它们保护的行为改错就红），不是「恒真断言」。

## 4. 发现

### F-V1-1（P3）`ensureConnected()` 会摘掉**还在建连**的 socket（`CONNECTING + offline` 空白格）

```dart
// ws_client.dart:139-152（提交版）
final bool trustworthy = socket != null &&
    isChannelTrustworthy(readyState: socket.readyState,
        liveness: heartbeatLiveness(lastHeartbeatAt: _lastHeartbeatAt, now: DateTime.now()));
if (trustworthy) return;
if (socket != null) { _detach(socket); _socket = null; }   // ← 对 CONNECTING 也会走到
_reconnectTimer?.cancel(); _attempt = 0; _open();
```
- `isLiveSocket` 把 `CONNECTING` 也算「留着」（`:34-36`），所以 `CONNECTING + online/suspect` 都可信；
  但 `+ offline` 时 **`ensureConnected()` 会把一条正在建连的 socket 也摘掉**，并 `_attempt = 0`（**重置退避**）后立刻重开。
- **与 HEAD 的差异（我核过旧实现）**：HEAD 是 `if (socket != null && isLiveSocket(socket.readyState)) return;`
  ⇒ CONNECTING 一律早退，**不会**被摘。所以这是本轮**新引入**的行为改变。
- **可达序列（读码推演）**：① 半开被看门狗在 T0+30s 处置 → `_open()` 新 socket 处于 CONNECTING；
  ② 后端持续不可达 ⇒ CONNECTING 挂着、`_lastHeartbeatAt` 仍是 T0；
  ③ 用户在 T0+95s 再点发送 ⇒ `ChatController.send` 调 `ensureConnected()` ⇒ liveness=offline ⇒ **摘掉这次连接尝试**。
  每次发送重复一次 ⇒ **退避被反复清零、建连尝试被反复打断**（服务恢复后仍能连上，故不是致命，但恢复更慢）。
- **现有测试矩阵正好缺这一格**：`ws_heartbeat_liveness_test.dart:164` 只测了 `CONNECTING + online`（`:164`）。
- **最小修法（1 行）**：判据只在 `OPEN` 时启用 ——
  `socket.readyState == kSocketOpen ? isChannelTrustworthy(...) : isLiveSocket(socket.readyState)`
  （或在 `onclose`/`_detach` 后把 `_lastHeartbeatAt = null`）。
- 我**没有**改任何代码去验证修法（只读授权），修法正确性属**未核实**。

### F-V1-2（P4）`sendFailedLocally` 在 `send()` **早退**时是陈旧值 ⇒ 可能出现**误清相位**

```dart
// chat_controller.dart
:107    if (text.isEmpty || _streaming) return;   // ← 早退在清零**之前**
:119    _sendFailedLocally = false;               // ← 清零在这里
// shell_chat.dart
:43     _ui.markTurnAccepted();                   // 先置 turnActive
:44     await _chat.send(text);                   // 早退：什么都没发
:53     if (_chat.sendFailedLocally) _ui.markTurnFailed();  // 读到**上一轮**的 true
```
- 可达序列：① 第 N 轮本地失败 ⇒ 标志 `true`（`:168`）；② 一个**外部/注入**轮次开始
  （`acceptInjectedUserTurn` `:220`，或 `_appendDelta` `:365`、`_applyTextFallback` `:411`、`onWsEvent` `:287`）
  ⇒ `_streaming = true` 而**标志没有被清**；③ 用户此时发送 ⇒ `send()` 在 `:107` 早退 ⇒ shell 读到陈旧的 `true`
  ⇒ `markTurnFailed()` **清掉正在飞的外部轮次相位**（探针 C/D 证实 `markTurnFailed` 会连带清 `voiceActive`）。
- 影响：相位被**提前**清（与 F-0007-1 要修的「永久卡住」相反的坏法），直到下一帧重新置位；窗口窄但语义错。
- **最小修法（1 行）**：把 `:119` 的清零挪到 `:107` **之前**；更彻底的做法是让 `send()` 返回结果
  （`Future<bool> locallyFailed`）而不是「粘一个可读标志」——同一个改动能一并收掉实施者自报的残余竞态（探针 C）。
- 附带（**非本轮引入，仅记录**）：`:107` 早退时 `shell_chat._send` 已经 `_input.clear()`，用户文字被**静默丢弃**——
  HEAD 同样如此，属既有缺陷，不在 W1-a 范围。
- 我**未改代码验证**修法，故修法正确性属**未核实**。

## 5. 红线自检（逐条）

| 红线 | 核验 | 结论 |
|---|---|---|
| WS 帧只增不改 | `git diff 93416f0c^ 93416f0c -- shell/flutter/lib/api/ws_frame.dart shell/flutter/lib/api/ws_status.dart` → **0 行**；`ws_client.dart` 里 `_handleText`/解析分支未动帧名字 | ✅ **零改动**（硬证据） |
| 错误码两侧同源 | 新增代码没有新增错误码，也没有 `contains('ok'/'ms')` 式文案判据（新增面只在看门狗/相位） | ✅ |
| 离线优先 | 未引 CDN / 未动 `--no-web-resources-cdn` 相关；`build/web` 本轮未重建（见未核实栏） | ✅ 未触碰 |
| `clean_for_tts` 不旁路 | 未改 `crates/**`（`git diff --name-only 93416f0c^ 93416f0c -- crates` → 0） | ✅ |
| 密钥 / `.env` | 未触碰 | ✅ |
| A1–A8 / `IdleState` / `mod_count` | 未触碰 `crates/**`；`mod_count_is_five` 不受影响 | ✅ |
| 测试条数不降 | 1378 → **1407**（+29，测试文件 **0 行删除**） | ✅ |
| 无第三方依赖 | 未改 `pubspec.yaml` / 未加依赖（改动清单里没有） | ✅ |

## 6. 对实施者「诚实栏」4 项的**独立严重度判断**

| # | 实施者自报 | 我的独立判断 | 理由 |
|---|---|---|---|
| 1 | `ws_client.dart`（`package:web`）看门狗本体**无自动化测试** | **可作为已知缺口，但必须补一条**（不是「可以不管」） | 「纯函数 + 消费者」是既有架构的正确应对，我核过链也确实成立；**但**正因为 `ws_client` 不可测，把判据塞进 `ensureConnected()` 的**未测分支**就产生了 F-V1-1。建议：把 `ensureConnected` 的 reopen 决策也抽成纯函数（其形状已接近 `isChannelTrustworthy`），补 `CONNECTING+offline` 一格 |
| 2 | 浏览器平台（真 `ChatController`）测试**未成**（Windows Chrome 经 WSL 三次失败） | **完全可接受** | 环境性前提缺失；另外「不留跑不起来的 `@TestOn('browser')` 文件」是对的——工作树里上轮就出现过 `zz_probe_tmp_test.dart`（已清理）。补法写清：需 Linux 侧 Chrome 或 CI 的 chrome 平台 |
| 3 | `shell_chat.dart:53` 那 1 行**无自动化覆盖** | **可接受（与既有结构一致），但 F-V1-2 的 1 行修复应顺手做** | `ChatController` 在 VM 上**根本无法构造**（依赖 `package:web`；`grep -rln "ChatController(" test/` = 0 命中）⇒ 这是**既有**结构性缺口，不是本轮引入。但既然本轮新加的「粘标志」语义在那儿，早退陈旧就是新代码自己的缺陷 |
| 4 | 产物未重建（`build/web` 仍旧） | **属实且必须写进前置条件** | 实测：`main.dart.js` = 2026-09-28 22:30，而 5 个被改的 `lib/**` 全部**新于**它（`raw/w1a_artifact_freshness.txt`）⇒ **`/app/` 上跑的是旧代码**，任何浏览器/肉眼验收在重建前**看不到 W1-a 的效果**。重建命令：`flutter build web --release --base-href /app/ --no-web-resources-cdn` |
| 5 | 残余窄竞态（`markTurnFailed` 会顺带清外部轮次） | **低危、可接受**，但实证成立（探针 C） | 需要「本地失败 + 外部轮次并发」才可达；后果是短时相位提前清（非数据损坏）。同一根因（没有轮次令牌）与 F-V1-2 同族，建议后续用「`send()` 返回结果」一次收掉 |

## 7. 声称 vs 实测

| 声称 | 实测 | 判定 |
|---|---|---|
| `flutter analyze` 0 issue | `No issues found!` | ✅ |
| `flutter test` **1407 passed / 0 failed** | `+1407 All tests passed!`（归档副本） | ✅ |
| 基线 1378 → +29 | 新测试声明 **29** 条；测试文件 **0 行删除** | ✅ |
| 「无范围外改动」5 lib + 3 test | `git show --name-status` 逐条一致；`shell_chat.dart` 9 行（8 注释 + 1 代码） | ✅ |
| 红线：`ws_frame.dart`/`ws_status.dart` 零改动 | diff 0 行；`crates/**` 0 行 | ✅ |
| 「纯函数有消费者」 | 3 个 `heartbeatLiveness` 调用点 + 1 个 `isChannelTrustworthy` + `signals.wsConnected` 消费 | ✅ |
| 「判别力三段 + 篡改」 | 我独立做 T1/T2/T3 三种篡改，全部**变红** | ✅ 复现 |
| 缺陷已修（F-0008-1 / F-0007-1 主症状） | 主链成立：看门狗→`disconnected`→`mustReleaseTurnOnWsLoss`；本地失败→`sendFailedLocally`→`markTurnFailed` | ✅ 主症状可修 |
| 无新缺陷 | ⚠ 我发现 **F-V1-1（P3，新行为）** 与 **F-V1-2（P4，新代码语义）** | ⚠ |

## 8. 未核实栏（**不许空**）

1. **产物未重建 ⇒ `/app/` 上的 W1-a 效果未验**（前提：跑一次
   `cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn`）。
   我**没有**重建（不在本任务范围，且会与在飞的 Dart worker 抢资源/污染产物）。
2. **真机半开不可复现**：SIGSTOP / `netem` / CDP 断网等手段本环境不可做（无 Linux 侧浏览器、无 root netem 授权）。
   ⇒ 「唤醒后 30 s 内真的摘掉 socket 并重连」只有**读码级**保证 + 纯函数级回归。
3. **浏览器平台测试未跑**（同实施者 #2）：真 `ChatController` / 真 `WsClient` 的任何分支在本环境**无法执行**。
4. **F-V1-1 / F-V1-2 的修法未验证**（我只读，不能改码）；两处「最小修法」是**建议**，正确性未证。
5. **`_recoverFromHalfOpen` 与 `onclose` 的真实时序未实测**：我判断 `_detach` 清回调后旧 socket 不会再触发
   `onclose`⇒无重复重连，但这是**规范级推理**，没有真浏览器证据。
6. **`ignite.sh --check` 未跑**（无服务，同 W0）；本轮与托管层无关，但 `/app/` 的任何结论都缺这一步。
7. **W1-b/c/d 在飞**：本复核只针对 `93416f0c` 这一个 commit；工作树当时的改动（`ws_client.dart` 之外的
   `llm_section` 等）不在本报告范围内。

---

## 附：原始输出清单（`docs/audit/2026-10-01-debloat/W1/raw/`）

| 文件 | 内容 |
|---|---|
| `w1a_flutter.txt` | 归档副本 `pub get` + `analyze` + `test` 全文（250 KB，`+1407`） |
| `w1a_full_diff.txt` | `git show 93416f0c` 全文（1,020 行） |
| `w1a_copy_integrity.txt` | 归档副本 8 文件 md5 vs commit（全 SAME） |
| `w1a_probe.txt` | verifier 对抗性探针 5 条（全通过，A/B/C/D） |
| `zz_verifier_probe_test.dart` | 探针源码（可原样重跑） |
| `w1a_tamper.txt` | 独立判别力自证 T1/T2/T3（全部变红） |
| `w1a_artifact_freshness.txt` | 产物陈旧度证据（5 个 lib 文件新于 `main.dart.js`） |
| `ws_liveness_93416f0c.dart` | 判据文件快照（离线查阅用） |
