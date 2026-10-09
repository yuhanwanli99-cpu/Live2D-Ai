/// 常态语音检测控制器回归（纯逻辑，注入 fake，零网络 / 零浏览器）。
///
/// 覆盖：缺省唤醒词「小可爱」/ 命中才发 / 只叫唤醒词不发 / 唤醒词与正文分两次
/// 定稿也能拼上 / 致命错误停止常驻 / 会话结束自动重启 / 剥词纯函数。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/voice_api.dart';
import 'package:live2d_ai_shell/voice/speech_recognizer.dart';
import 'package:live2d_ai_shell/voice/voice_listen_controller.dart';

/// 可编程的识别器：把回调存下来，测试手动投喂结果 / 错误 / 结束。
class FakeRecognizer implements SpeechRecognizer {
  late void Function(SpeechResult) onResult;
  late void Function(String, bool) onError;
  late void Function() onEnd;

  int startCalls = 0;
  int stopCalls = 0;
  String? lang;

  @override
  Future<void> start({
    required String lang,
    required void Function(SpeechResult result) onResult,
    required void Function(String message, bool fatal) onError,
    required void Function() onEnd,
  }) async {
    startCalls++;
    this.lang = lang;
    this.onResult = onResult;
    this.onError = onError;
    this.onEnd = onEnd;
  }

  @override
  Future<void> stop() async {
    stopCalls++;
  }

  @override
  Future<void> dispose() async {}
}

/// 记录被送出的文本，返回一份可编程的结论。
class FakeSender {
  FakeSender({this.result = const VoiceTranscriptResult(ok: true, text: '正文')});

  VoiceTranscriptResult result;
  Object? throwError;
  final List<String> sent = <String>[];

  /// 每次调用是否带 `ptt: true`（P0-4 断言用）。
  final List<bool> pttFlags = <bool>[];

  Future<VoiceTranscriptResult> call(String text, {bool ptt = false}) async {
    sent.add(text);
    pttFlags.add(ptt);
    if (throwError != null) throw throwError!;
    return result;
  }
}

VoiceListenController make({
  SpeechRecognizer? recognizer,
  FakeSender? sender,
  String wake = '小可爱',
  Future<String> Function()? loader,
  void Function(String)? onBusy,
  void Function(String)? onDictation,
}) {
  return VoiceListenController(
    recognizer: recognizer,
    send: (sender ?? FakeSender()).call,
    loadWakePhrase: loader ?? () async => wake,
    onBusyResult: onBusy,
    onDictation: onDictation,
    restartDelay: Duration.zero,
    // 测试不等最后定稿窗口（生产 300ms）。
    pttFinalizeDelay: Duration.zero,
  );
}

