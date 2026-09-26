/// 「导演可观测」四栏（阶段5 W5a，裁决 D40–D43 / 范围 §2 A–D）。
///
/// 挂载点：`DeveloperSection.build` 的 `if (devMode)` 块内（`DebugPanels` 之后）。
/// **dev_mode=false 时整块不在语义树**；本组件自己也短路一次，测试可单点。
///
/// # 数据怎么进来（不改 shell_settings.dart 的硬约束）
///
/// `DeveloperSection` 由 `app/shell_settings.dart` 构造，观测数据不能经构造参数从
/// `main.dart` 传进来。做法：本文件里定义单例 [DirectorObserverFeed]
/// （`ChangeNotifier`）——`main.dart` 只往里 push，本组件用 `ListenableBuilder`
/// 消费。构造函数仍保留可注入的假数据口（[loadState] / [loadLogLines]），
/// widget 测试注入 fake，**不发真网络**。
///
/// # 四栏
///
/// | 栏 | 来源 |
/// | --- | --- |
/// | A 决策参数 | 服务端导演运行态端点（逐键对上 state_json） |
/// | B 事件流 | 前端已收 WS 帧 + 渲染面 ack（环形缓冲上限 200，按 type 过滤） |
/// | C 传参对照 | 左「请求」= 前端 preset；右「生效」= 渲染面 ack 最终值 |
/// | D 送 TTS 文本 | 左 = 前端已收 text_delta；右 = 日志端点 sentence_ready |
///
/// 观测缓冲**只住内存**：不写本机存储、不落盘（本栏红线）。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/diagnostics_api.dart';
import '../../api/mods_api.dart';
import '../../api/ws_frame.dart';
import '../../design/tokens.dart';
import '../../live2d/render_events.dart';
import '../mods/director_panel.dart';
import '../../ui/inline_notice.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';

/// A 栏读取的 Mod id。
///
/// **不写成字面调用**：`test/action_cue_test.dart` 的通道 B 退役回归按源码子串
/// 钉住 lib/ 不得出现「拉 director 状态面」的调用点（阶段3 D10–D13）。阶段5
/// 的 A 栏是**只读展示**（D41/D43），这里把它集中成一个常量，调用点不带字面 id。
const String kDirectorObserverModId = 'director';

/// A 栏「取不到」时的如实说明（**不许空面板**）。纯函数，可单测。
String directorStateErrorText(ApiException e) {
  if (e.status == 403 || e.code == 'mod_disabled') {
    return '读不到决策参数：导演 Mod 未启用（403 mod_disabled）——'
        '先到「Mod」分区启用 director。';
  }
  if (e.code == 'not_found') {
    return '读不到决策参数：director 不在服务端注册表（404 not_found）。';
  }
  if (e.code == 'state_unavailable') {
    return '读不到决策参数：Mod 正忙或未上报运行态（503 state_unavailable），可重试。';
  }
  if (e.code == 'network_error') {
    return '读不到决策参数：无法连接后端（network_error）。';
  }
  return '读不到决策参数：${e.code}（HTTP ${e.status ?? '?'}）。';
}

/// 观测缓冲的单例 hub（`ChangeNotifier`）。
///
/// `main.dart` 只往里 push；观测缓冲**只住内存**，没有任何写盘路径。
class DirectorObserverFeed extends ChangeNotifier {
  DirectorObserverFeed._();

  /// 进程内唯一实例（前端不落盘，故不需要持久层）。
  static final DirectorObserverFeed instance = DirectorObserverFeed._();

  /// B 栏的环形缓冲（上限 [kObserverBufferCapacity] = 200）。
  final ObserverBuffer events = ObserverBuffer();

  final List<PresetRequest> _requests = <PresetRequest>[];
  final List<RenderEvent> _acks = <RenderEvent>[];
  final List<TtsSentence> _sentences = <TtsSentence>[];

