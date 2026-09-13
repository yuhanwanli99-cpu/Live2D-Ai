/// 一轮的生命周期判据：**纯逻辑，零 Flutter / 零 `package:web` 依赖**。
///
/// 抽出来的理由与 `audio/epoch_gate.dart`、`audio/sample_rate.dart` 完全一样：
/// `chat_controller.dart` 经 `ws_client.dart` 间接依赖 `package:web`，于是
/// **它里面的任何分支在 `flutter test` 里都跑不到**。而下面这两个判据都是
/// 「错了就静默坏掉」的类型，恰恰是最该有回归的那一类。
///
/// # 为什么会有这个文件（2026-09-11，用户报「第一次对话之后没输出了」）
///
/// 现场：一轮**正常完成**（`turn_state{completed}`、没有任何 `error` 帧）、
/// 但**一个字都没有**——当时 LLM 调用了 `perform_action` 工具而没有输出文字，
/// TTS 因此一次合成请求都没收到（CosyVoice 日志里那一轮是空的）。
/// （2026-09-11 同日后续：LLM 已裁定为**无工具、只做对话**，动作子系统整个
/// 移除，所以「模型去调工具所以没说话」这条原因不再存在；但「一轮正常完成
/// 却没有文字」这件事本身仍要能被看见，下面这三条规则与原因无关，继续成立。）
///
/// 前端的处理曾经是「空气泡直接删掉」，理由是「不许伪造模型没说过的内容」。
/// 那条理由是对的，**结论是错的**：删掉之后界面上什么都没发生，与
/// 「应用坏了」**无法区分**——用户就是这样读的。所以现在的规则是：
///
/// - 没有文字 → 往会话里放一条**系统行**（[TurnSettlement.wordless]）；
/// - 它不是模型说的话（`ChatRole.system`），不会被误认成回复；
/// - 但它**看得见**，于是「模型没说话」与「链路断了」从此可分辨。
library;

import '../api/ws_status.dart';

/// 一轮结束时，那条 assistant 气泡该怎么收口。
enum TurnSettlement {
  /// 有文字：原样保留（失败时那段文字就是错误说明）。
  keep,

  /// 失败了、而且没有文字：写成一条明确的失败气泡（走 danger 面色）。
  failed,

  /// 没失败、也没有文字：写成一条**系统提示**。
  ///
  /// 绝不「什么都不显示」——见文件头注。
  wordless,

  /// 用户（或别的客户端）**主动停止**了这一轮，而且一个字都还没收到。
  ///
  /// 与 [wordless] 必须分开：两者都是「没有文字的收口」，但原因完全不同——
  /// 一个是模型的产出，一个是用户的意志。用同一句话会把用户自己按的停止
  /// 说成「模型没返回文字」。
  stopped,

  /// 没有正文，**但有思考**（2026-09-13，推理模型）。
  ///
  /// 为什么单列一类：实测推理模型的思考会占用同一份输出预算，长思考的提问
  /// 会把正文挤到为空或只剩半句——而半句切不出完整句，于是**一个字都不上屏**。
  /// 这时界面上其实**有**东西可看（思考就在气泡上），只是没有正文。
  ///
  /// 处理方式与 [wordless] 不同：
  /// - [wordless] → 换成一条**系统行**（气泡里确实什么都没有）；
  /// - [keepReasoningOnly] → **保留气泡**（思考留给用户看），由气泡自己
  ///   标一句 [kReasoningOnlyCaption] 说明「只有思考、没有正文」。
  ///
  /// 若这里也换成系统行，就等于把用户最想看的那段思考**扔掉**。
  keepReasoningOnly,
}

/// 一轮收口方式的判据。
///
/// 显式传 `text` / `failed` / `stopped` 而不是接收 `ChatMessage`：这个文件要能
/// 在 VM 上测，不能依赖消息对象（它与存储层耦合）。
///
/// 「没有文字」按 `trim()` 判：只有空白（空格 / 换行）的回复在界面上同样是
/// 一个空气泡，与一个字都没有没有区别——同样必须走系统提示那条路。
///
/// 有文字时**一律保留**（哪怕这一轮失败了、哪怕是被停止的）：那是**真实收到
/// 的内容**，丢了就是篡改记录。
TurnSettlement settleTurn({
  required bool failed,
  required String text,
  bool stopped = false,
  bool hasReasoning = false,
}) {
  if (text.trim().isNotEmpty) return TurnSettlement.keep;
  if (failed) return TurnSettlement.failed;
  if (stopped) return TurnSettlement.stopped;
  // 「只有思考」优先于「什么都没有」：气泡里有真内容，不能当空气泡丢掉。
  return hasReasoning ? TurnSettlement.keepReasoningOnly : TurnSettlement.wordless;
}

