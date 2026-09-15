/// 统一「Mod 变更 → 需重新点火 / 重启后生效」提示（L1 基座，2026-09-15）。
///
/// # 为什么要有统一的一份
///
/// 在此之前，产品里有**三条互不相干**的重启提示：设置保存的 SnackBar、模型库的
/// 「服务端称需重启生效」、Mod 配置保存的「已保存，服务端已重启」。而真正会
/// 改变运行行为的动作（启停 Mod、导入角色卡、导入/清空记忆、改唤醒短语）**一条
/// 提示都没有**——用户在设置里点完，回聊天一看没变化，只能靠猜。
///
/// 这一份把文案与入口收敛到一处，并**如实说明**什么时候真的需要重启：
/// 服务端 Mod 的热启停一般下一轮就生效，但「界面/链路没跟着变」时，
/// 标准动作就是重新点火。
///
/// # 文案纪律
///
/// - **必须**出现「重新点火」与「重启」两个词（用户口径）；
/// - **必须**给出可执行的入口（命令 / 刷新页面），不能只有一句「请重启」；
/// - 不得声称「已经重启」——宿主并不知道服务端进程状态。
library;

/// 重新点火命令（WSL2，唯一进程宿主）。
const String kIgniteCommand = './scripts/ignite.sh';

/// 提示标题（用户口径的原文）。
const String kRestartNoticeTitle = '需重新点火 / 重启后生效';

/// 一句可执行的处置。
const String kRestartNoticeHint = '在 WSL2 重跑 $kIgniteCommand；或按 F5 刷新页面。';

/// 组装一条完整提示。
///
/// [what] 是「刚刚发生了什么」的过去式短语（如 `已启用 persona`），
/// **由调用方给出**——统一层不猜动作，避免出现「已保存」却不知道保存了什么。
String modRestartNoticeText(String what) {
  final String head = what.trim().isEmpty ? 'Mod 变更已提交' : what.trim();
  return '$head。$kRestartNoticeTitle：$kRestartNoticeHint';
}

/// SnackBar 用的短文案（一行放得下）。
String modRestartSnackText(String what) {
  final String head = what.trim().isEmpty ? 'Mod 变更已提交' : what.trim();
  return '$head · $kRestartNoticeTitle（$kIgniteCommand）';
}
