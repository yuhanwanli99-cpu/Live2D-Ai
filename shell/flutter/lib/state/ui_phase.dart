/// UI 状态派生：**纯函数、无 web 依赖、无 `BuildContext`**。
///
/// 规格 §6.2。这是状态呈现的**唯一**真源：`StatePill`、舞台角标、聊天面板头
/// 都读同一个 [deriveUiPhase] 的结果，**不允许各自判断**——6 个开源项目的
/// 教训是「状态机层面本来就是同一个值，界面上却长出 6 套视觉」
/// （OLV-Web：6 个 AI 状态共用 1 种视觉，只有文字变）。
///
/// # 为什么状态要从信号**派生**而不是等一个 `phase` 帧
///
/// 调研曾把 core 的 `Phase`（`Idle`/`Thinking`/`Speaking`）列为「已在 WS 上的
/// 状态源」。**这不准确**：`web_api/ws/*.rs` 全文没有 `phase` 投影
/// （已实测 `grep -rn phase web_api/ws/` 零命中）。所以只能从
/// `runtime_status` + `text_delta` + `turn_state` + `epoch` 派生。
library;

import '../design/tokens.dart';

/// 界面相位（**枚举里刻意没有 `listening`**）。
///
/// 本项目当前没有输入侧：core 只有 `Idle`/`Thinking`/`Speaking`，全仓库没有
/// ASR/麦克风端点，链路是「用户**文本** → LLM → …」。规格 §6.7 的诚实结论是
/// **不设**该状态——而不是设一个永远不会出现的枚举值来假装支持。
enum UiPhase {
  /// 后端不可达（WS 未连接，或渲染面进入错误态）。
  offline,

  /// 空闲。
  idle,

  /// 思考中（本轮已受理、还没开始出声）。
  thinking,

  /// 说话中（收到 `voice_started`、未收到 `voice_ended`）。
  speaking,

  /// 刚被打断（`new_epoch` 之后的一个短窗口）。
  interrupted,

  /// 出错了（`error` 帧 / `turn_state: failed`，且用户还没关掉提示）。
  error;

  /// 界面上给人看的短标签。
  String get label => switch (this) {
    UiPhase.offline => '后端未连接',
    UiPhase.idle => '空闲',
    UiPhase.thinking => '思考中',
    UiPhase.speaking => '说话中',
    UiPhase.interrupted => '已打断',
    UiPhase.error => '出错',
  };
}

/// 派生 [UiPhase] 所需的全部信号。
///
/// 全是布尔/枚举——**没有 `BuildContext`、没有 Stream、没有时间**。
/// 「打断窗口」由调用方折算成 [interrupted] 布尔（定时器是副作用，
/// 不属于纯函数）。
class UiSignals {
  const UiSignals({
    this.wsConnected = false,
    this.turnActive = false,
    this.voiceActive = false,
    this.interrupted = false,
    this.errorActive = false,
  });

  /// `WsStatus == connected`。
  final bool wsConnected;


  /// `POST /api/v1/chat` 已受理且本轮尚未收口。
  final bool turnActive;

  /// 收到 `voice_started`、未收到 `voice_ended`。
  final bool voiceActive;

  /// 处于 `new_epoch` 后的短窗口内。
  final bool interrupted;

  /// 有未关闭的错误（`error` 帧 / `turn_state: failed`）。
  final bool errorActive;

  UiSignals copyWith({
    bool? wsConnected,
    bool? turnActive,
    bool? voiceActive,
    bool? interrupted,
    bool? errorActive,
  }) => UiSignals(
    wsConnected: wsConnected ?? this.wsConnected,
    turnActive: turnActive ?? this.turnActive,
    voiceActive: voiceActive ?? this.voiceActive,
    interrupted: interrupted ?? this.interrupted,
    errorActive: errorActive ?? this.errorActive,
  );

  @override
  String toString() =>
      'UiSignals(ws: $wsConnected, '
      'turn: $turnActive, voice: $voiceActive, interrupted: $interrupted, '
      'error: $errorActive)';
}

/// 派生界面相位。**判定顺序本身就是契约**（自上而下第一个命中即返回）。
///
/// 顺序为什么是这样（每条都有理由，不是随意排的）：
///
/// 1. **`errorActive` 最先**：错误必须能盖住一切。若排在 `voiceActive` 后面，
///    一轮中途失败时界面会一直显示「说话中」，用户看不到失败。
/// 2. **`offline` 只看「后端连不上」这一个判据**（2026-09-11 修）。
///    过去这里还有一条 `bridgeError → offline`，它有两个问题：
///    ① 渲染面（iframe）出错时后端**明明是连着的**，胶囊却写「后端未连接」
///       —— 而旁边的连接徽标写「已连接」，两个元件互相矛盾；
///    ② 那个信号**只置位、永不清除**（见 `UiStateTracker` 的旧 `onBridgePhase`），
///       于是渲染面任何一次**瞬时**错误都会把胶囊**永久**钉在错误态，
///       直到用户刷新页面。
///    渲染面自己的失败已经有**它该在的地方**：舞台上的「模型加载失败 + 重试」
///    覆盖层。同一条错误长出两个通道，正是规格 §6.3 明令禁止的。
/// 3. **`interrupted` 先于 `speaking`**：打断的意义就是「原本在说话，现在停了」。
///    若 `voiceActive` 还残留着（`voice_ended` 尚未到达），排在后面会让
///    「已打断」**永远显示不出来**。这是个容易写反的地方，有专门测试。
/// 4. **`voiceActive` 先于 `turnActive`**：一轮里「先思考后出声」，
///    出声之后仍处于本轮内，此时要显示「说话中」而不是「思考中」。
UiPhase deriveUiPhase(UiSignals s) {
  if (s.errorActive) return UiPhase.error;
  if (!s.wsConnected) return UiPhase.offline;
  if (s.interrupted) return UiPhase.interrupted;
  if (s.voiceActive) return UiPhase.speaking;
  if (s.turnActive) return UiPhase.thinking;
  return UiPhase.idle;
}

/// 「已打断」提示的展示窗口。
///
/// 取值理由：要长到人眼能看见（约一次扫视），短到不会和下一轮重叠。
/// 规格 §6.3 写的是「一次性 200 ms 淡出后回落」——那是**淡出**时长；
/// 保持态需要更久，否则 200 ms 的保持期根本来不及读。
///
/// 真源在 `design/tokens.dart` 的 [AppRhythms]（与思考呼吸、流式三点并列），
/// 这里只是给状态层一个短名字。
const Duration kInterruptedHold = AppRhythms.interruptedHold;