/// 实时通道掉了、而**有一轮正在进行** → 必须就地收口。
///
/// 为什么必须：前端「这一轮结束了」的唯一信号是 WS 上的
/// `turn_state` / `text_delta{completed}`。帧在断连期间**不会重放**，
/// 那条收口帧一旦丢在断连窗口里，`_streaming` 就再也不会回到 false——
/// 输入框从此永远显示「停止本轮」，而 `send()` 会**静默 return**
/// （不发请求、不报错、日志里一片空白）。这就是「发出去没回应」的最坏形态。
///
/// 判据用 [WsStatus.isProblem]（`disconnected` / `closed`）而**不是**
/// `!isUsable`：`connecting` 也不可用，但它是「正在建连」——
/// 发送瞬间通道可能还在连（`send()` 里刚调过 `ensureConnected`），
/// 那时把这一轮收掉是错的（回复随后就到）。
bool mustReleaseTurnOnWsLoss({
  required bool turnInFlight,
  required WsStatus status,
}) => turnInFlight && status.isProblem;

/// 「模型整轮没有返回文字」那条系统提示的文案。
///
/// 措辞刻意只陈述**观察到的事实**（没有文字）——不写成模型的台词
/// （曾经的 `（本轮没有文字输出——通常是模型只调用了动作工具）` 是一条
/// assistant 气泡，长得和真回复一模一样）。
///
/// 2026-09-11 改：括号里原为「可能只触发了动作」。LLM 现已无工具，
/// 那句话既解释不了现象、又让用户以为还有个动作系统在跑，故删去——
/// 只说「本轮没有返回文字」，原因留给诊断日志。
const String kWordlessTurnNotice = '本轮模型没有返回文字';

/// 「只有思考、没有正文」时气泡上的说明行（2026-09-13）。
///
/// 它必须**同时**说清两件事：发生了什么（只有思考）+ 下一步能做什么
/// （调大输出上限）。只说「没有返回文字」会把用户送回原来那个坑——
/// 界面上有一条系统行，但他依然不知道模型其实想了 1200 字、也不知道去哪改。
const String kReasoningOnlyCaption =
    '本轮只有思考、没有正文（正文可能被「输出 token 上限」截断；'
    '可在「设置 → 对话模型」把它调大）';

/// 通道中断导致本轮收口时的错误码（与后端 `ErrorKind::code()` 同一约定：
/// 用户拿界面上的码去日志里搜）。
const String kWsDroppedMidTurnCode = 'ws_dropped_mid_turn';

/// 上面对应的给人看的一行。
///
/// **同一句话同时用于两处**（错误横幅 + 会话里的系统行）：横幅是暂时的，
/// 而历史是长久的——只写横幅的话，用户刷新之后只会看到「一条没有回复的
/// 消息」，又回到了「与坏了无法区分」那个坑。
const String kWsDroppedMidTurnMessage = '本轮回复未收到（实时通道断开）';

/// 用户主动停止、且一个字都还没收到时的那条系统提示。
const String kStoppedTurnNotice = '已停止本轮';

/// 「上一轮被新代次作废」导致收口时的码。
///
/// 触发点是 WS 的 `runtime_status{event:"new_epoch"}`——它的语义就是
/// 「上一轮作废」（见 `audio/epoch_gate.dart` 的 `mustInterruptAudio`）。
/// **必须在这里也收口**：停止路径在后端走的是 `RootEffect::TurnAborted`，
/// 而它**不投影任何 WS 帧**（`supervisor/handlers.rs`），也就是停止**不会**
/// 产生 `turn_state`。于是「别人（或另一个客户端）按了停止」在本客户端看来
/// 就是「这一轮永远不结束」——输入框锁死。
const String kTurnPreemptedCode = 'turn_preempted';

/// 上面对应的给人看的一行。
const String kTurnPreemptedMessage = '本轮已被中断（新的代次开始）';