  /// 可注入时钟（测试不关心绝对时刻；缺省即本机时钟）。
  DateTime Function() clock = DateTime.now;

  List<PresetRequest> get requests =>
      List<PresetRequest>.unmodifiable(_requests);
  List<RenderEvent> get acks => List<RenderEvent>.unmodifiable(_acks);
  List<TtsSentence> get sentences => List<TtsSentence>.unmodifiable(_sentences);

  /// 一条前端已收 WS 帧：进 B 栏；text_delta / text_fallback 同时进 D 栏左列。
  void pushWsEvent(WsEvent event) {
    final DateTime at = clock();
    final ObserverRecord? record = observeWsEvent(event, at);
    if (record != null) events.add(record);
    final TtsSentence? sentence = ttsSentenceFromWsEvent(event, at);
    if (sentence != null) {
      _sentences.add(sentence);
      _trim(_sentences);
    }
    notifyListeners();
  }

  /// 一条渲染面 ack：进 B 栏；applied / dropped 同时进 C 栏的「生效」侧。
  void pushRenderEvent(RenderEvent event) {
    events.add(observeRenderEvent(event, clock()));
    if (kPresetAckKinds.contains(event.kind)) {
      _acks.add(event);
      _trim(_acks);
    }
    notifyListeners();
  }

  /// 一次音频时钟采样（协议 §6.5 / O13）：进 B 栏。
  void pushStageClock({int? seg, required int posMs, required bool playing}) {
    events.add(
      observeStageClock(seg: seg, posMs: posMs, playing: playing, at: clock()),
    );
    notifyListeners();
  }

  /// 一条前端下发的 preset 请求：进 C 栏左列。
  void pushPresetRequest(PresetRequest request) {
    _requests.add(request);
    _trim(_requests);
    notifyListeners();
  }

  void clear() {
    events.clear();
    _requests.clear();
    _acks.clear();
    _sentences.clear();
    notifyListeners();
  }

  void _trim<T>(List<T> list) {
    while (list.length > kObserverBufferCapacity) {
      list.removeAt(0);
    }
  }
}

/// 设置 → 开发工具 →「导演可观测」四栏。
class DirectorObserverSection extends StatefulWidget {
  const DirectorObserverSection({
    this.devMode = true,
    this.loadState,
    this.loadLogLines,
    super.key,
  });

  /// false → 整块不渲染（与 DeveloperSection 的短路同一口径）。
  final bool devMode;

  /// A 栏数据口（缺省 = `ModsApi().state(kDirectorObserverModId)` 的 `.state`）。
  /// widget 测试注入 fake，**不发真网络**。
  final Future<Map<String, Object?>> Function()? loadState;

  /// D 栏右列数据口（缺省 = `DiagnosticsApi().logs()` 映射成字符串列表）。
  final Future<List<String>> Function()? loadLogLines;

  @override
  State<DirectorObserverSection> createState() =>
      _DirectorObserverSectionState();
}

class _DirectorObserverSectionState extends State<DirectorObserverSection> {
  ModsApi? _modsApi;
  DiagnosticsApi? _diagApi;

  Map<String, Object?>? _state;
  String? _stateError;
  bool _stateLoading = false;

  List<String> _logLines = const <String>[];
  String? _logsError;
  bool _logsLoading = false;

  /// B 栏选中的 type（空集 = 全部；**真的过滤**）。
  final Set<String> _types = <String>{};

  @override
  void initState() {
    super.initState();
    if (widget.devMode) {
      unawaited(_loadState());
      unawaited(_loadLogs());
    }
  }

