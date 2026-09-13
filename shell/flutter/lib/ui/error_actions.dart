/// 错误横幅的「下一步」派生：**按错误码给出路，不猜显示文案**。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` 的 `_errorActions` **原样搬出**，
/// 只把三个动作入口（切分区 / 打断 / 重试）改成显式回调：组合根负责接线，
/// 这里只负责「什么错配什么出路」。规格 §6.6：失败必须有出路。
library;

import '../settings/settings_sections.dart';

import 'error_banner.dart';

/// 错误提示的「下一步」（规格 §6.6：失败必须有出路，不能只报告）。
///
/// **优先按 `code` 分支**（2026-09-11）：码是契约、文案会改，拿显示文案做
/// `contains` 判断迟早会漂移。最典型的是 `llm_upstream_401`——后端提示
/// 「去后端进程环境设 `api_key_env`」，界面上能做的下一步就是**跳到 LLM
/// 分区**看端点/密钥配置。只有码缺失（旧服务端 / 非链路错误）时才回退到
/// 原来的文案启发式。
List<ErrorAction> errorActionsFor(
  String? message, {
  String? code,
  required void Function(SettingsSection section) onGoto,
  required void Function() onStop,
  required void Function() onSend,
}) {
  if (message == null && code == null) return const <ErrorAction>[];
  ErrorAction goto(SettingsSection section, String label) => ErrorAction(
    label: label,
    // 走唯一入口：切分区会清掉一次性结果（P2-2）。
    onPressed: () => onGoto(section),
  );
  if (code != null) {
    if (code.startsWith('llm_') || code == 'no_supervisor') {
      return <ErrorAction>[goto(SettingsSection.llm, '去 LLM 设置')];
    }
    if (code.startsWith('tts_') || code.startsWith('decode_')) {
      return <ErrorAction>[goto(SettingsSection.tts, '去语音合成设置')];
    }
    if (code == 'busy') {
      return <ErrorAction>[
        ErrorAction(label: '打断并重发', onPressed: onStop),
      ];
    }
  }
  if (message == null) return const <ErrorAction>[];
  if (message.contains('busy') || message.contains('上一轮')) {
    return <ErrorAction>[
      ErrorAction(
        label: '打断并重发',
        onPressed: onStop,
      ),
    ];
  }
  if (message.contains('no_supervisor') || message.contains('未就绪')) {
    return <ErrorAction>[goto(SettingsSection.llm, '去 LLM 设置')];
  }
  return <ErrorAction>[
    ErrorAction(label: '重试', onPressed: onSend),
  ];
}