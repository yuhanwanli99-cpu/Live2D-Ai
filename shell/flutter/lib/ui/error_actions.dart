/// 错误横幅的「下一步」派生：**按错误码给出路，不猜显示文案**。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` 的 `_errorActions` **原样搬出**，
/// 只把三个动作入口（切分区 / 打断 / 重试）改成显式回调：组合根负责接线，
/// 这里只负责「什么错配什么出路」。规格 §6.6：失败必须有出路。
///
/// 2026-10-05（F-0007-2，审计 2026-09-28）：busy 那条路的入口从「只打断」
/// 换成**组合的一份**——旧实现 `onPressed: onStop` 只调 stop，按钮却写着
/// 「打断并重发」⇒ 用户按下去只看到「已打断」，那条消息**没有被重新发出去**。
/// 现在由 [interruptAndResend] 按序真的做两件事，接线见 `main.dart`。
library;

import '../settings/settings_sections.dart';

import 'error_banner.dart';

/// busy 的唯一出路：**先打断，再重发**（F-0007-2）。
///
/// # 顺序是契约（不是风格）
///
/// 1. **先打断**——`stop` 会中止服务端在飞的那一轮（`POST /api/v1/chat/stop`）。
///    不先做，重发的新请求会撞上**同一个** busy（这正是用户按下这个按钮时
///    面对的那个 429）。
/// 2. **再重发**——把用户刚发的那条原样再发一次。`stop` 还没回来就发，
///    等于把两个请求的先后交给 TCP/调度去掷骰子。
///
/// # 为什么单独成一个函数
///
/// 这**就是** F-0007-2 的缺陷面：旧实现只有第 1 步。抽成两行纯编排之后，
/// 「两步都被调用、且顺序是 stop → resend」可以在 VM 里被钉住
/// （`test/error_action_busy_resend_test.dart`）；而 `main.dart` 是
/// `package:web` + `part` 组合根，VM 里 import 不了，只负责接线。
///
/// [resend] 的返回值（是否真的发起了）不在这里判：它属于下一层的状态判断
/// （见 `ChatController.resendLastUserMessage` 与 `shell_chat` 的接线）。
Future<void> interruptAndResend({
  required Future<void> Function() stop,
  required Future<void> Function() resend,
}) async {
  await stop();
  await resend();
}

/// 错误提示的「下一步」（规格 §6.6：失败必须有出路，不能只报告）。
///
/// **优先按 `code` 分支**（2026-09-11）：码是契约、文案会改，拿显示文案做
/// `contains` 判断迟早会漂移。最典型的是 `llm_upstream_401`——后端提示
/// 「去后端进程环境设 `api_key_env`」，界面上能做的下一步就是**跳到 LLM
/// 分区**看端点/密钥配置。只有码缺失（旧服务端 / 非链路错误）时才回退到
/// 原来的文案启发式。
///
/// [onInterruptAndResend] 而不是「一个 onStop」：busy 的出路是**两件事**
/// （F-0007-2）。类型是 `void Function()`（与其它动作一致，UI 层不等它），
/// 真正的按序实现在 [interruptAndResend]。
List<ErrorAction> errorActionsFor(
  String? message, {
  String? code,
  required void Function(SettingsSection section) onGoto,
  required void Function() onInterruptAndResend,
  required void Function() onSend,
}) {
  if (message == null && code == null) return const <ErrorAction>[];
  ErrorAction goto(SettingsSection section, String label) => ErrorAction(
    label: label,
    // 走唯一入口：切分区会清掉一次性结果（P2-2）。
    onPressed: () => onGoto(section),
  );
  ErrorAction busy() => ErrorAction(
    label: '打断并重发',
    // **两件事**：打断 + 重发（见 [interruptAndResend]）。
    onPressed: onInterruptAndResend,
  );
  if (code != null) {
    if (code.startsWith('llm_') || code == 'no_supervisor') {
      return <ErrorAction>[goto(SettingsSection.llm, '去 LLM 设置')];
    }
    if (code.startsWith('tts_') || code.startsWith('decode_')) {
      return <ErrorAction>[goto(SettingsSection.tts, '去语音合成设置')];
    }
    if (code == 'busy') {
      return <ErrorAction>[busy()];
    }
  }
  if (message == null) return const <ErrorAction>[];
  if (message.contains('busy') || message.contains('上一轮')) {
    return <ErrorAction>[busy()];
  }
  if (message.contains('no_supervisor') || message.contains('未就绪')) {
    return <ErrorAction>[goto(SettingsSection.llm, '去 LLM 设置')];
  }
  return <ErrorAction>[ErrorAction(label: '重试', onPressed: onSend)];
}
