/// 动作强度出厂常量与导演按句 cue 的**持有与应用策略**（纯逻辑，VM 可测）。
///
/// # 为什么单独成文件（0.3.10）
///
/// 两块原本住在 `live2d_stage.dart`：该文件越过 xtask 的「Dart lib 单文件
/// > 800 行」硬门禁（`RATCHET_DART_800 = 0`），按现有类边界搬到这里。
/// 公开符号仍由 `live2d_stage.dart` re-export，既有调用方的
/// `import 'package:live2d_ai_shell/live2d/live2d_stage.dart'` 一行都不用改。
///
/// 行为零变化：只搬声明位置，类与常量逐字保留。
library;

import '../api/ws_frame.dart';

/// 动作预设的**出厂强度**（L1 产品级，2026-09-16）。
///
/// 1.2 让头 / 身的摆幅肉眼明显（1.0 偏「微抖」，用户实测反馈看不出来），
/// 仍在渲染面 `MAX_INTENSITY = 3.0` 的钳位内。调试面板可在 0.5~3.0 之间调。
const double kDefaultPresetIntensity = 1.2;

/// **基础表情强度**的出厂值（2026-09-23，表情调试 / 动作调试共用）。
///
/// 与手势的 [kDefaultPresetIntensity]（1.2）刻意不同：表情调试的目的是
/// 「确认基础表情强度」，1.0 就是表值本身——看得清、不放大。手势需要 1.2 才
/// 肉眼明显（用户实测 1.0 像微抖），所以两个滑条各有各的缺省，不合并。
const double kDefaultExpressionIntensity = 1.0;

/// 导演按句 cue 的**持有与应用策略**（纯逻辑，VM 可测；阶段3 起取代 W4 那条
/// 基于只读状态面的预设判据）。
///
/// # 为什么抽在这里（而不是留在 `main.dart`）
///
/// 与 PresetStatus（已搬到 `preset_status.dart`）同因：`main.dart` 经 `app/browser_io.dart` 依赖
/// `package:web`，在 `flutter test`（Dart VM）里**加载不了**；而「一份 plan
/// 整体替换、按句**取用一次即移除**、空 id 不动、`none` = 撤销」这条顺序
/// 恰恰是最容易写错、也最该被回归钉住的行为——阶段3 §6.2 明确：「同 seq
/// 重复帧只应用一次」这条保护由整表替换 + 取用即移除承担。
///
/// # 契约（阶段3 D10 / D13）
///
/// - [`replace`]：新 `action_cue` 到达即**整体替换**（一份 plan 覆盖上一份；
///   同 seq 多条只留最后一条 = map 语义）。
/// - [`take`]：该句音频**开始播放**时按 `sentence_seq` 取用一次，**取用即移除**
///   ⇒ 同一句（含同 seq 重复帧）只会被应用一次。
/// - [`applyForSeq`]：取到 cue 才下发；`preset_id` 为空串 = 「本轮不动」（不
///   调用下发）；`preset_id == 'none'` = 显式撤销，**照常下发一次**（渲染面把
///   `none` 翻成两槽 `Revoke`，与旧状态面通道的撤销同义；那条通道已退役）。
class DirectorCuePlan {
  final Map<int, ActionCue> _bySeq = <int, ActionCue>{};

  /// 当前计划里还剩多少条 cue（排障 / 测试用）。
  int get length => _bySeq.length;

  /// 新 plan 到达：**整体替换**。同 seq 的重复帧在这里被 map 语义吃掉。
  void replace(List<ActionCue> cues) {
    _bySeq
      ..clear()
      ..addEntries(
        cues.map((ActionCue c) => MapEntry<int, ActionCue>(c.sentenceSeq, c)),
      );
  }

  /// 按句子序号取用一次并移除；`seq == null` / 已取过 → `null`。
  ActionCue? take(int? seq) {
    if (seq == null) return null;
    return _bySeq.remove(seq);
  }

  /// 取用 + 下发：空 `presetId` = 本轮不动；其余（含 `'none'`）**恰好一次**。
  Future<void> applyForSeq(
    int? seq,
    Future<void> Function(ActionCue cue) apply,
  ) async {
    final ActionCue? cue = take(seq);
    if (cue == null || cue.presetId.isEmpty) return;
    await apply(cue);
  }
}
