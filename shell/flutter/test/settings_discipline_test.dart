/// 0.3.6 T3（2026-10-18）· 设置纪律：**改动只进草稿，只有「保存并重载」生效**。
///
/// # 规则（真源：`docs/architecture/CURRENT.md` §5.1 / AGENTS 前端层）
///
/// - 设置**表单字段**的所有修改只进草稿；切换分区 / 关闭设置 / 点取消
///   **零写盘、零下发**（不 PATCH、不写本机存储、不向舞台/音频发帧）；
/// - 只有「保存并重载」在**全部成功**之后才生效，并且只刷新一次；
///   保存失败草稿保留、舞台不变；
/// - 例外只有两个：模型 activate（独立立即动作）与 `dev_mode` 调试临时覆盖
///   （显式开发工具）。显式动作按钮（保存 .env 密钥 / Mod 启停 / Mod 卡片
///   「保存并应用」）保留各自立即语义，**不属**切分区/关设置/取消的副作用。
///
/// # 这个文件钉什么（全部是**可失败计数断言**，没有空断言）
///
/// 1. `ActionScalesSyncer.scheduleSaved`：同值不排帧 ⇒ 拖 20 次草稿 0 帧、
///    保存成功 1 帧、保存失败 0 帧、放弃 0 帧；
/// 2. `SaveFlow`：成功 = 1 PATCH + 1 偏好写 + 1 下发 + 1 刷新；
///    失败 = 0 下发 / 0 刷新（服务端失败时偏好也不写）；
/// 3. 本模型覆盖（D40）也走草稿：单键 / 开关 / 恢复跟随全局都只进
///    `SettingsDraft.actionModels`，与全局三键同一个 PATCH；
/// 4. 同形宿主（`_gotoSection` / `_discardPrefsDraft` 的接线在 `main.dart`，
///    VM 里加载不了 —— 先例 `display_prefs_style_change_detection_test.dart`）：
///    关设置放弃 / 留下 / 保存并离开三条路的计数；
/// 5. 源码扫描：接线本身（`_saveAll` 只经 SaveFlow、放弃不补发、草稿不进舞台载荷）。
library;

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/live2d/action_scales_sync.dart';
import 'package:live2d_ai_shell/settings/save_flow.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

/// 一份 GET 响应：全局三键 + 当前模型 + 可选的模型覆盖。
String settingsJson({
  double head = 0.75,
  double body = 0.80,
  double expression = 1.0,
  String activeModelId = 'bai',
  Map<String, Object?> models = const <String, Object?>{},
}) => jsonEncode(<String, Object?>{
  'llm': <String, Object?>{
    'base_url': 'https://api.deepseek.com/v1',
    'model': 'deepseek-flash',
    'has_api_key': true,
    'max_tokens': 512,
  },
  'tts': <String, Object?>{
    'base_url': 'http://127.0.0.1:8080/v1',
    'model': null,
    'voice': 'skystar',
    'has_api_key': false,
    'sample_rate': 24000,
    'channels': 1,
  },
  'persona': <String, Object?>{'system_prompt': '', 'max_history_pairs': 0},
  'active_model_id': activeModelId,
  'action': <String, Object?>{
    'head_scale': head,
    'body_scale': body,
    'expression_scale': expression,
    'models': models,
  },
  'dev_mode': false,
});

