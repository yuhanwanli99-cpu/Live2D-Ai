/// 「保存并重载」的**唯一编排**（0.3.6 T3，2026-10-18）。
///
/// # 它保证什么
///
/// 设置面的所有表单改动只进草稿；只有用户按「保存并重载」时，才**按顺序**
/// 走完这三步，并且「全部成功」之后才有副作用：
///
/// 1. 服务端草稿 → 一次 `PATCH /api/v1/settings`（写盘 + 热重载 / 只重启 18080）；
/// 2. 本机偏好草稿 → 一次本机存储写入（成功后才提交背景字节改动、清草稿）；
/// 3. 两样都成功 → **恰好一次**舞台幅度下发 + **恰好一次** `/app/` 刷新。
///
/// 任一步失败：**不**下发、**不**刷新、草稿保留（用户还能就地改或重试）。
///
/// # 为什么抽出来（而不是留在 `_saveAll` 里）
///
/// `shell_settings.dart` 是 `part of main.dart`：它经 `app/browser_io.dart`
/// 依赖 `package:web`，**在 `flutter test`（Dart VM）里加载不了**——
/// 写在那里的「几次 PATCH / 几次刷新」没有任何回归能钉住。本类只做顺序与
/// 副作用计数（不碰 HTTP / widget），因此「成功 1/1/1/1、失败 0/0 + 草稿保留」
/// 可以在 VM 上用**可失败计数断言**钉死（见 `test/settings_discipline_test.dart`）。
///
/// 宿主（`_saveAll`）只注入回调，不自己决定顺序：
/// - [submitModDrafts]：Mod 卡片表单里尚未提交的配置（失败只记文案，不中断）；
/// - [saveServer] / [savePrefs]：两步真正的写；
/// - [onPrefsSaved]：本机偏好写成功后的提交（删背景字节、清草稿、同步音量）；
/// - [syncStage]：保存成功后**唯一一次**动作幅度下发（吃服务端回填的已保存值）；
/// - [reload]：保存成功后**唯一一次**页面刷新（由宿主按 [SaveOutcome] 决定
///   是等重启还是等动效——刷新失败类不刷新，见 `_reloadPageAfterSave`）。
library;

import 'settings_controller.dart' show SaveOutcome;

/// 一次「保存并重载」的结果（供宿主上屏 / 决定是否离开设置）。
class SaveFlowResult {
  const SaveFlowResult({
    required this.outcome,
    this.failedMods = const <String>[],
    this.prefsWriteFailed = false,
    this.didWork = false,
    this.synced = false,
    this.reloadRequested = false,
  });

  /// 服务端保存的结局；没有服务端草稿时是 `savedApplied`（或 `noChange`）。
  final SaveOutcome outcome;

  /// Mod 表单草稿提交失败的那几张卡（文案给宿主上屏；不影响整体结局）。
  final List<String> failedMods;

  /// 本机偏好**没能写入**（无痕 / 配额满 / 存储被禁）——草稿必须保留。
  final bool prefsWriteFailed;

  /// 这次是否真的提交过东西（任何一份草稿非空）。
  final bool didWork;

  /// 是否调用了「保存成功后下发」回调（成功且真生效时恰好一次；
  /// 热重载失败时 0 次——引擎仍用旧配置）。
  final bool synced;

  /// 是否请求了刷新（成功路径恰好一次；实际是否刷新由宿主的 reload 决定）。
  final bool reloadRequested;
}

/// 「保存并重载」顺序编排器（纯逻辑、无 IO）。
class SaveFlow {
  const SaveFlow({
    required this.modDirty,
    required this.submitModDrafts,
    required this.serverDirty,
    required this.saveServer,
    required this.prefsDirty,
    required this.savePrefs,
    required this.onPrefsSaved,
    required this.syncStage,
    required this.reload,
  });

  /// 进保存前 Mod 卡片里有没有未提交的草稿（决定「提交后要不要刷新」）。
  final bool modDirty;

  /// 提交 Mod 卡片草稿（返回失败的 Mod 名；**不抛**）。
  final Future<List<String>> Function() submitModDrafts;

  /// 服务端设置草稿是否脏（运行期再判一次：PATCH 可能耗时）。
  final bool Function() serverDirty;

  /// 服务端保存（一次 PATCH）。失败返回 `SaveOutcome.failed`。
  final Future<SaveOutcome> Function() saveServer;

  /// 本机偏好草稿是否脏（运行期再判一次）。
  final bool Function() prefsDirty;

  /// 本机偏好写入（一次落盘）。失败返回 `false`。**不得**在这里改草稿。
  final bool Function() savePrefs;

  /// 本机偏好写成功后提交草稿（清草稿 / 删背景字节 / 同步音量）。
  final void Function() onPrefsSaved;

  /// 保存成功后**唯一一次**舞台幅度下发。
  final void Function() syncStage;

  /// 保存成功后**唯一一次**刷新请求。
  final void Function(SaveOutcome outcome) reload;

  /// 跑一遍。**成功 = 恰好一次下发 + 一次刷新；失败 = 都不调用。**
  Future<SaveFlowResult> run() async {
    final List<String> failedMods = await submitModDrafts();
    final bool serverWork = serverDirty();
    final bool prefsWork = prefsDirty();
    final bool anyWork = modDirty || serverWork || prefsWork;
    if (!anyWork) {
      // 没有任何草稿：不发 PATCH、不下发、不刷新（旧实现会白刷一次页面）。
      return SaveFlowResult(
        outcome: SaveOutcome.noChange,
        failedMods: failedMods,
      );
    }

    SaveOutcome outcome = SaveOutcome.savedApplied;
    if (serverWork) {
      outcome = await saveServer();
      if (outcome == SaveOutcome.failed) {
        // 服务端写失败：本机偏好**不写**（两件事是一起生效的），不刷新。
        return SaveFlowResult(outcome: outcome, failedMods: failedMods);
      }
    }
    if (prefsWork) {
      if (!savePrefs()) {
        return SaveFlowResult(
          outcome: SaveOutcome.failed,
          failedMods: failedMods,
          prefsWriteFailed: true,
        );
      }
      onPrefsSaved();
      // 服务端认为「没变化」（persisted:false）但偏好确实写进去了：整体仍算成功。
      if (outcome == SaveOutcome.noChange) outcome = SaveOutcome.savedApplied;
    }

    // 全部成功：**只在这一处**下发 + 刷新。
    //
    // 例外：写盘成功但热重载失败（S-001，`savedReloadFailed`）——引擎仍用旧配置，
    // 把新幅度推到舞台会与「实际没生效」自相矛盾；与旧实现「不刷新」同一条口径，
    // 此时**不下发**（宿主仍收到 reload 请求，由 `_reloadPageAfterSave` 决定不刷新）。
    final bool applied = outcome != SaveOutcome.savedReloadFailed;
    if (applied) syncStage();
    reload(outcome);
    return SaveFlowResult(
      outcome: outcome,
      failedMods: failedMods,
      didWork: true,
      synced: applied,
      reloadRequested: true,
    );
  }
}
