/// **在册 Mod id 的唯一真源**：从 Rust 静态注册表读出来（D6，2026-10-05）。
///
/// # 为什么要有这个文件
///
/// D6 的第一条是「对已删 crate 的**硬编码夹具假绿灯**」：测试里手抄一份
/// `{"id":"local-llm", ...}` / `{"id":"pet-desktop", ...}` 的夹具，
/// **crate 被物理删除之后它照样绿**（前端只是解析 JSON，谁在对面无所谓）。
/// 那种绿不能当覆盖证据——它连「后端还有没有这个 Mod」都不知道。
///
/// 修法不是把名单从 Dart 抄一份到 Dart，而是把判据接回**唯一真源**：
/// `crates/live2d-ai-desktop/src/main.rs` 的 `AVAILABLE_MOD_FACTORIES`
///（那也正是 Rust 侧 `mod_count_is_five` 守着的表）。crate 删了 / 改名了 /
/// 工厂 id 变了 ⇒ 引用它的夹具立刻红，而不是继续绿着骗人。
///
/// # 已知边界（诚实记录）
///
/// 工厂 id 是由「crate 名去前缀 + `_`→`-`」推出来的（`live2d_ai_mod_voice_input`
/// → `voice-input`）——它**不是**解析 `FACTORY.id`（那要跑 Rust）。在册的五个
/// crate 名与 id 目前逐一对得上；若哪天不等了，这条门禁会**报红**（而不是
/// 静默放过），届时把映射写明确即可。
library;

import 'dart:io';

import 'source_scan.dart';

/// Rust 静态注册表的位置（相对 `shell/flutter` 的工作目录）。
const String kModFactoriesSource = '../../crates/live2d-ai-desktop/src/main.rs';

/// 已编译进二进制、**当前真的在册**的 Mod id 集合。
///
/// 解析失败（文件没了 / `AVAILABLE_MOD_FACTORIES` 不在 / 一个工厂都没扫到）
/// 返回**空集**：调用方必须断言它非空，否则这条门禁就是空转
///（「零命中 ≠ 通过」——本仓已经踩过三次「读片段 → 断言整体」）。
Set<String> registeredModIds({String source = kModFactoriesSource}) {
  final File file = File(source);
  if (!file.existsSync()) return const <String>{};
  // 先剥注释：声明前的长头注里也写着这个名字，不剥会把锚点指错地方。
  final String src = stripCommentsAndStrings(file.readAsStringSync());
  final int at = src.indexOf('AVAILABLE_MOD_FACTORIES');
  if (at < 0) return const <String>{};
  // 锚在**赋值**上（`= &[`）：声明里的类型也含 `[`（`&[&dyn ModFactory]`），
  // 找第一个 `[` 会把类型当数组体，于是永远解出 0 个 id（空转）。
  final int eq = src.indexOf('= &[', at);
  if (eq < 0) return const <String>{};
  final int start = src.indexOf('[', eq);
  if (start < 0) return const <String>{};
  final int end = matchBracket(src, start, '[', ']');
  if (end < 0) return const <String>{};
  final String body = src.substring(start, end + 1);
  final Set<String> ids = <String>{};
  for (final RegExpMatch m in RegExp(
    r'live2d_ai_mod_([a-z0-9_]+)::FACTORY',
  ).allMatches(body)) {
    ids.add(m.group(1)!.replaceAll('_', '-'));
  }
  return ids;
}
