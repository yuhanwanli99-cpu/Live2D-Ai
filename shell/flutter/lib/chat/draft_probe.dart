/// **草稿防抖探针**（2026-11-16 热槽记忆 M3 的前端半边）。
///
/// # 它做什么
///
/// 输入框每变一次就记下文本，**停顿 debounce 之后**才发一次
/// POST /api/v1/chat/draft。服务端**不**自己防抖（计划 §6 M3 明确分工），
/// 所以「连打时不发请求、停下来才发一次」这件事只在这里实现。
///
/// # 三条纪律
///
/// 1. **绝不阻塞发送**：草稿请求在后台跑；调用方不等它，也不因为它在跑而禁用发送键；
/// 2. **失败静默**：草稿是辅助（它的产物只影响**下一次**的 system 文本），
///    转圈或弹错只会干扰打字；
/// 3. **不落盘**：草稿文本与热卡都是**这一轮**的事，不写 localStorage、不写配置。
///
/// # 为什么单独一个文件
///
/// 它有自己的计时器与「最后一次为准」语义，和聊天面板的渲染无关——混进
/// chat_controller 会让那个已经很长的控制器再涨一块与消息流无关的状态。
///
/// # 2026-11-18 审计 #20：会话必须在**排计时器那一刻**固定
///
/// 旧实现只捕获正文，`sessionId` 留到计时器回调里现读字段。用户在 400 ms 停顿
/// 窗口里切了会话（会话切换监听顺序出一点岔子、或调用方忘了 cancel），A 的草稿
/// 就会带着 B 的 `session_id` 发出去——服务端按 body 里的 id 换热卡，于是 A 的
/// 检索结果写进 B 的热卡（「下一次发送」的 system 文本被上一个会话的检索污染）。
///
/// 现在的两条防线：
/// - [onChanged] 把（正文, 会话 id）作为**一个不可变快照**交给计时器；
/// - [_fire] 发现目标会话已经变了就**不发**：就算 cancel 漏了，A 的草稿也不会
///   落到 B 上。响应侧没有可写的会话状态（热卡是服务端状态，前端只读 lines
///   诊断），所以「响应不写别的会话」在这里等价于「请求不跨会话」。
library;

import 'dart:async';

import '../api/api_client.dart';

/// 缺省停顿（毫秒）。与服务端 `chat_draft::DRAFT_DEBOUNCE_MS` 同一个数字：
/// 前端停下 400 ms 才发，服务端不再等第二次。
/// 这个 400 ms 是**协议参数**（客户端与服务端约定的停顿时长，服务端常量
/// `chat_draft::DRAFT_DEBOUNCE_MS` 同一个数），**不是 UI 动效时长**——所以它
/// 刻意不取 `AppDurations` 的 4 档（那 4 档是为 hover / 内容切换这类交互过渡
/// 定的）。本文件挂在 `lib/chat/`，因此 `design_tokens_lint_test.dart` 需要
/// 一条路径前缀豁免：见 `kTimingExemptPrefixCaps` 里的 `lib/chat/draft_probe`。
const int kDraftDebounceMs = 400;

/// 一次待发的草稿：**排计时器那一刻**的正文与会话，之后不再变。
///
/// 会话 id 是排计时器的输入而不是回调时的环境：切会话之后读到的 id 属于下一个
/// 上下文，拿它解释上一次输入就是串台（#20）。
class _PendingDraft {
  const _PendingDraft(this.text, this.sessionId);

  final String text;
  final String? sessionId;
}

/// 草稿探针：输入 → 防抖 → 换热卡。
class DraftProbe {
  DraftProbe({
    required this.api,
    this.debounce = const Duration(milliseconds: kDraftDebounceMs),
    this.sessionId,
  });

  final ApiClient api;

  /// 停顿阈值。测试传 `Duration.zero` 以便直接断言「发了几次」。
  final Duration debounce;

  /// 当前会话（缺省 = 全局桶）。切会话时调用方要更新它并 [cancel]。
  String? sessionId;

  Timer? _timer;

  /// 待发的那一次（诊断 / 测试；不落盘）。
  _PendingDraft? _pending;

  /// 有没有待发的那一次（诊断 / 测试）。
  bool get hasPending => _pending != null;

  /// 最近一次发出的草稿（诊断 / 测试；不落盘）。
  String? lastSent;

  /// 输入变化：重置计时器。**不发请求**——真正的请求在停顿后那一次发出。
  ///
  /// **会话在这一刻固定**（#20）：之后 [sessionId] 再变，这一次也不会改投别的
  /// 会话（见 [_fire] 的第二条防线）。
  void onChanged(String text) {
    _timer?.cancel();
    final _PendingDraft pending = _PendingDraft(text, sessionId);
    _pending = pending;
    _timer = Timer(debounce, () => _fire(pending));
  }

  Future<void> _fire(_PendingDraft pending) async {
    _pending = null;
    // **响应绝不落到别的会话**（#20）：目标会话已经不是排计时器时那个 → 说明
    // 用户切走了，这一次请求没有意义（旧实现会把 A 的正文以 B 的 id 发出去）。
    if (pending.sessionId != sessionId) return;
    lastSent = pending.text;
    await api.sendDraft(pending.text, sessionId: pending.sessionId);
  }

  /// 取消待发的那一次（切会话 / 关闭面板时调）。
  ///
  /// **不做「补发」**：切走之后那句草稿属于旧会话，替它换热卡只会把 A 的检索
  /// 结果落到 B 上。
  void cancel() {
    _timer?.cancel();
    _timer = null;
    _pending = null;
  }

  /// 发送成功后清空（下一轮的热卡已经用过了，草稿不再需要）。
  void reset() {
    cancel();
    lastSent = null;
  }

  /// 释放：取消计时器。
  void dispose() => cancel();
}