http.Response jsonResponse(String body, int status) => http.Response.bytes(
  utf8.encode(body),
  status,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

/// 造控制器：GET 回 [getBody]；PATCH 记账并按补丁回填一个「服务端已接受」的 view。
///
/// [patchStatus] 非 200 → PATCH 失败（`settings_controller.save` 返回 failed）。
({SettingsController controller, List<Map<String, Object?>> patched}) build({
  String? getBody,
  int patchStatus = 200,
}) {
  final String get = getBody ?? settingsJson();
  final List<Map<String, Object?>> patched = <Map<String, Object?>>[];
  final MockClient client = MockClient((http.Request request) async {
    if (request.method == 'GET') return jsonResponse(get, 200);
    final Map<String, Object?> body =
        jsonDecode(request.body) as Map<String, Object?>;
    patched.add(body);
    if (patchStatus != 200) {
      return jsonResponse(
        '{"error":{"code":"io","message":"写不进去"}}',
        patchStatus,
      );
    }
    // 回填：把补丁里的显式值覆盖到 base 上（够用即止，不实现完整 merge）。
    final Map<String, Object?> base =
        jsonDecode(get) as Map<String, Object?>;
    final Object? actionPatch = body['action'];
    if (actionPatch is Map) {
      final Map<String, Object?> action =
          (base['action']! as Map).cast<String, Object?>();
      for (final String key in <String>[
        'head_scale',
        'body_scale',
        'expression_scale',
      ]) {
        final Object? v = actionPatch[key];
        if (v is num) action[key] = v.toDouble();
      }
      final Object? models = actionPatch['models'];
      if (models is Map) {
        final Map<String, Object?> merged =
            (action['models'] as Map?)?.cast<String, Object?>() ??
                <String, Object?>{};
        models.forEach((Object? rawId, Object? patch) {
          final String id = '$rawId';
          if (patch == null) {
            merged.remove(id);
            return;
          }
          final Map<String, Object?> row =
              (merged[id] as Map?)?.cast<String, Object?>() ??
                  <String, Object?>{};
          (patch as Map).forEach((Object? k, Object? v) => row['$k'] = v);
          merged[id] = row;
        });
        action['models'] = merged;
      }
      base['action'] = action;
    }
    return jsonResponse(
      jsonEncode(<String, Object?>{
        'persisted': true,
        'apply_status': 'applied',
        'settings': base,
      }),
      200,
    );
  });
  return (
    controller: SettingsController(api: ApiClient(client: client)),
    patched: patched,
  );
}

/// 生产载荷来源：**只读 remote**（草稿不进）。宿主模拟与 shell_prefs 同口径。
Map<String, double>? savedPayload(SettingsController c) {
  final ActionSettingsView? saved = c.remote?.action;
  return saved == null ? null : toActionScalesPayload(saved);
}

/// 等过防抖窗口（真实定时器；一次 150ms，等 250ms 留余量）。
Future<void> settleDebounce() =>
    Future<void>.delayed(kActionScalesDebounce * 2);

void main() {
  group('A. 舞台幅度只吃已保存值（计数）', () {
    test('拖 20 次草稿 = 0 帧；保存成功 = 1 帧；保存失败 = 0 帧；放弃 = 0 帧', () async {
      final built = build();
      final SettingsController controller = built.controller;
      final List<Map<String, double>> frames = <Map<String, double>>[];
      final ActionScalesSyncer syncer = ActionScalesSyncer(frames.add);
      // 与生产监听器同形：settings 每次通知都过一遍「已保存值」下发入口。
      controller.addListener(
        () => syncer.scheduleSaved(savedPayload(controller)),
      );

      // 首次加载：载荷从 null 变成磁盘值 —— 这是唯一一次「装载即下发」。
      await controller.load();
      await settleDebounce();
      expect(frames, hasLength(1), reason: '设置加载完成后舞台要拿到已保存幅度');
      expect(frames.single['head'], 0.75);

      // 拖动 5 次、每次都停过防抖窗口：已保存值没变 ⇒ 连一帧都不许有
      // （只靠 debounce 合并是不够的：慢速拖动/中途停顿也会走同样的通知路）。
      for (int i = 1; i <= 5; i++) {
        controller.edit(
          (SettingsDraft d) => d.actionHeadScale = 0.4 + i * 0.3,
        );
        expect(
          syncer.hasScheduledFrame,
          isFalse,
          reason: '第 $i 次拖动连待发帧都不许排（已保存值没变）',
        );
        await settleDebounce();
        expect(
          frames,
          hasLength(1),
          reason: '第 $i 次拖动后不得下发（T3：舞台只吃已保存值）',
        );
      }
      expect(built.patched, isEmpty, reason: '拖动也不得 PATCH');

      // 保存成功：1 次 PATCH、1 次下发（SaveFlow 的 syncStage，去重版）。
      await controller.save();
      expect(built.patched, hasLength(1), reason: '保存 = 恰好一次 PATCH');
      syncer.syncNow(savedPayload(controller));
      expect(frames, hasLength(2), reason: '保存成功 = 恰好一次幅度下发');
      await settleDebounce();
      expect(frames, hasLength(2), reason: '防抖帧与同步帧同值，不得重复下发');
      expect(
        frames.last['head'],
        closeTo(1.9, 1e-9),
        reason: '下发的是服务端回填的已保存值',
      );

      // 再改一次 → 保存失败：0 次 PATCH? 不，失败 = 1 次尝试但 0 次下发。
      final builtFail = build(patchStatus: 500);
      // 这里换一个失败宿主，重新挂监听（上面的 frames 只观测成功宿主）。
      final SettingsController fail = builtFail.controller;
      final List<Map<String, double>> failFrames = <Map<String, double>>[];
      final ActionScalesSyncer failSyncer = ActionScalesSyncer(failFrames.add);
      fail.addListener(
        () => failSyncer.scheduleSaved(savedPayload(fail)),
      );
      await fail.load();
      await settleDebounce();
      expect(failFrames, hasLength(1));
      fail.edit((SettingsDraft d) => d.actionBodyScale = 0.3);
      final SaveOutcome outcome = await fail.save();
      expect(outcome, SaveOutcome.failed);
      // 失败路径 **不调用** syncStage（SaveFlow 的判据）。
      await settleDebounce();
      expect(
        failFrames,
        hasLength(1),
        reason: '保存失败 = 0 次下发（舞台不变）',
      );
      expect(fail.dirty, isTrue, reason: '保存失败草稿必须保留');
      expect(fail.draft.actionBodyScale, 0.3);

      // 放弃草稿：也不下发。
      fail.discard();
      await settleDebounce();
      expect(failFrames, hasLength(1), reason: '放弃草稿 = 0 次下发');
      syncer.dispose();
      failSyncer.dispose();
    });

    test('scheduleSaved 纯逻辑：同值不排帧、null 不排帧、值变了才排 1 帧', () async {
      final List<Map<String, double>> frames = <Map<String, double>>[];
      final ActionScalesSyncer syncer = ActionScalesSyncer(frames.add);
      const Map<String, double> a = <String, double>{
        'head': 0.75,
        'body': 0.80,
        'expression': 1.0,
      };
      const Map<String, double> b = <String, double>{
        'head': 1.2,
        'body': 0.80,
        'expression': 1.0,
      };

      syncer.scheduleSaved(null);
      await settleDebounce();
      expect(frames, isEmpty, reason: '未加载 → 不下发（渲染面用出厂默认）');

      // 首次推入已保存值要排一帧。
      syncer.scheduleSaved(a);
      expect(syncer.hasScheduledFrame, isTrue, reason: '首次推入已保存值必须排帧');
      await settleDebounce();
      expect(frames, hasLength(1), reason: '首次推入 = 1 帧');

      // 之后每次推同值之间都停过防抖窗口：只有「同值不排帧」才能保持 0 新增。
      for (int i = 0; i < 3; i++) {
        syncer.scheduleSaved(a);
        expect(
          syncer.hasScheduledFrame,
          isFalse,
          reason: '同值第 $i 次推入不许排帧（T3：草稿变化不留待发帧）',
        );
        await settleDebounce();
        expect(frames, hasLength(1), reason: '同值第 $i 次推入仍是 1 帧');
      }

      syncer.scheduleSaved(b);
      expect(syncer.hasScheduledFrame, isTrue, reason: '值真变了才排帧');
      await settleDebounce();
      expect(frames, hasLength(2), reason: '值真变了才多 1 帧');
      expect(frames.last['head'], 1.2);
      syncer.dispose();
    });
  });

  group('B. SaveFlow：成功 1/1/1/1，失败 0 下发 0 刷新', () {
    ({SaveFlow flow, Map<String, int> count}) flowOf({
      required bool modDirty,
      required bool serverDirty,
      required bool prefsDirty,
      SaveOutcome serverOutcome = SaveOutcome.savedApplied,
      bool prefsWriteOk = true,
      List<String> failedMods = const <String>[],
    }) {
      final Map<String, int> count = <String, int>{
        'mods': 0,
        'server': 0,
        'prefs': 0,
        'commit': 0,
        'sync': 0,
        'reload': 0,
      };
      return (
        flow: SaveFlow(
          modDirty: modDirty,
          submitModDrafts: () async {
            count['mods'] = count['mods']! + 1;
            return failedMods;
          },
          serverDirty: () => serverDirty,
          saveServer: () async {
            count['server'] = count['server']! + 1;
            return serverOutcome;
          },
          prefsDirty: () => prefsDirty,
          savePrefs: () {
            count['prefs'] = count['prefs']! + 1;
            return prefsWriteOk;
          },
          onPrefsSaved: () => count['commit'] = count['commit']! + 1,
          syncStage: () => count['sync'] = count['sync']! + 1,
          reload: (SaveOutcome _) => count['reload'] = count['reload']! + 1,
        ),
        count: count,
      );
    }

    test('三份草稿都脏 + 全部成功：1 PATCH + 1 偏好写 + 1 下发 + 1 刷新', () async {
      final t = flowOf(modDirty: true, serverDirty: true, prefsDirty: true);
      final SaveFlowResult r = await t.flow.run();
      expect(t.count, <String, int>{
        'mods': 1,
        'server': 1,
        'prefs': 1,
        'commit': 1,
        'sync': 1,
        'reload': 1,
      });
      expect(r.synced, isTrue);
      expect(r.reloadRequested, isTrue);
      expect(r.outcome.isSuccess, isTrue);
    });

    test('服务端失败：0 偏好写 / 0 下发 / 0 刷新（草稿由控制器保留）', () async {
      final t = flowOf(
        modDirty: false,
        serverDirty: true,
        prefsDirty: true,
        serverOutcome: SaveOutcome.failed,
      );
      final SaveFlowResult r = await t.flow.run();
      expect(t.count['server'], 1);
      expect(t.count['prefs'], 0, reason: '服务端失败时偏好也不写（两件事一起生效）');
      expect(t.count['sync'], 0);
      expect(t.count['reload'], 0);
      expect(r.outcome, SaveOutcome.failed);
    });

    test('偏好写失败：服务端已写但 0 下发 / 0 刷新 / 草稿保留', () async {
      final t = flowOf(
        modDirty: false,
        serverDirty: true,
        prefsDirty: true,
        prefsWriteOk: false,
      );
      final SaveFlowResult r = await t.flow.run();
      expect(t.count['server'], 1);
      expect(t.count['prefs'], 1);
      expect(t.count['commit'], 0, reason: '写盘失败不得提交草稿（不删背景字节 / 不清草稿）');
      expect(t.count['sync'], 0);
      expect(t.count['reload'], 0);
      expect(r.prefsWriteFailed, isTrue);
      expect(r.outcome, SaveOutcome.failed);
    });

    test('什么都没改：0 PATCH / 0 下发 / 0 刷新（不白刷页面）', () async {
      final t = flowOf(modDirty: false, serverDirty: false, prefsDirty: false);
      final SaveFlowResult r = await t.flow.run();
      expect(t.count['server'], 0);
      expect(t.count['prefs'], 0);
      expect(t.count['sync'], 0);
      expect(t.count['reload'], 0);
      expect(r.outcome, SaveOutcome.noChange);
    });

    test('只改了 Mod 表单：提交 1 次 + 下发/刷新各 1 次（Mod 配置也要重载页面）', () async {
      final t = flowOf(modDirty: true, serverDirty: false, prefsDirty: false);
      await t.flow.run();
      expect(t.count['mods'], 1);
      expect(t.count['sync'], 1);
      expect(t.count['reload'], 1);
    });

    test('Mod 提交失败：不阻断保存，失败名单交给宿主上屏', () async {
      final t = flowOf(
        modDirty: true,
        serverDirty: true,
        prefsDirty: false,
        failedMods: <String>['memory'],
      );
      final SaveFlowResult r = await t.flow.run();
      expect(r.failedMods, <String>['memory']);
      expect(t.count['server'], 1);
      expect(t.count['sync'], 1);
      expect(t.count['reload'], 1);
    });

    test('写盘成功但热重载失败：0 下发（引擎仍用旧配置），刷新请求仍交给宿主', () async {
      final t = flowOf(
        modDirty: false,
        serverDirty: true,
        prefsDirty: false,
        serverOutcome: SaveOutcome.savedReloadFailed,
      );
      final SaveFlowResult r = await t.flow.run();
      expect(
        t.count['sync'],
        0,
        reason: 'S-001：引擎没装上就不许把新幅度推给舞台（不许自相矛盾）',
      );
      expect(t.count['reload'], 1, reason: '宿主仍收到刷新请求，由它决定不刷新');
      expect(r.synced, isFalse);
      expect(r.outcome, SaveOutcome.savedReloadFailed);
    });

    test('服务端 noChange 但偏好写成功：整体按 savedApplied（旧行为不变）', () async {
      final t = flowOf(
        modDirty: false,
        serverDirty: true,
        prefsDirty: true,
        serverOutcome: SaveOutcome.noChange,
      );
      final SaveFlowResult r = await t.flow.run();
      expect(r.outcome, SaveOutcome.savedApplied);
      expect(t.count['sync'], 1);
      expect(t.count['reload'], 1);
    });
  });

  group('C. 本模型覆盖（D40）也走草稿：同一个 PATCH 提交', () {
    test('单键只写该键；开关三键；恢复跟随全局 = 显式 null', () {
      final SettingsDraft d = SettingsDraft();
      d.setActionModelKey('bai', 'head_scale', 1.2);
      expect(d.toPatch().toJson(), <String, Object?>{
        'action': <String, Object?>{
          'models': <String, Object?>{
            'bai': <String, Object?>{'head_scale': 1.2},
          },
        },
      }, reason: '只写用户真动过的键：body/expression 保持「未覆盖」逐键回落全局');

      final SettingsDraft seeded = SettingsDraft();
      seeded.seedActionModelOverride(
        'bai',
        const ActionSettingsView(
          headScale: 0.9,
          bodyScale: 1.1,
          expressionScale: 1.3,
        ),
      );
      expect(
        (seeded.toPatch().toJson()['action']!
            as Map<String, Object?>)['models'],
        <String, Object?>{
          'bai': <String, Object?>{
            'head_scale': 0.9,
            'body_scale': 1.1,
            'expression_scale': 1.3,
          },
        },
        reason: '开启覆盖 = 用当前有效三键建初始覆盖（与 T3 之前的语义一致）',
      );

      final SettingsDraft cleared = SettingsDraft();
      cleared.clearActionModelOverride('bai');
      expect(cleared.toPatch().toJson(), <String, Object?>{
        'action': <String, Object?>{
          'models': <String, Object?>{'bai': null},
        },
      }, reason: '恢复跟随全局必须写显式 null（省略 = 服务端「不动」）');
    });

    test('改回已保存值 = 没改动（prune：键级 / 整份）', () async {
      final built = build(
        getBody: settingsJson(
          models: <String, Object?>{
            'bai': <String, Object?>{
              'head_scale': 1.2,
              'body_scale': null,
              'expression_scale': null,
            },
          },
        ),
      );
      final SettingsController c = built.controller;
      await c.load();

      // 拖到 1.5 再拖回 1.2：键级 prune → 不 dirty。
      c.edit((SettingsDraft d) => d.setActionModelKey('bai', 'head_scale', 1.5));
      expect(c.dirty, isTrue);
      c.edit((SettingsDraft d) => d.setActionModelKey('bai', 'head_scale', 1.2));
      expect(c.dirty, isFalse, reason: '改回磁盘值不算改动（否则一直挂「未保存」）');

      // 待删除，但磁盘上本来就没有覆盖 → 也撤回。
      c.edit((SettingsDraft d) => d.setActionModelKey('hiyori', 'head_scale', 1.4));
      c.edit((SettingsDraft d) => d.clearActionModelOverride('hiyori'));
      expect(c.dirty, isFalse, reason: '删除本来不存在的覆盖 = 没改动');
    });

    test('全局三键 + 本模型覆盖进**同一个** PATCH；失败时草稿仍在', () async {
      final built = build(
        getBody: settingsJson(
          models: <String, Object?>{
            'bai': <String, Object?>{
              'head_scale': 1.2,
              'body_scale': null,
              'expression_scale': null,
            },
            'hiyori': <String, Object?>{
              'head_scale': 0.9,
              'body_scale': null,
              'expression_scale': null,
            },
          },
        ),
      );
      final SettingsController c = built.controller;
      await c.load();
      c.edit((SettingsDraft d) => d.actionBodyScale = 1.6);
      c.edit((SettingsDraft d) => d.setActionModelKey('bai', 'expression_scale', 0.8));
      c.edit((SettingsDraft d) => d.clearActionModelOverride('hiyori'));

      await c.save();
      expect(built.patched, hasLength(1), reason: '一次保存 = 一次 PATCH');
      final Map<String, Object?> action =
          built.patched.single['action']! as Map<String, Object?>;
      expect(action['body_scale'], 1.6);
      final Map<String, Object?> models =
          (action['models']! as Map).cast<String, Object?>();
      expect(models['bai'], <String, Object?>{'expression_scale': 0.8});
      expect(models.containsKey('hiyori'), isTrue, reason: '显式 null 才会发');
      expect(models['hiyori'], isNull);

      // 失败路径：草稿保留（含模型覆盖）。
      final builtFail = build(patchStatus: 500);
      final SettingsController fail = builtFail.controller;
      await fail.load();
      fail.edit((SettingsDraft d) => d.setActionModelKey('bai', 'head_scale', 2.0));
      expect(await fail.save(), SaveOutcome.failed);
      expect(fail.dirty, isTrue);
      expect(fail.draft.actionModels['bai'], isNotNull);
    });
  });

  group('D. 同形宿主：关设置/切分区/取消的计数（与 _confirmDiscard 同形）', () {
    /// 生产 `_confirmDiscard` 的**同形**版本：脏 → 问；用户选放弃 → 丢两份草稿。
    /// 这里数的是「离开设置时到底发生了几次写/下发」（应为 0）。
    Future<({int asks, int writes, int patches, int syncs})> leave({
      required SettingsController controller,
      required ActionScalesSyncer syncer,
      required bool Function() prefsDirty,
      required void Function() discardPrefs,
      required Future<bool> Function() ask,
    }) async {
      int asks = 0;
      int writes = 0;
      final int patchesBefore = 0; // PATCH 只可能来自 controller.save，这里不调它
      if (!controller.dirty && !prefsDirty()) {
        return (asks: 0, writes: 0, patches: patchesBefore, syncs: 0);
      }
      asks++;
      if (!await ask()) {
        return (asks: asks, writes: 0, patches: patchesBefore, syncs: 0);
      }
      controller.discard();
      discardPrefs();
      // 离开不补发：舞台吃已保存值，草稿从未下发过。
      expect(syncer.sentSnapshot, isNull, reason: '本测试前提：还没下发过任何帧');
      return (asks: asks, writes: writes, patches: patchesBefore, syncs: 0);
    }

    test('用户选「留下」：0 写盘 0 PATCH 0 下发，草稿保留', () async {
      final built = build();
      final SettingsController c = built.controller;
      await c.load();
      c.edit((SettingsDraft d) => d.ttsVoice = 'nova');
      int prefsWrites = 0;
      int prefsDiscards = 0;
      bool prefsDirty = true;
      final ActionScalesSyncer syncer = ActionScalesSyncer((_) {});
      final result = await leave(
        controller: c,
        syncer: syncer,
        prefsDirty: () => prefsDirty,
        discardPrefs: () {
          prefsDiscards++;
          prefsDirty = false;
        },
        ask: () async => false,
      );
      expect(result.asks, 1);
      expect(result.writes, 0);
      expect(result.patches, 0);
      expect(result.syncs, 0);
      expect(c.dirty, isTrue, reason: '留下 → 草稿不丢');
      expect(prefsDiscards, 0);
      expect(prefsWrites, 0);
      syncer.dispose();
    });

    test('用户选「放弃」：0 写盘 0 PATCH 0 下发，两份草稿都清掉', () async {
      final built = build();
      final SettingsController c = built.controller;
      await c.load();
      c.edit((SettingsDraft d) => d.llmModel = 'gpt');
      int prefsDiscards = 0;
      bool prefsDirty = true;
      final ActionScalesSyncer syncer = ActionScalesSyncer((_) {});
      final result = await leave(
        controller: c,
        syncer: syncer,
        prefsDirty: () => prefsDirty,
        discardPrefs: () {
          prefsDiscards++;
          prefsDirty = false;
        },
        ask: () async => true,
      );
      expect(result.asks, 1);
      expect(result.writes, 0, reason: '放弃 = 0 次本机偏好写');
      expect(result.patches, 0, reason: '放弃 = 0 次 PATCH');
      expect(result.syncs, 0, reason: '放弃 = 0 次舞台下发');
      expect(c.dirty, isFalse);
      expect(prefsDiscards, 1, reason: '本机偏好草稿被丢掉（字节清理在删图路径上）');
      expect(built.patched, isEmpty);
      syncer.dispose();
    });
  });

  group('E. 源码守卫：接线本身（main.dart 在 VM 里加载不了）', () {
    final String main = stripCommentsAndStrings(
      readLibrarySource('lib/main.dart'),
    );

    String bodyOf(String anchor) {
      // anchor 不带结尾的 `(`：closureBodyAfter 自己会跳过参数括号再去取闭包体。
      final String? body = closureBodyAfter(main, anchor);
      expect(body, isNotNull, reason: '找不到 `$anchor` 的闭包体');
      return body!;
    }

    test('_gotoSection：切分区不保存 / 不写盘 / 不下发（0 副作用）', () {
      final String body = bodyOf('void _gotoSection');
      for (final String forbidden in <String>[
        '_saveAll',
        'onPrefsChanged',
        '_applyPrefs',
        'patchSettings',
        'saveDisplayPrefs',
        '_syncActionScalesNow',
        '_actionScalesSyncer.syncNow',
        // 0.3.6 T3 删掉的「覆盖防抖 PATCH」也不许回来
        '_modelOverrideCoalescer',
        'ModelOverrideCoalescer',
      ]) {
        expect(
          body.contains(forbidden),
          isFalse,
          reason: '切分区不得出现 $forbidden（规则：切分区零写盘零下发）',
        );
      }
      expect(body.contains('_ensureSettingsLoaded'), isTrue, reason: '只允许读一次设置');
    });

    test('_discardPrefsDraft：放弃只回补已保存值，不写盘、不下发', () {
      final String body = bodyOf('void _discardPrefsDraft');
      for (final String forbidden in <String>[
        '_applyPrefs',
        'onPrefsChanged',
        '_syncActionScalesNow',
        '_actionScalesSyncer',
        'patchSettings',
        'saveDisplayPrefs',
      ]) {
        expect(
          body.contains(forbidden),
          isFalse,
          reason: '放弃草稿不得出现 $forbidden（0 写盘 / 0 下发）',
        );
      }
      expect(
        body.contains('forgetBackground'),
        isTrue,
        reason: '仅删「本次草稿新加、还没进已保存偏好」的背景字节（旧计划 line31 允许）',
      );
    });

    test('_updatePrefs：只进草稿（不落盘 / 不下发 / 不改正在播的音频）', () {
      final String body = bodyOf('void _updatePrefs');
      for (final String forbidden in <String>[
        'onPrefsChanged',
        '_applyPrefs',
        '_syncActionScalesNow',
        '_stageKey',
        '_audio.',
        'patchSettings',
        'saveDisplayPrefs',
      ]) {
        expect(
          body.contains(forbidden),
          isFalse,
          reason: '改偏好不得出现 $forbidden（规则：只进草稿）',
        );
      }
    });

    test('_saveAll：只经 SaveFlow；下发与刷新各一处（成功路径）', () {
      final String body = bodyOf('Future<SaveOutcome> _saveAll');
      expect(body.contains('SaveFlow('), isTrue);
      expect(body.contains('saveServer: _settings.save'), isTrue);
      expect(body.contains('syncStage: _syncActionScalesNow'), isTrue);
      expect(
        body.contains('reloadAppPage'),
        isFalse,
        reason: '刷新只经 reload 回调（_reloadPageAfterSave），不得在 _saveAll 里直调',
      );
      expect(
        RegExp(r'_syncActionScalesNow').allMatches(body).length,
        1,
        reason: '成功路径只有一处幅度下发',
      );
    });

    test('shell_prefs：舞台载荷只读 remote（草稿不进），下发走 scheduleSaved', () {
      final String prefs = stripCommentsAndStrings(
        readLibrarySource('lib/app/shell_prefs.dart'),
      );
      expect(
        prefs.contains('_settings.remote?.action'),
        isTrue,
        reason: '载荷真源 = 已保存的 action 段',
      );
      expect(
        prefs.contains('_settings.draft'),
        isFalse,
        reason: 'shell_prefs 不得读设置草稿（拖滑条 0 次舞台 sync 的判据）',
      );
      expect(
        prefs.contains('scheduleSaved('),
        isTrue,
        reason: '已保存值变化才排帧（同值不排帧）',
      );
    });
  });
}
