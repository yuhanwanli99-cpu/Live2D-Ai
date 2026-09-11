import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/api/settings_models.dart';
import 'package:live2d_ai_shell/settings/settings_controller.dart';

/// 真实抓包形状的 `GET /api/v1/settings`（v0.4.8，含 `max_tokens`）。
const String kGetJson = '''
{"llm":{"base_url":"https://api.deepseek.com/v1","model":"deepseek-flash",
"has_api_key":true,"max_tokens":512},
"tts":{"base_url":"http://127.0.0.1:8080/v1","model":null,"voice":"skystar",
"has_api_key":false,"sample_rate":24000,"channels":1},
"persona":{"system_prompt":"你是桌面上的 Live2D 桌宠。","max_history_pairs":0,
"name":"Neko","description":"一只猫","personality":"慵懒","scenario":"桌面",
"first":"喵"},
"dev_mode":false}
''';

http.Response jsonResponse(String body, int status) => http.Response.bytes(
  utf8.encode(body),
  status,
  headers: const <String, String>{
    'content-type': 'application/json; charset=utf-8',
  },
);

/// 造一个控制器，并把 PATCH 的响应交给回调决定。
({SettingsController controller, List<Map<String, Object?>> patched}) build({
  String getBody = kGetJson,
  int getStatus = 200,
  String Function(Map<String, Object?> body)? patchBody,
  int patchStatus = 200,
  Duration delay = Duration.zero,
  bool throwNetwork = false,
}) {
  final List<Map<String, Object?>> patched = <Map<String, Object?>>[];

  http.Response patchResponse(http.Request request) {
    final Map<String, Object?> body =
        jsonDecode(request.body) as Map<String, Object?>;
    patched.add(body);
    if (throwNetwork) throw const _Boom();
    if (patchStatus != 200) {
      return jsonResponse(
        '{"error":{"code":"url_invalid","message":"base_url 不合法"}}',
        patchStatus,
      );
    }
    final String payload =
        patchBody?.call(body) ??
        '{"persisted":true,"apply_status":"applied","settings":$getBody}';
    return jsonResponse(payload, 200);
  }

  final MockClient client = MockClient((http.Request request) async {
    if (delay > Duration.zero) await Future<void>.delayed(delay);
    if (request.method == 'GET') {
      if (getStatus != 200) {
        return jsonResponse(
          '{"error":{"code":"io","message":"读不到配置"}}',
          getStatus,
        );
      }
      return jsonResponse(getBody, 200);
    }
    return patchResponse(request);
  });

  return (
    controller: SettingsController(
      api: ApiClient(base: 'http://127.0.0.1:18080', client: client),
    ),
    patched: patched,
  );
}

class _Boom implements Exception {
  const _Boom();
  @override
  String toString() => '连接被拒绝';
}