void main() {
  test('缺省唤醒词就是「小可爱」（与 Rust gate::DEFAULT_WAKE_PHRASE 同源）', () {
    expect(kDefaultWakePhrase, '小可爱');
    expect(make().wakePhrase, '小可爱');
  });

  test('不支持（recognizer == null）：不进入 listening，并给可读错误', () async {
    final VoiceListenController c = make(recognizer: null);
    expect(c.supported, isFalse);
    await c.toggle();
    expect(c.listening, isFalse);
    expect(c.error, contains('没有语音识别'));
  });

  test('命中唤醒词 + 有正文 → 发**含唤醒词的原文**（剥词由服务端做）', () async {
    final FakeRecognizer r = FakeRecognizer();
    final FakeSender s = FakeSender();
    final VoiceListenController c = make(recognizer: r, sender: s);
    await c.toggle();
    expect(c.listening, isTrue);
    expect(r.lang, 'zh-CN');

    r.onResult(const SpeechResult(transcript: '小可爱 今天天气怎么样', isFinal: true));
    await Future<void>.delayed(Duration.zero);

    expect(s.sent, <String>['小可爱 今天天气怎么样'], reason: '送原文，服务端剥词');
    expect(c.statusLine, contains('已发送'));
    expect(c.error, isNull);
  });

  test('只叫唤醒词（没有正文）不发；纯闲聊（没有唤醒词）也不发', () async {
    // ① 只有唤醒词 → 不发空回合。
    final FakeRecognizer r1 = FakeRecognizer();
    final FakeSender s1 = FakeSender();
    final VoiceListenController c1 = make(recognizer: r1, sender: s1);
    await c1.toggle();
    r1.onResult(const SpeechResult(transcript: '小可爱', isFinal: true));
    await Future<void>.delayed(Duration.zero);
    expect(s1.sent, isEmpty);

    // ② 一句没有唤醒词的闲聊 → 不累积、不发。
    final FakeRecognizer r2 = FakeRecognizer();
    final FakeSender s2 = FakeSender();
    final VoiceListenController c2 = make(recognizer: r2, sender: s2);
    await c2.toggle();
    r2.onResult(const SpeechResult(transcript: '今天吃什么', isFinal: true));
    await Future<void>.delayed(Duration.zero);
    expect(s2.sent, isEmpty);
  });

  test('唤醒后一直没等到正文：超过窗口自动放弃，无关的话不会被发出去', () async {
    final FakeRecognizer r = FakeRecognizer();
    final FakeSender s = FakeSender();
    final VoiceListenController c = VoiceListenController(
      recognizer: r,
      send: s.call,
      loadWakePhrase: () async => '小可爱',
      restartDelay: Duration.zero,
      wakeWindow: const Duration(milliseconds: 10),
      pttFinalizeDelay: Duration.zero,
    );
    await c.toggle();
    r.onResult(const SpeechResult(transcript: '小可爱', isFinal: true));
    await Future<void>.delayed(const Duration(milliseconds: 30));
    r.onResult(const SpeechResult(transcript: '今天吃什么', isFinal: true));
    await Future<void>.delayed(Duration.zero);
    expect(s.sent, isEmpty, reason: '窗口过了就不再当正文收');
  });

  test('唤醒词与正文分两次定稿：第二句到达后一次发出', () async {
    final FakeRecognizer r = FakeRecognizer();
    final FakeSender s = FakeSender();
    final VoiceListenController c = make(recognizer: r, sender: s);
    await c.toggle();

    r.onResult(const SpeechResult(transcript: '小可爱', isFinal: true));
    await Future<void>.delayed(Duration.zero);
    expect(s.sent, isEmpty, reason: '只有唤醒词不发空回合');

    r.onResult(const SpeechResult(transcript: '把灯打开', isFinal: true));
    await Future<void>.delayed(Duration.zero);
    expect(s.sent, <String>['小可爱把灯打开']);
  });

  test('自定义唤醒词按 loader 生效（已有用户自定义不覆盖）', () async {
    final FakeRecognizer r = FakeRecognizer();
    final FakeSender s = FakeSender();
    final VoiceListenController c = make(
      recognizer: r,
      sender: s,
      loader: () async => '把窗户',
    );
    await c.toggle();
    expect(c.wakePhrase, '把窗户');

    r.onResult(const SpeechResult(transcript: '小可爱 你好', isFinal: true));
    await Future<void>.delayed(Duration.zero);
    expect(s.sent, isEmpty, reason: '缺省词不生效——用户设了「把窗户」');

    r.onResult(const SpeechResult(transcript: '把窗户关小点', isFinal: true));
    await Future<void>.delayed(Duration.zero);
    expect(s.sent, <String>['把窗户关小点']);
  });

  test('致命错误（权限 / 无麦克风）停止常驻；非致命错误保留 listening', () async {
    final FakeRecognizer r = FakeRecognizer();
    final VoiceListenController c = make(recognizer: r);
    await c.toggle();

    r.onError('没听到声音', false);
    expect(c.listening, isTrue);
    expect(c.error, '没听到声音');

    r.onError('麦克风权限被拒绝', true);
    expect(c.listening, isFalse);
    expect(c.error, '麦克风权限被拒绝');
  });

  test('会话自然结束 → 未按停止就重启（常态监听）', () async {
    final FakeRecognizer r = FakeRecognizer();
    final VoiceListenController c = make(recognizer: r);
    await c.toggle();
    expect(r.startCalls, 1);

    r.onEnd();
    await Future<void>.delayed(Duration.zero);
    expect(r.startCalls, 2, reason: 'onEnd 后必须重启（常态检测）');

    await c.stop();
    r.onEnd();
    await Future<void>.delayed(Duration.zero);
    expect(r.startCalls, 2, reason: '用户按停止后不得再重启');
  });

  test('主链忙碌（200 ok:false）如实显示，不当成成功', () async {
    final FakeRecognizer r = FakeRecognizer();
    final FakeSender s = FakeSender(
      result: const VoiceTranscriptResult(
        ok: false,
        code: 'busy',
        message: '主链忙碌',
      ),
    );
    final VoiceListenController c = make(recognizer: r, sender: s);
    await c.toggle();
    r.onResult(const SpeechResult(transcript: '小可爱 你好', isFinal: true));
    await Future<void>.delayed(Duration.zero);
    expect(c.error, contains('busy'));
    expect(c.statusLine, isNot(contains('已发送')));
  });

  group('按住说话（PTT，P0-4）', () {
    test('按住 → 松手提交带 ptt:true 的正文（不要求唤醒词）', () async {
      final FakeRecognizer r = FakeRecognizer();
      final FakeSender s = FakeSender();
      final VoiceListenController c = make(recognizer: r, sender: s);
      await c.pressStart();
      expect(c.pttActive, isTrue);
      expect(c.statusLine, contains('按住说话'));

      r.onResult(const SpeechResult(transcript: '今天天气怎么样', isFinal: true));
      await Future<void>.delayed(Duration.zero);
      expect(s.sent, isEmpty, reason: '松手前不提交');

      await c.pressRelease();
      await Future<void>.delayed(Duration.zero);
      expect(s.sent, <String>['今天天气怎么样']);
      expect(s.pttFlags, <bool>[true], reason: 'PTT 必须带 ptt:true');
      expect(c.pttActive, isFalse);
    });

    test('PTT 期间的多条定稿一起提交（不丢中间结果）', () async {
      final FakeRecognizer r = FakeRecognizer();
      final FakeSender s = FakeSender();
      final VoiceListenController c = make(recognizer: r, sender: s);
      await c.pressStart();
      r.onResult(const SpeechResult(transcript: '把灯', isFinal: true));
      r.onResult(const SpeechResult(transcript: '打开', isFinal: true));
      await c.pressRelease();
      await Future<void>.delayed(Duration.zero);
      expect(s.sent, <String>['把灯打开']);
      expect(s.pttFlags, <bool>[true]);
    });

    test('没听清（空正文）不发，并给可读提示', () async {
      final FakeRecognizer r = FakeRecognizer();
      final FakeSender s = FakeSender();
      final VoiceListenController c = make(recognizer: r, sender: s);
      await c.pressStart();
      await c.pressRelease();
      await Future<void>.delayed(Duration.zero);
      expect(s.sent, isEmpty);
      expect(c.error, contains('没听清'));
    });

    test('PTT 优先于常驻：按住时常驻让位，松手后自动恢复', () async {
      final FakeRecognizer r = FakeRecognizer();
      final VoiceListenController c = make(recognizer: r);
      await c.toggle();
      expect(c.listening, isTrue);
      expect(r.startCalls, 1);

      await c.pressStart();
      expect(c.pttActive, isTrue);
      await c.pressRelease();
      await Future<void>.delayed(Duration.zero);
      expect(c.listening, isTrue, reason: '常驻意图不被 PTT 关掉');
      expect(r.startCalls, 3, reason: '常驻 → PTT → 松手恢复各起一次');
    });

    test('主链忙：不排队，把识别到的正文交回宿主（落输入框）', () async {
      final FakeRecognizer r = FakeRecognizer();
      final FakeSender s = FakeSender(
        result: const VoiceTranscriptResult(
          ok: false,
          code: 'busy',
          message: '主链忙碌',
        ),
      );
      final List<String> fallback = <String>[];
      final VoiceListenController c = make(
        recognizer: r,
        sender: s,
        onBusy: fallback.add,
      );
      await c.pressStart();
      r.onResult(const SpeechResult(transcript: '把灯打开', isFinal: true));
      await c.pressRelease();
      await Future<void>.delayed(Duration.zero);
      expect(fallback, <String>['把灯打开']);
      expect(c.error, contains('busy'));
    });
  });

  group('播报期间暂停听（P0-4）', () {
    test('voice_started 暂停听，voice_ended 恢复常驻', () async {
      final FakeRecognizer r = FakeRecognizer();
      final VoiceListenController c = make(recognizer: r);
      await c.toggle();
      expect(r.startCalls, 1);

      await c.suspendForPlayback();
      expect(c.suspendedForPlayback, isTrue);
      r.onEnd();
      await Future<void>.delayed(Duration.zero);
      expect(r.startCalls, 1, reason: '暂停期间不自动重启');

      await c.resumeAfterPlayback();
      await Future<void>.delayed(Duration.zero);
      expect(r.startCalls, 2, reason: '播报结束恢复常驻');
    });

    test('暂停期间迟到的定稿不会被当成正文发出去', () async {
      final FakeRecognizer r = FakeRecognizer();
      final FakeSender s = FakeSender();
      final VoiceListenController c = make(recognizer: r, sender: s);
      await c.toggle();
      await c.suspendForPlayback();
      r.onResult(const SpeechResult(transcript: '小可爱 你好', isFinal: true));
      await Future<void>.delayed(Duration.zero);
      expect(s.sent, isEmpty);
    });
  });

  group('voice-input Mod 未启用（L1，2026-09-16）', () {
    VoiceListenController withMod(
      SpeechRecognizer r,
      Future<bool> Function() load,
    ) => VoiceListenController(
      recognizer: r,
      send: FakeSender().call,
      loadWakePhrase: () async => '小可爱',
      loadModEnabled: load,
      restartDelay: Duration.zero,
      pttFinalizeDelay: Duration.zero,
    );

    test('start / pressStart 都被预检拦下，并给可执行的提示', () async {
      final FakeRecognizer r = FakeRecognizer();
      final VoiceListenController c = withMod(r, () async => false);
      await c.toggle();
      expect(c.listening, isFalse, reason: 'Mod 未启用不得进入 listening');
      expect(c.error, kVoiceModDisabledMessage);
      expect(r.startCalls, 0, reason: '不得起识别会话');

      await c.pressStart();
      expect(c.pttActive, isFalse, reason: 'PTT 也走同一条语音输入端点');
      expect(c.error, kVoiceModDisabledMessage);
    });

    test('Mod 已启用：预检放行，照常 listening', () async {
      final FakeRecognizer r = FakeRecognizer();
      final VoiceListenController c = withMod(r, () async => true);
      await c.toggle();
      expect(c.listening, isTrue);
      expect(c.error, isNull);
    });

    test('预检读失败（网络 / 服务未起）：不拦，交给端点 403 兜底', () async {
      final FakeRecognizer r = FakeRecognizer();
      final VoiceListenController c = withMod(
        r,
        () async => throw StateError('offline'),
      );
      await c.toggle();
      expect(c.listening, isTrue, reason: '读不到不误拦（服务端仍是真源）');
    });
  });

  group('剥词纯函数（只用于「要不要发」；服务端仍是真源）', () {
    test('忽略空白 / 大小写', () {
      expect(containsWakePhrase('小 可爱 你好', '小可爱'), isTrue);
      expect(containsWakePhrase('hey 你好', 'Hey'), isTrue);
      expect(containsWakePhrase('随便说', '小可爱'), isFalse);
    });

    test('wakeBody：剥词 + 去开头分隔符', () {
      expect(wakeBody('小可爱，关灯', '小可爱'), '关灯');
      expect(wakeBody('小可爱 关灯', '小可爱'), '关灯');
      // P0-4 句首锚定：唤醒词在中间 → 不算命中（旧行为会误触发）。
      expect(wakeBody('请 小可爱 关灯', '小可爱'), '');
      expect(wakeBody('小可爱', '小可爱'), '');
      expect(wakeBody('没有唤醒词', '小可爱'), '');
    });

    test('空唤醒词永远不命中（不会把一切都发出去）', () {
      expect(containsWakePhrase('任何话', ''), isFalse);
      expect(wakeBody('任何话', ''), '');
    });
    test('句首锚定：中间出现唤醒词不命中（P0-4）', () {
      expect(containsWakePhrase('我昨天说小可爱好看', '小可爱'), isFalse);
      expect(containsWakePhrase('  小可爱 你好', '小可爱'), isTrue);
      expect(containsWakePhrase('小可爱 你好', '小可爱'), isTrue);
      expect(wakeBody('我昨天说小可爱好看', '小可爱'), '');
    });
  });

  group('appendVoiceText：定稿写进输入框的口径（纯函数）', () {
    test('空串 = 输入框不动（「没听清」走这条路）', () {
      expect(appendVoiceText('已有的话', ''), '已有的话');
      expect(appendVoiceText('已有的话', '   '), '已有的话');
      expect(appendVoiceText('', ''), '');
    });

    test('输入框空 → 就是定稿本身（去掉首尾空白）', () {
      expect(appendVoiceText('', '  明天几点开会 '), '明天几点开会');
      expect(appendVoiceText('   ', '明天几点开会'), '明天几点开会');
    });

    test('输入框已有文字 → 接在后面，中间补一个空格', () {
      expect(appendVoiceText('前半句', '后半句'), '前半句 后半句');
      expect(appendVoiceText('前半句 ', ' 后半句'), '前半句  后半句');
    });
  });

  group('产品听写：点一下开始、再点一下结束（2026-10-08）', () {
    test('第一次点开始录音；第二次点结束，定稿交给宿主；全程不打语音端点', () async {
      final FakeRecognizer r = FakeRecognizer();
      final FakeSender s = FakeSender();
      final List<String> handed = <String>[];
      final VoiceListenController c = make(
        recognizer: r,
        sender: s,
        onDictation: handed.add,
      );

      await c.toggleDictation();
      expect(c.dictating, isTrue);
      expect(c.listening, isTrue, reason: '按钮据此变「停」');
      expect(c.statusLine, '正在听，说完再点一次');
      expect(r.startCalls, 1);

      r.onResult(const SpeechResult(transcript: '明天几点开会', isFinal: true));
      await Future<void>.delayed(Duration.zero);
      expect(handed, isEmpty, reason: '还没点「停」就不该交稿');

      await c.toggleDictation();
      expect(c.dictating, isFalse);
      expect(c.listening, isFalse);
      expect(handed, <String>['明天几点开会']);
      expect(c.error, isNull);
      expect(s.sent, isEmpty, reason: '听写路径**不打** /api/v1/voice/transcript');
      expect(s.pttFlags, isEmpty);
    });

    test('多条定稿拼成一段（不丢中间结果）', () async {
      final FakeRecognizer r = FakeRecognizer();
      final List<String> handed = <String>[];
      final VoiceListenController c = make(recognizer: r, onDictation: handed.add);
      await c.startDictation();
      r.onResult(const SpeechResult(transcript: '把灯', isFinal: true));
      r.onResult(const SpeechResult(transcript: '打开', isFinal: true));
      await c.stopDictation();
      expect(handed, <String>['把灯打开']);
    });

    test('空定稿：宿主收到空串，控制器写「没听清，再点一次说」', () async {
      final FakeRecognizer r = FakeRecognizer();
      final List<String> handed = <String>[];
      final VoiceListenController c = make(recognizer: r, onDictation: handed.add);
      await c.startDictation();
      await c.stopDictation();
      expect(handed, <String>[''], reason: '成功与失败都走同一个回调');
      expect(c.error, '没听清，再点一次说');
      expect(c.dictating, isFalse);
    });

    test('角色正在播报：不允许开始，就地写「角色在说话，说完再听」，不开麦', () async {
      final FakeRecognizer r = FakeRecognizer();
      final VoiceListenController c = make(recognizer: r);
      await c.suspendForPlayback();
      await c.startDictation();
      expect(c.dictating, isFalse);
      expect(r.startCalls, 0, reason: '播报期间不许开麦');
      expect(c.error, '角色在说话，说完再听');
    });

    test('已在录音时开始播报：停掉识别，已听到的字仍交给宿主', () async {
      final FakeRecognizer r = FakeRecognizer();
      final List<String> handed = <String>[];
      final VoiceListenController c = make(recognizer: r, onDictation: handed.add);
      await c.startDictation();
      r.onResult(const SpeechResult(transcript: '已经说了一半', isFinal: true));
      await Future<void>.delayed(Duration.zero);

      await c.suspendForPlayback();
      expect(c.dictating, isFalse);
      expect(handed, <String>['已经说了一半'], reason: '播报开始不该把听到的字吞掉');
      expect(c.listening, isFalse);
    });

    test('不支持语音识别：给可读错误，不进 dictating', () async {
      final VoiceListenController c = make(recognizer: null);
      await c.toggleDictation();
      expect(c.dictating, isFalse);
      expect(c.error, contains('没有语音识别'));
    });
  });
}

