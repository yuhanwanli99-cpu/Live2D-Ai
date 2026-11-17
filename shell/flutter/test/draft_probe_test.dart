/// 草稿防抖探针的单测（2026-11-16 热槽记忆 M3 前端半边）。
///
/// 守四件事：
/// 1. **连打不发请求**：多次输入只换计时器，只有停顿之后才发**一次**；
/// 2. **最后一次为准**：停顿发出去的是最后那段文本；
/// 3. **切会话不补发**：cancel 之后旧会话的草稿不会落到新会话上；
/// 4. **会话跟随**：请求里带的是当时的活动会话 id。
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/api/api_client.dart';
import 'package:live2d_ai_shell/chat/draft_probe.dart';

/// 记下每次 sendDraft 的正文与会话，不碰网络。
class _RecordingApi extends ApiClient {
  _RecordingApi() : super(base: 'http://127.0.0.1:1');

  final List<String> sent = <String>[];
  final List<String?> sessions = <String?>[];

  @override
  Future<int?> sendDraft(String text, {String? sessionId}) async {
    sent.add(text);
    sessions.add(sessionId);
    return 0;
  }
}

void main() {
  test('连打不逐字发请求：停顿后只发一次，且是最后那段文本', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: const Duration(milliseconds: 20),
    );
    probe.onChanged('薄');
    probe.onChanged('薄荷');
    probe.onChanged('薄荷是');
    expect(api.sent, isEmpty, reason: '还没停顿，一次都不该发');

    await Future<void>.delayed(const Duration(milliseconds: 60));
    expect(api.sent, <String>['薄荷是'], reason: '停顿后只发最后一次');
    probe.dispose();
  });

  test('会话跟着探针走：请求带上当前会话 id', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: Duration.zero,
      sessionId: 's1',
    );
    probe.onChanged('薄荷');
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(api.sessions, <String?>['s1']);
    probe.dispose();
  });

  test('cancel 之后不再发：切会话不把旧草稿落到新会话上', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: const Duration(milliseconds: 30),
    );
    probe.onChanged('旧会话的草稿');
    // 这就是 _onChatSessionMaybeChanged 在会话切换时做的事：先取消、再换 id。
    probe.cancel();
    probe.sessionId = 'b';
    await Future<void>.delayed(const Duration(milliseconds: 80));
    expect(api.sent, isEmpty, reason: '取消后不得补发');
    probe.dispose();
  });

  test('reset 之后计时器停掉且不记 lastSent', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: const Duration(milliseconds: 30),
    );
    probe.onChanged('已经发出去的那一句');
    probe.reset();
    expect(probe.lastSent, isNull);
    await Future<void>.delayed(const Duration(milliseconds: 80));
    expect(api.sent, isEmpty);
    probe.dispose();
  });

  test('防抖常量与服务端是同一个数字', () {
    // 计划 §6 M3：客户端停顿 400 ms 再发，服务端不自己防抖。
    expect(kDraftDebounceMs, 400);
  });
}