void main() {
  group('加载：服务端真相进 _remote，草稿清空', () {
    test('load 后 loaded / remote 正确 / 不 dirty', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      expect(t.controller.loaded, isFalse);
      await t.controller.load();
      expect(t.controller.loaded, isTrue);
      expect(t.controller.remote!.llm.model, 'deepseek-flash');
      expect(t.controller.remote!.llm.maxTokens, 512);
      expect(t.controller.remote!.tts.voice, 'skystar');
      expect(t.controller.remote!.persona.name, 'Neko');
      expect(t.controller.dirty, isFalse);
      expect(t.controller.error, isNull);
    });

    test('加载失败 → error 有值、loaded 仍为 false（界面显示重试而不是空面板）', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build(getStatus: 500);
      await t.controller.load();
      expect(t.controller.error, isNotNull);
      expect(t.controller.loaded, isFalse);
      // 失败时**不能** dirty：否则会弹「未保存」而用户什么都没改。
      expect(t.controller.dirty, isFalse);
    });

    test('网络层失败也走 error，不抛给 UI', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build(getStatus: 200);
      // 换一个一定连不上的 base。
      final SettingsController c = SettingsController(
        api: ApiClient(base: 'http://127.0.0.1:9', client: MockClient((_) async {
          throw const _Boom();
        })),
      );
      await c.load();
      expect(c.error, contains('network_error'));
      expect(t.controller.loaded, isFalse);
    });

    test('loading 期间置位（UI 可显示骨架）', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build(delay: const Duration(milliseconds: 30));
      final Future<void> pending = t.controller.load();
      expect(t.controller.loading, isTrue);
      await pending;
      expect(t.controller.loading, isFalse);
    });
  });

  group('dirty：与「用户真的改了值」一致，不是「碰过控件」', () {
    test('改一个字段 → dirty；改回原值 → 不 dirty', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();

      t.controller.edit((SettingsDraft d) => d.ttsVoice = 'nova');
      expect(t.controller.dirty, isTrue);

      t.controller.edit((SettingsDraft d) => d.ttsVoice = 'skystar');
      expect(t.controller.dirty, isFalse, reason: '改回原值 = 没改');
    });

    test('只改一个字段，补丁里**只有**那个字段（不误伤同段其它字段）', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.ttsVoice = 'nova');

      final Map<String, Object?> json = t.controller.draft.toPatch().toJson();
      expect(json.keys, <String>['tts']);
      final Map<String, Object?> tts = json['tts']! as Map<String, Object?>;
      expect(tts, <String, Object?>{'voice': 'nova'});
      expect(tts.containsKey('base_url'), isFalse, reason: '不能顺手清掉 base_url');
      expect(tts.containsKey('api_key_env'), isFalse);
    });

    test('dirty 与键序无关（同样两个字段，顺序不同结论相同）', () async {
      final a = build();
      final b = build();
      await a.controller.load();
      await b.controller.load();

      a.controller.edit((SettingsDraft d) {
        d.ttsVoice = 'nova';
        d.llmModel = 'gpt';
      });
      b.controller.edit((SettingsDraft d) {
        d.llmModel = 'gpt';
        d.ttsVoice = 'nova';
      });
      expect(a.controller.dirty, b.controller.dirty);
      expect(
        jsonEncode(a.controller.draft.toPatch().toJson()),
        jsonEncode(b.controller.draft.toPatch().toJson()),
      );
    });

    test('TriKeep 不计入 dirty', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.llmMaxTokens = Tri.keep<int>());
      expect(t.controller.dirty, isFalse);
      expect(t.controller.draft.toPatch().isEmpty, isTrue);
    });

    test('未加载时 dirty 恒为 false（避免加载失败后弹「未保存」）', () {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      t.controller.edit((SettingsDraft d) => d.ttsVoice = 'nova');
      expect(t.controller.dirty, isFalse);
    });

    test('discard 清空草稿并回到不 dirty', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.llmModel = 'gpt');
      expect(t.controller.dirty, isTrue);
      t.controller.discard();
      expect(t.controller.dirty, isFalse);
      expect(t.controller.remote!.llm.model, 'deepseek-flash', reason: '远端真相不变');
    });
  });

  group('保存：应用 apply_status 分流（规格 §4.4）', () {
    for (final (String wire, SaveOutcome expected, String message) in <
      (String, SaveOutcome, String)
    >[
      ('applied', SaveOutcome.savedApplied, '已保存并生效'),
      ('restart_required', SaveOutcome.savedRestartRequired, '已保存，需重启生效'),
      ('queued', SaveOutcome.savedQueued, '已保存，正在生效'),
      ('no_supervisor', SaveOutcome.savedNoSupervisor, '已保存，配置将在下次启动后生效'),
    ]) {
      test('apply_status=$wire → $message', () async {
        final t = build(
          patchBody: (Map<String, Object?> _) =>
              '{"persisted":true,"apply_status":"$wire","settings":$kGetJson}',
        );
        await t.controller.load();
        t.controller.edit((SettingsDraft d) => d.llmModel = 'gpt');
        final SaveOutcome outcome = await t.controller.save();
        expect(outcome, expected);
        expect(outcome.message, message);
      });
    }

    test('persisted:false → noChange（不谎报「已保存」）', () async {
      final t = build(
        patchBody: (Map<String, Object?> _) =>
            '{"persisted":false,"apply_status":"applied","settings":$kGetJson}',
      );
      await t.controller.load();
      // 直接改草稿再手工设成与远端一致 → 服务端会回 NoChange。
      t.controller.edit((SettingsDraft d) => d.llmModel = 'gpt');
      final SaveOutcome outcome = await t.controller.save();
      expect(outcome, SaveOutcome.noChange);
      expect(outcome.isSuccess, isFalse);
    });

    test('未识别的 apply_status **不谎报已生效**（保守按需重启）', () async {
      final t = build(
        patchBody: (Map<String, Object?> _) =>
            '{"persisted":true,"apply_status":"brand_new","settings":$kGetJson}',
      );
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.llmModel = 'gpt');
      expect(await t.controller.save(), SaveOutcome.savedRestartRequired);
    });

    test('缺 apply_status（旧服务端）→ 也按需重启处理', () async {
      final t = build(
        patchBody: (Map<String, Object?> _) =>
            '{"persisted":true,"settings":$kGetJson}',
      );
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.llmModel = 'gpt');
      expect(await t.controller.save(), SaveOutcome.savedRestartRequired);
    });

    test('没有任何改动时 save 不发网络请求', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      final SaveOutcome outcome = await t.controller.save();
      expect(outcome, SaveOutcome.noChange);
      expect(t.patched, isEmpty, reason: '空补丁不该白发一次往返');
    });

    test('保存成功后用**服务端回填的** view（不是本地草稿）', () async {
      const String normalized = '''
{"llm":{"base_url":"https://api.deepseek.com/v1","model":"gpt",
"has_api_key":true,"max_tokens":512},
"tts":{"base_url":"http://127.0.0.1:8080/v1","model":null,"voice":"skystar",
"has_api_key":false,"sample_rate":24000,"channels":1},
"persona":{"system_prompt":"","max_history_pairs":0,"name":"","description":"",
"personality":"","scenario":"","first":""},
"dev_mode":false}
''';
      final t = build(
        patchBody: (Map<String, Object?> _) =>
            '{"persisted":true,"apply_status":"applied","settings":$normalized}',
      );
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.llmModel = 'gpt');
      await t.controller.save();
      expect(t.controller.remote!.llm.model, 'gpt');
      expect(
        t.controller.remote!.persona.name,
        '',
        reason: '回填必须来自响应——否则本地会显示服务端并不认可的值',
      );
      expect(t.controller.dirty, isFalse);
    });
  });

  group('保存失败：草稿必须保留（否则用户要重打一遍）', () {
    test('400 url_invalid → failed + error 文案 + 草稿还在', () async {
      final t = build(patchStatus: 400);
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.llmBaseUrl = '不是URL');
      final SaveOutcome outcome = await t.controller.save();
      expect(outcome, SaveOutcome.failed);
      expect(t.controller.error, contains('url_invalid'));
      expect(t.controller.dirty, isTrue, reason: '失败后草稿不能丢');
      expect(t.controller.draft.llmBaseUrl, '不是URL');
    });

    // 2026-09-11：上屏文案必须带上服务端给的具体错误——只显示「保存失败」
    // 等于「后端出错无具体错误代码」（用户原话）。
    test('上屏文案带上服务端错误码（messageWith）', () async {
      final t = build(patchStatus: 400);
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.llmBaseUrl = '不是URL');
      final SaveOutcome outcome = await t.controller.save();
      final String shown = outcome.messageWith(error: t.controller.error);
      expect(shown, startsWith('保存失败：'));
      expect(shown, contains('url_invalid'), reason: '码必须在文案里: $shown');
    });

    test('成功结局不吃 error 参数（避免「已保存并生效：xxx」这种怪句子）', () {
      expect(
        SaveOutcome.savedApplied.messageWith(error: 'HTTP 500 boom'),
        '已保存并生效',
      );
      expect(SaveOutcome.failed.messageWith(), '保存失败');
      expect(SaveOutcome.failed.messageWith(error: '   '), '保存失败');
    });

    test('网络失败 → failed，且不 dirty 变假', () async {
      final t = build(throwNetwork: true);
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.ttsVoice = 'nova');
      expect(await t.controller.save(), SaveOutcome.failed);
      expect(t.controller.dirty, isTrue);
      expect(t.controller.error, contains('network_error'));
    });

    test('失败后重试成功 → 草稿清空', () async {
      int calls = 0;
      final MockClient client = MockClient((http.Request request) async {
        if (request.method == 'GET') return jsonResponse(kGetJson, 200);
        calls++;
        if (calls == 1) return jsonResponse('{"error":{"code":"x"}}', 500);
        return jsonResponse(
          '{"persisted":true,"apply_status":"applied","settings":$kGetJson}',
          200,
        );
      });
      final SettingsController c = SettingsController(
        api: ApiClient(base: 'http://127.0.0.1:18080', client: client),
      );
      await c.load();
      c.edit((SettingsDraft d) => d.ttsVoice = 'nova');
      expect(await c.save(), SaveOutcome.failed);
      expect(c.dirty, isTrue);
      expect(await c.save(), SaveOutcome.savedApplied);
      expect(c.dirty, isFalse);
    });
  });

  group('三处拦截：有未保存改动时先问用户', () {
    test('不 dirty → 直接放行，且不问', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      bool asked = false;
      final bool ok = await t.controller.confirmLeave(() async {
        asked = true;
        return false;
      });
      expect(ok, isTrue);
      expect(asked, isFalse, reason: '没有改动就不该弹确认框');
    });

    test('dirty + 用户选「留下」→ 不放行，草稿保留', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.ttsVoice = 'nova');
      final bool ok = await t.controller.confirmLeave(() async => false);
      expect(ok, isFalse);
      expect(t.controller.dirty, isTrue, reason: '用户选择留下，改动当然要留');
    });

    test('dirty + 用户选「放弃」→ 放行且草稿清空', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.ttsVoice = 'nova');
      final bool ok = await t.controller.confirmLeave(() async => true);
      expect(ok, isTrue);
      expect(t.controller.dirty, isFalse);
    });
  });

  group('三态构造：草稿 → 线上 JSON（草稿层最容易出错的地方）', () {
    test('clear_api_key 走段级标志，而不是裸 null（服务端 P0-2 保护）', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.clearLlmApiKey = true);

      final Map<String, Object?> json = t.controller.draft.toPatch().toJson();
      final Map<String, Object?> llm = json['llm']! as Map<String, Object?>;
      expect(llm['clear_api_key'], isTrue);
      expect(llm.containsKey('api_key_env'), isFalse);
    });

    test('显式给了新变量名时，clear 标志不生效（显式值优先）', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      t.controller.edit((SettingsDraft d) {
        d.clearLlmApiKey = true;
        d.llmApiKeyEnv = 'MY_KEY';
      });
      final Map<String, Object?> llm =
          t.controller.draft.toPatch().toJson()['llm']! as Map<String, Object?>;
      expect(llm['api_key_env'], 'MY_KEY');
    });

    test('max_tokens = 0（不限制）与清除是两种 JSON', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();

      t.controller.edit((SettingsDraft d) => d.llmMaxTokens = Tri.set(0));
      Map<String, Object?> llm =
          t.controller.draft.toPatch().toJson()['llm']! as Map<String, Object?>;
      expect(llm['max_tokens'], 0);

      t.controller.discard();
      t.controller.edit((SettingsDraft d) => d.llmMaxTokens = Tri.clear<int>());
      llm = t.controller.draft.toPatch().toJson()['llm']! as Map<String, Object?>;
      expect(llm.containsKey('max_tokens'), isTrue);
      expect(llm['max_tokens'], isNull);
    });

    test('dev_mode 是顶层三态', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.devMode = true);
      final Map<String, Object?> json = t.controller.draft.toPatch().toJson();
      expect(json['dev_mode'], true);
    });

    test('多段同时改 → 各段互不干扰，且空段不出现', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      t.controller.edit((SettingsDraft d) {
        d.llmModel = 'gpt';
        d.personaName = 'Inu';
      });
      final Map<String, Object?> json = t.controller.draft.toPatch().toJson();
      expect(json.keys.toSet(), <String>{'llm', 'persona'});
      expect(json.containsKey('tts'), isFalse, reason: '没碰 tts 就不该出现空段');
    });
  });

  group('PATCH 的 400 分流：url_invalid 与 invalid_env_name 要能分辨', () {
    test('invalid_env_name → error 里带得出这个 code（界面据此提示对应字段）', () async {
      final MockClient client = MockClient((http.Request request) async {
        if (request.method == 'GET') return jsonResponse(kGetJson, 200);
        return jsonResponse(
          '{"error":{"code":"invalid_env_name",'
          '"message":"api_key_env 非法","details":{"section":"llm"}}}',
          400,
        );
      });
      final SettingsController c = SettingsController(
        api: ApiClient(base: 'http://x', client: client),
      );
      await c.load();
      c.edit((SettingsDraft d) => d.llmApiKeyEnv = '1-BAD NAME');
      expect(await c.save(), SaveOutcome.failed);
      expect(c.error, contains('invalid_env_name'));
      expect(c.dirty, isTrue, reason: '失败后草稿要留着，用户只要改一处');
    });

    test('超时（抛 TimeoutException）→ failed 且不抛给 UI', () async {
      final MockClient client = MockClient((http.Request request) async {
        if (request.method == 'GET') return jsonResponse(kGetJson, 200);
        throw TimeoutException('12s');
      });
      final SettingsController c = SettingsController(
        api: ApiClient(base: 'http://x', client: client),
      );
      await c.load();
      c.edit((SettingsDraft d) => d.ttsVoice = 'nova');
      expect(await c.save(), SaveOutcome.failed);
      expect(c.error, isNotNull);
      expect(c.dirty, isTrue);
    });

    test('加载超时也走 error，不抛', () async {
      final SettingsController c = SettingsController(
        api: ApiClient(
          base: 'http://x',
          client: MockClient((_) async => throw TimeoutException('12s')),
        ),
      );
      await c.load();
      expect(c.error, isNotNull);
      expect(c.loaded, isFalse);
    });
  });

  group('GET / PATCH 乱序：响应回来得晚也不能覆盖新状态', () {
    test('load → save → 新的 load，最终 remote 是最后一次 load 的结果', () async {
      int gets = 0;
      final MockClient client = MockClient((http.Request request) async {
        if (request.method == 'GET') {
          gets++;
          // 第二次 GET 回一个不同的 model，用来分辨「哪一次的响应生效了」。
          return jsonResponse(
            gets == 1 ? kGetJson : kGetJson.replaceAll('deepseek-flash', 'later-model'),
            200,
          );
        }
        return jsonResponse(
          '{"persisted":true,"apply_status":"applied","settings":$kGetJson}',
          200,
        );
      });
      final SettingsController c = SettingsController(
        api: ApiClient(base: 'http://x', client: client),
      );
      await c.load();
      expect(c.remote!.llm.model, 'deepseek-flash');

      c.edit((SettingsDraft d) => d.ttsVoice = 'nova');
      expect(await c.save(), SaveOutcome.savedApplied);
      expect(c.dirty, isFalse);

      await c.load();
      expect(c.remote!.llm.model, 'later-model', reason: '最后一次 load 才是真相');
      expect(c.dirty, isFalse, reason: 'load 会丢弃草稿');
    });

    test('load 会丢弃未保存草稿（调用方必须先问用户——见 confirmLeave）', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build();
      await t.controller.load();
      t.controller.edit((SettingsDraft d) => d.ttsVoice = 'nova');
      expect(t.controller.dirty, isTrue);
      await t.controller.load();
      expect(t.controller.dirty, isFalse);
    });
  });

  group('错误清理', () {
    test('clearError 同时清掉 lastOutcome；本来就空则不通知', () async {
      final ({SettingsController controller, List<Map<String, Object?>> patched}) t =
          build(getStatus: 500);
      await t.controller.load();
      expect(t.controller.error, isNotNull);

      int notified = 0;
      t.controller.addListener(() => notified++);
      t.controller.clearError();
      expect(t.controller.error, isNull);
      expect(notified, 1);

      t.controller.clearError();
      expect(notified, 1, reason: '已经干净了就不该再通知');
    });
  });
}
