/// 草稿探针的**会话竞态**单测（2026-11-18 审计 #20）。
///
/// 现场：`POST /api/v1/chat/draft` 的语义是「按 body 里的 session_id 整块替换
/// 该会话的热卡」。旧探针把会话 id 留到计时器回调里现读字段：用户在 400 ms 停顿
/// 窗口里切了会话（监听顺序出一点岔子 / 调用方忘了 cancel），A 的草稿就会带着
/// B 的 id 发出去——服务端把 A 的检索结果写进 B 的热卡，B 下一次发送的 system
/// 文本因此带着上一个会话的检索（这就是「上一会话的检索写进新会话热卡」的
/// 前端那一半；服务端侧按 body 里的 id 写，见 chat_draft.rs，没有活动会话回退）。
///
/// 守两条防线：
/// 1. 会话在**排计时器时**固定，请求带的是那一刻的 id；
/// 2. 触发时目标会话已经变了就**不发**——cancel 漏了也不会跨会话。
///
/// 服务端默认桶（不带 id）与具名会话是两个桶：null 与 'a' 之间也必须算「变了」。
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

/// 比防抖窗口长一点的等待（真实计时器；与既有 draft_probe_test 同一条写法）。
Future<void> _afterDebounce() =>
    Future<void>.delayed(const Duration(milliseconds: 60));

void main() {
  test('排计时器后切走会话（没 cancel）→ 这一次不发，绝不带新 id 发旧正文', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: const Duration(milliseconds: 20),
      sessionId: 'a',
    );
    probe.onChanged('A 会话的草稿');
    // 模拟「切到 B，但 cancel 没走到」（监听顺序 / 调用方漏了）。
    probe.sessionId = 'b';

    await _afterDebounce();
    expect(api.sent, isEmpty, reason: '带 B 的 id 发 A 的正文 = 服务端把 A 的检索写进 B 的热卡');
    expect(probe.lastSent, isNull);
    probe.dispose();
  });

  test('请求带的是**排计时器时**的会话，不是触发时现读的字段', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: const Duration(milliseconds: 20),
      sessionId: 'a',
    );
    probe.onChanged('薄荷');
    await _afterDebounce();
    expect(api.sent, <String>['薄荷']);
    expect(api.sessions, <String?>['a']);
    probe.dispose();
  });

  test('停顿窗口内切走又切回同一会话：这一次照发（会话没变）', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: const Duration(milliseconds: 20),
      sessionId: 'a',
    );
    probe.onChanged('薄荷');
    probe.sessionId = 'b';
    probe.sessionId = 'a';
    await _afterDebounce();
    expect(api.sessions, <String?>['a']);
    probe.dispose();
  });

  test('全局桶与具名会话也算变了：null → a 不许发', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: const Duration(milliseconds: 20),
    );
    probe.onChanged('全局桶的草稿');
    probe.sessionId = 'a';
    await _afterDebounce();
    expect(api.sent, isEmpty);
    probe.dispose();
  });

  test('同一窗口里切走后继续输入：新的一次用新会话，旧那次已作废', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: const Duration(milliseconds: 20),
      sessionId: 'a',
    );
    probe.onChanged('甲');
    probe.sessionId = 'b';
    probe.onChanged('乙'); // 用户在 B 里继续打：这一次属于 B
    await _afterDebounce();
    expect(api.sent, <String>['乙'], reason: '同一窗口只有最后一次发');
    expect(api.sessions, <String?>['b']);
    probe.dispose();
  });

  test('cancel 之后旧草稿不发（既有防线仍在）', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: const Duration(milliseconds: 20),
      sessionId: 'a',
    );
    probe.onChanged('旧会话的草稿');
    probe.cancel();
    probe.sessionId = 'b';
    await _afterDebounce();
    expect(api.sent, isEmpty);
    probe.dispose();
  });

  test('reset 停掉计时器并清 lastSent（既有语义不变）', () async {
    final _RecordingApi api = _RecordingApi();
    final DraftProbe probe = DraftProbe(
      api: api,
      debounce: const Duration(milliseconds: 20),
      sessionId: 'a',
    );
    probe.onChanged('已经发出去的那一句');
    probe.reset();
    await _afterDebounce();
    expect(api.sent, isEmpty);
    expect(probe.lastSent, isNull);
    probe.dispose();
  });
}
