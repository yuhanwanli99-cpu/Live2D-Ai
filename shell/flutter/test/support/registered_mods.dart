/// **主仓 Mod 注册表的唯一真源**：从 Rust 静态注册表读出来（D6，2026-10-05）。
///
/// # 为什么要有这个文件
///
/// D6 的第一条是「对已删 crate 的**硬编码夹具假绿灯**」：测试里手抄一份
/// `{"id":"local-llm", ...}` / `{"id":"pet-desktop", ...}` 的夹具，
/// **crate 被物理删除之后它照样绿**（前端只是解析 JSON，谁在对面无所谓）。
/// 那种绿不能当覆盖证据——它连「后端还有没有这个 Mod」都不知道。
///
/// 修法不是把名单从 Dart 抄一份到 Dart，而是把判据接回**唯一真源**：
/// `crates/live2d-ai-desktop/src/main.rs` 的 `AVAILABLE_MOD_FACTORIES`。
///
/// # 2026-10-19：一 Mod 一本地仓库，主仓**缺省不编入任何 Mod**
///
/// 工厂表在缺省主仓里是**空切片**（七个实现各自一个本地仓库；要挂某个 Mod，
/// 由维护者把它加回依赖与工厂表）。所以「空」有两种完全不同的含义，本函数
/// **必须把它们分开**：
///
/// | 返回 | 含义 | 调用方该怎么做 |
/// | --- | --- | --- |
/// | `null` | **解析失败**：文件没了 / 找不到 `AVAILABLE_MOD_FACTORIES` 锚点 / 数组体解析不出来 / 表非空却解不出 id | 门禁坏了，**必须红** |
/// | 空集 | 解析成功、真源**就是 0 个工厂** | 现行正确状态，**不是空转** |
/// | 非空集 | 解析成功、在册 N 个工厂 | 交叉核对夹具 / 断言具体 id |
///
/// 旧语义（「返回空集 = 解析失败，调用方必须断言非空」）已删除：它在工厂表
/// 归零之后会把**正确缺省**误判成门禁失效。区分方法就是上面的 `null`。
///
/// # 已知边界（诚实记录）
///
/// 工厂 id 是由「crate 名去前缀 + `_`→`-`」推出来的（`live2d_ai_mod_voice_input`
/// → `voice-input`、`live2d_ai_mod_local_tts` → `local-tts`）——它**不是**解析
/// `FACTORY.id`（那要跑 Rust）。
///
/// # 2026-10-09：第二个工厂推不出来，只能**显式登记**
///
/// `live2d_ai_mod_local_tts` 导出**两个**工厂（CosyVoice3 与 MeloTTS），
/// 第二个的 id 是 `local-tts-melo`——同一个 crate，**机械推导不出这个 id**。
/// 所以下面留一份显式映射：**将来把某个 Mod 挂回主仓时**，同 crate 多工厂 /
/// id 与 crate 名不同源的那条必须登记在这里；解不出的工厂会让本函数返回
/// `null`（解析失败），而不是悄悄漏掉一个 id 继续绿。
/// 现行主仓工厂表为空，这份映射暂无命中项。
library;

import 'dart:io';

import 'source_scan.dart';

/// Rust 静态注册表的位置（相对 `shell/flutter` 的工作目录）。
const String kModFactoriesSource = '../../crates/live2d-ai-desktop/src/main.rs';

/// **机械推导之外的显式登记**：键是 `crate::工厂名`，值是它真正的 descriptor.id。
///
/// 只有「同一个 crate 导出多个工厂 / id 与 crate 名不同源」时才需要写在这里；
/// 写进来就会在 [registeredModIds] 里生效。[registeredModIds] 遇到推不出 id
/// 的工厂会返回 `null`（解析失败）——所以漏登记不会静默变成「少一个 id」。
const Map<String, String> kExtraFactoryIds = <String, String>{
  'live2d_ai_mod_local_tts::MELO_FACTORY': 'local-tts-melo',
};

/// 已编译进二进制、**当前真的在册**的 Mod id 集合。
///
/// - 返回 `null`：**解析失败**（文件缺失 / 找不到 `AVAILABLE_MOD_FACTORIES`
///   锚点 / 数组体解析失败 / 表非空但解不出 id）。调用方必须断言 `isNotNull`，
///   否则这条门禁就是空转。
/// - 返回空集：解析**成功**，且真源就是 0 个工厂（2026-10-19 起缺省正确状态）。
/// - 返回非空集：解析成功，在册 N 个工厂。
Set<String>? registeredModIds({String source = kModFactoriesSource}) {
  final File file = File(source);
  if (!file.existsSync()) return null;
  // 先剥注释：声明前的长头注里也写着这个名字，不剥会把锚点指错地方。
  final String src = stripCommentsAndStrings(file.readAsStringSync());
  final int at = src.indexOf('AVAILABLE_MOD_FACTORIES');
  if (at < 0) return null;
  // 锚在**赋值**上（`= &[`）：声明里的类型也含 `[`（`&[&dyn ModFactory]`），
  // 找第一个 `[` 会把类型当数组体，于是永远解出 0 个 id（空转）。
  final int eq = src.indexOf('= &[', at);
  if (eq < 0) return null;
  final int start = src.indexOf('[', eq);
  if (start < 0) return null;
  final int end = matchBracket(src, start, '[', ']');
  if (end < 0) return null;
  final String body = src.substring(start + 1, end);
  // 解析成功 + 空切片 = 主仓缺省 0 个工厂（**不是**解析失败）。
  if (body.trim().isEmpty) return <String>{};
  final Set<String> ids = <String>{};
  bool unparsed = false;
  for (final RegExpMatch m in RegExp(
    r'live2d_ai_mod_([a-z0-9_]+)::([A-Z_]+)',
  ).allMatches(body)) {
    final String crate = m.group(1)!;
    final String factory = m.group(2)!;
    final String? explicit = kExtraFactoryIds['live2d_ai_mod_$crate::$factory'];
    if (explicit != null) {
      ids.add(explicit);
      continue;
    }
    // 其余工厂一律按「crate 名 → id」推导（`::FACTORY` 是唯一约定）。
    if (factory == 'FACTORY') {
      ids.add(crate.replaceAll('_', '-'));
      continue;
    }
    // 认得工厂形态、却推不出 id 且没显式登记：宁可判解析失败，也不能漏掉一个。
    unparsed = true;
  }
  // 表非空却一个工厂都没认出来（锚点/数组体指错了）：空集不能冒充「缺省 0」。
  if (unparsed || ids.isEmpty) return null;
  return ids;
}