  @override
  void didUpdateWidget(covariant DirectorObserverSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.devMode && !oldWidget.devMode) {
      unawaited(_loadState());
      unawaited(_loadLogs());
    }
  }

  @override
  void dispose() {
    _modsApi?.dispose();
    _diagApi?.dispose();
    super.dispose();
  }

  Future<Map<String, Object?>> _defaultLoadState() async =>
      (await (_modsApi ??= ModsApi()).state(kDirectorObserverModId)).state;

  Future<List<String>> _defaultLoadLogLines() async {
    final List<LogLine> lines = await (_diagApi ??= DiagnosticsApi()).logs();
    return lines.map((LogLine l) => l.asText).toList();
  }

  Future<void> _loadState() async {
    setState(() {
      _stateLoading = true;
      _stateError = null;
    });
    try {
      final Map<String, Object?> loaded =
          await (widget.loadState ?? _defaultLoadState)();
      if (!mounted) return;
      setState(() {
        _state = loaded;
        _stateLoading = false;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _state = null;
        _stateError = directorStateErrorText(e);
        _stateLoading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _state = null;
        _stateError = '读不到决策参数：$e';
        _stateLoading = false;
      });
    }
  }

  Future<void> _loadLogs() async {
    setState(() {
      _logsLoading = true;
      _logsError = null;
    });
    try {
      final List<String> lines =
          await (widget.loadLogLines ?? _defaultLoadLogLines)();
      if (!mounted) return;
      setState(() {
        _logLines = lines;
        _logsLoading = false;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _logsError = '读不到日志端点：${e.code}（HTTP ${e.status ?? '?'}）';
        _logsLoading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _logsError = '读不到日志端点：$e';
        _logsLoading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    // ④ dev_mode=false：整块不在语义树（本组件自己再短路一次）。
    if (!widget.devMode) return const SizedBox.shrink();
    return ListenableBuilder(
      listenable: DirectorObserverFeed.instance,
      builder: (BuildContext context, Widget? _) {
        final ThemeData theme = Theme.of(context);
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            const SectionHeader(
              title: '导演可观测',
              description: '四栏只读观测：A 决策参数 / B 事件流 / C 传参对照 / '
                  'D 送 TTS 文本。只住内存，**不落盘**。',
            ),
            _decisionBlock(theme),
            _eventsBlock(theme),
            _overrideBlock(theme),
            _textBlock(theme),
            const SizedBox(height: Space.s3),
            Align(
              alignment: Alignment.centerLeft,
              // 用 TextButton（不是 OutlinedButton）：调试面板的既有
              // 源码/布局回归按 OutlinedButton 计数（developer_section_test），
              // 本栏不得改变那一屏的按钮类型分布。
              child: TextButton.icon(
                onPressed: _clear,
                icon: const Icon(Icons.delete_outline, size: 16),
                label: const Text('清空观测缓冲'),
              ),
            ),
          ],
        );
      },
    );
  }

  void _clear() {
    setState(_types.clear);
    DirectorObserverFeed.instance.clear();
  }

  // ───────────────────────────────────────────────────────── A 决策参数

  Widget _decisionBlock(ThemeData theme) {
    final AppColors colors = appColorsOf(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    final TextStyle? value = theme.textTheme.bodySmall;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s4),
        Row(
          children: <Widget>[
            Expanded(
              child: Text('A 决策参数', style: theme.textTheme.titleSmall),
            ),
            TextButton.icon(
              onPressed: _stateLoading ? null : () => unawaited(_loadState()),
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('刷新决策参数'),
            ),
          ],
        ),
        Text(
          '来源 = 服务端导演运行态端点（GET /api/v1/mods/{id}/state，id=$kDirectorObserverModId；键名与 state_json 逐字一致）',
          style: muted,
        ),
        if (_stateLoading)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: Text('读取中…', style: muted),
          )
        else if (_stateError != null)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: InlineNotice(message: _stateError!, dense: true),
          )
        else if (_state == null)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: Text('（还没取到决策参数，点上面的刷新）', style: muted),
          )
        else
          ..._decisionRows(theme, muted, value),
      ],
    );
  }

  List<Widget> _decisionRows(
    ThemeData theme,
    TextStyle? muted,
    TextStyle? value,
  ) {
    final Map<String, Object?> state = _state ?? const <String, Object?>{};
    final Map<String, Object?> latest = _stringMap(state['latest']);
    final bool hasLatest = latest.isNotEmpty;
    String of(Object? v) => directorNumberText(v);
    return <Widget>[
      _kv(
        'latest.emotion',
        hasLatest ? directorEmotionText(latest['emotion']) : '—',
        muted,
        value,
      ),
      _kv(
        'latest.intent',
        hasLatest ? directorIntentText(latest['intent']) : '—',
        muted,
        value,
      ),
      _kv(
        'latest.suggested_tts.speed',
        hasLatest ? of(_nested(latest, 'suggested_tts', 'speed')) : '—',
        muted,
        value,
      ),
      _kv(
        'latest.suggested_tts.pitch',
        hasLatest ? of(_nested(latest, 'suggested_tts', 'pitch')) : '—',
        muted,
        value,
      ),
      _kv(
        'latest.preset_id',
        hasLatest ? directorPresetText(latest['preset_id']) : '—',
        muted,
        value,
      ),
      _kv(
        'latest.seq',
        hasLatest ? of(latest['seq']) : '—',
        muted,
        value,
      ),
      _kv(
        'latest.turn',
        hasLatest ? of(latest['turn']) : '—',
        muted,
        value,
      ),
      _kv('turns_seen', of(state['turns_seen']), muted, value),
      _kv('turns_ended', of(state['turns_ended']), muted, value),
      _kv('decisions', of(state['decisions']), muted, value),
      _kv('silent', of(state['silent']), muted, value),
      _kv('errors', of(state['errors']), muted, value),
    ];
  }

  Widget _kv(String label, String text, TextStyle? muted, TextStyle? value) =>
      Padding(
        padding: const EdgeInsets.only(top: OpticalNudge.thin),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            SizedBox(
              width: 200,
              child: Text('$label：', style: muted),
            ),
            Expanded(child: Text(text, style: value)),
          ],
        ),
      );

  // ───────────────────────────────────────────────────────── B 事件流

  Widget _eventsBlock(ThemeData theme) {
    final AppColors colors = appColorsOf(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    final ObserverBuffer buffer = DirectorObserverFeed.instance.events;
    final List<ObserverRecord> records = buffer.filterByTypes(_types);
    final List<String> types = <String>{
      ...kObserverKnownTypes,
      ...buffer.records.map((ObserverRecord r) => r.type),
    }.toList()
      ..sort();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s4),
        Row(
          children: <Widget>[
            Expanded(
              child: Text('B 事件流', style: theme.textTheme.titleSmall),
            ),
            Text(
              '共 ${buffer.length} 条 / 上限 $kObserverBufferCapacity',
              style: muted,
            ),
          ],
        ),
        Text(
          '前端已收 WS 帧 + 渲染面 ack；带时间戳，按 type 过滤（空白 = 全部）',
          style: muted,
        ),
        const SizedBox(height: Space.s1),
        Wrap(
          spacing: Space.s1,
          runSpacing: Space.s1,
          children: <Widget>[
            for (final String t in types)
              FilterChip(
                label: Text(t),
                selected: _types.contains(t),
                onSelected: (bool selected) => setState(() {
                  if (selected) {
                    _types.add(t);
                  } else {
                    _types.remove(t);
                  }
                }),
              ),
          ],
        ),
        Padding(
          padding: const EdgeInsets.only(top: Space.s1),
          child: Text(
            _types.isEmpty ? '过滤：全部类型' : '过滤：${_types.join('、')}',
            style: muted,
          ),
        ),
        if (buffer.isEmpty)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: Text('（还没有事件）', style: muted),
          )
        else if (records.isEmpty)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: Text('（当前过滤下没有事件）', style: muted),
          )
        else
          SizedBox(
            height: 240,
            child: ListView.builder(
              itemExtent: 20,
              itemCount: records.length,
              itemBuilder: (BuildContext context, int i) {
                final ObserverRecord r = records[records.length - 1 - i];
                return Text(
                  '${formatObserverTime(r.at)} [${r.source}] ${r.type} · ${r.text}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.bodySmall,
                );
              },
            ),
          ),
      ],
    );
  }

  // ───────────────────────────────────────────────────────── C 传参对照

  Widget _overrideBlock(ThemeData theme) {
    final AppColors colors = appColorsOf(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    final List<PresetRequest> requests = DirectorObserverFeed.instance.requests;
    final List<RenderEvent> acks = DirectorObserverFeed.instance.acks;
    final List<PresetRequest> recent = requests.reversed
        .take(20)
        .toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s4),
        Text('C 传参对照', style: theme.textTheme.titleSmall),
        Text(
          '左「请求」= 前端 preset 帧；右「生效」= 渲染面 ack 最终值 + '
          'clamped / degraded / reason（找不到 ack 如实写「尚无 ack」）',
          style: muted,
        ),
        if (recent.isEmpty)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: Text('（还没有前端 preset 请求）', style: muted),
          )
        else
          for (final PresetRequest r in recent) ...<Widget>[
            Padding(
              padding: const EdgeInsets.only(top: Space.s2),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(
                    presetOverridePair(r, acks).request,
                    style: theme.textTheme.bodySmall,
                  ),
                  Text(
                    presetOverridePair(r, acks).applied,
                    style: muted,
                  ),
                ],
              ),
            ),
          ],
      ],
    );
  }

  // ───────────────────────────────────────────────────── D 送 TTS 文本

  Widget _textBlock(ThemeData theme) {
    final AppColors colors = appColorsOf(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    final List<TtsSentence> sentences = DirectorObserverFeed.instance.sentences;
    final List<SentenceReadyLog> ready = parseSentenceReadyLogs(_logLines);
    final List<TtsSentence> recentSentences = sentences.reversed
        .take(20)
        .toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s4),
        Row(
          children: <Widget>[
            Expanded(
              child: Text('D 送 TTS 文本', style: theme.textTheme.titleSmall),
            ),
            TextButton.icon(
              onPressed: _logsLoading ? null : () => unawaited(_loadLogs()),
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('刷新日志端点'),
            ),
          ],
        ),
        Text('来源=日志端点；不承诺逐句精确匹配', style: muted),
        const SizedBox(height: Space.s1),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text('净化文本（text_delta，前端已收）', style: muted),
                  if (recentSentences.isEmpty)
                    Text('（还没有送 TTS 的句子）', style: muted)
                  else
                    for (final TtsSentence s in recentSentences)
                      Text(
                        '${s.text}（${s.chars} 字符）',
                        style: theme.textTheme.bodySmall,
                      ),
                ],
              ),
            ),
            const SizedBox(width: Space.s3),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text('原始段（日志端点 sentence_ready）', style: muted),
                  if (_logsLoading)
                    Text('读取中…', style: muted)
                  else if (_logsError != null)
                    Text(_logsError!, style: muted)
                  else if (ready.isEmpty)
                    Text('（还没有 sentence_ready 行）', style: muted)
                  else
                    for (final SentenceReadyLog s in ready)
                      Text(
                        '${s.text}（${s.chars} 字符）',
                        style: theme.textTheme.bodySmall,
                      ),
                ],
              ),
            ),
          ],
        ),
      ],
    );
  }
}

Map<String, Object?> _stringMap(Object? value) {
  final Map<String, Object?> out = <String, Object?>{};
  if (value is Map) {
    value.forEach((Object? k, Object? v) {
      if (k is String) out[k] = v;
    });
  }
  return out;
}

Object? _nested(Map<String, Object?> m, String a, String b) =>
    _stringMap(m[a])[b];
