/// **草稿防抖探针**（2026-11-16 热槽记忆 M3 的前端半边）。
///
/// # 它做什么
///
/// 输入框每变一次就记下文本，**停顿 `debounce` 之后**才发一次
/// `POST /api/v1/chat/draft`。服务端**不**自己防抖（计划 §6 M3 明确分工），
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
/// `chat_controller` 会让那个已经很长的控制器再涨一块与消息流无关的状态。
library;

import 'dart:async';

import '../api/api_client.dart';

/// 缺省停顿（毫秒）。与服务端 `chat_draft::DRAFT_DEBOUNCE_MS` 同一个数字：
/// 前端停下 400 ms 才发，服务端不再等第二次。
const int kDraftDebounceMs = 400;

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

  /// 最近一次发出的草稿（诊断 / 测试；不落盘）。
  String? lastSent;

  /// 输入变化：重置计时器。**不发请求**——真正的请求在停顿后那一次发出。
  void onChanged(String text) {
    _timer?.cancel();
    _timer = Timer(debounce, () => _fire(text));
  }

  Future<void> _fire(String text) async {
    lastSent = text;
    await api.sendDraft(text, sessionId: sessionId);
  }

  /// 取消待发的那一次（切会话 / 关闭面板时调）。
  ///
  /// **不做「补发」**：切走之后那句草稿属于旧会话，替它换热卡只会把 A 的检索
  /// 结果落到 B 上。
  void cancel() {
    _timer?.cancel();
    _timer = null;
  }

  /// 发送成功后清空（下一轮的热卡已经用过了，草稿不再需要）。
  void reset() {
    cancel();
    lastSent = null;
  }

  /// 释放：取消计时器。
  void dispose() => cancel();
}
