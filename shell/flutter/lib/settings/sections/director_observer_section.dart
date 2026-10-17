/// 「导演可观测」四栏（阶段5 W5a，裁决 D40–D43 / 范围 §2 A–D）。
///
/// 挂载点：一级「导演」卡的 `devMode` 块内（`settings/sections/director_section.dart`）。
/// **dev_mode=false 时整块不在语义树**；本组件自己也短路一次，测试可单点。
///
/// # rc4（2026-10-17）：A 栏降级为静态，不再读服务端端点
///
/// 导演已并回核心常驻——**没有**决策账本可读（`GET /api/v1/mods/director/state`
/// 的读取路径从本文件删除，403/404 文案一并删除）。A 栏只陈述一个事实：
/// 「核心常驻（无决策账本）」。B/C/D 三栏与 Feed 一字未改。
///
/// # 数据怎么进来
///
/// 本文件里定义单例 [DirectorObserverFeed]（`ChangeNotifier`）——`main.dart`
/// 只往里 push，本组件用 `ListenableBuilder` 消费。D 栏的日志端点仍保留可注入的
/// 假数据口（[loadLogLines]），widget 测试注入 fake，**不发真网络**。
///
/// # 四栏
///
/// | 栏 | 来源 |
/// | --- | --- |
/// | A 决策参数 | **静态**：核心常驻（无决策账本）——不发网络请求 |
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
import '../../api/ws_frame.dart';
import '../../design/tokens.dart';
import '../../live2d/render_events.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';

/// A 栏的**静态**结论（rc4：导演常驻核心，不再有决策账本可读）。纯文本常量，
/// 单测直接断言它上屏。
const String kDirectorCoreResidentText = '核心常驻（无决策账本）';

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

  /// 预设 id → 展示名（T3，2026-10-13 执行单）：`action_cue` 那一行把
  /// `preset_id` 显示成 `preset_labels.json` 的 `zh`，旁边保留 id。
  ///
  /// 由宿主注入（`shell_admin._loadPresetLabels` 拿到表之后设成
  /// `table.display`）；没注入 / 该 id 没中文名 = 只显示 id——**不在这里
  /// 硬编码第二套译名**。
  String Function(String id)? presetLabel;

  List<PresetRequest> get requests =>
      List<PresetRequest>.unmodifiable(_requests);
  List<RenderEvent> get acks => List<RenderEvent>.unmodifiable(_acks);
  List<TtsSentence> get sentences => List<TtsSentence>.unmodifiable(_sentences);

  /// 一条前端已收 WS 帧：进 B 栏；text_delta / text_fallback 同时进 D 栏左列。
  void pushWsEvent(WsEvent event) {
    final DateTime at = clock();
    final ObserverRecord? record = observeWsEvent(event, at, labelFor: presetLabel);
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
    this.loadLogLines,
    super.key,
  });

  /// false → 整块不渲染（与 DirectorSection 的短路同一口径）。
  final bool devMode;

  /// D 栏右列数据口（缺省 = `DiagnosticsApi().logs()` 映射成字符串列表）。
  final Future<List<String>> Function()? loadLogLines;

  @override
  State<DirectorObserverSection> createState() =>
      _DirectorObserverSectionState();
}

class _DirectorObserverSectionState extends State<DirectorObserverSection> {
  DiagnosticsApi? _diagApi;

  List<String> _logLines = const <String>[];
  String? _logsError;
  bool _logsLoading = false;

  /// B 栏**不列出**的 type（默认 = 口型时钟 + 段结束；**真的过滤**）。
  ///
  /// T3（2026-10-13 执行单）：语义从「只看勾中的」改成「默认都看，除了这两个」。
  final Set<String> _excluded = <String>{...kObserverDefaultDeselectedTypes};

  /// 被点开的那一条（在列表**下方**展开全文；再点一次收起）。
  ObserverRecord? _expanded;

  @override
  void initState() {
    super.initState();
    if (widget.devMode) {
      unawaited(_loadLogs());
    }
  }

  @override
  void didUpdateWidget(covariant DirectorObserverSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.devMode && !oldWidget.devMode) {
      unawaited(_loadLogs());
    }
  }

  @override
  void dispose() {
    _diagApi?.dispose();
    super.dispose();
  }

  Future<List<String>> _defaultLoadLogLines() async {
    final List<LogLine> lines = await (_diagApi ??= DiagnosticsApi()).logs();
    return lines.map((LogLine l) => l.asText).toList();
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
            const SectionHeader(title: '导演可观测'),
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
    setState(() {
      _excluded
        ..clear()
        ..addAll(kObserverDefaultDeselectedTypes);
      _expanded = null;
    });
    DirectorObserverFeed.instance.clear();
  }

  // ───────────────────────────────────────────────────── A 决策参数（静态）
  //
  // rc4：导演并回核心常驻，不再有「决策账本」可读。A 栏**不发任何网络请求**，
  // 只把这一事实摆在开发者面上——保留栏位是为了让四栏结构与 B/C/D 的对照还在。
  Widget _decisionBlock(ThemeData theme) {
    final AppColors colors = appColorsOf(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s4),
        Text('A 决策参数', style: theme.textTheme.titleSmall),
        const SizedBox(height: Space.s1),
        Text(kDirectorCoreResidentText, style: theme.textTheme.bodySmall),
        Text(
          '导演已并回核心：按每一句原文给动作，不再有可读的决策计数或最近一次决策。',
          style: muted,
        ),
      ],
    );
  }

  // ───────────────────────────────────────────────────────── B 事件流

  Widget _eventsBlock(ThemeData theme) {
    final AppColors colors = appColorsOf(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    final ObserverBuffer buffer = DirectorObserverFeed.instance.events;
    // T3（2026-10-13 执行单）：默认**不列出**口型时钟与段结束（见
    // kObserverDefaultDeselectedTypes）；语义是「不看的」而不是「只看这些」。
    final List<ObserverRecord> records = filterObserverRecords(
      buffer.records,
      _excluded,
    );
    // 芯片清单 = 已知类型 ∪ 缓冲里真的出现过的类型（未知类型照记，用线名）。
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
        // 标题下的这句导览（T3）：这一栏是「本轮交给舞台和页面的帧」，
        // 不是第二路模型的决策原文。
        Text(
          '这里是本轮交给舞台和页面的帧。动作看「动作」；口型时钟默认不列出。',
          style: muted,
        ),
        const SizedBox(height: Space.s1),
        Wrap(
          spacing: Space.s1,
          runSpacing: Space.s1,
          children: <Widget>[
            for (final String t in types)
              FilterChip(
                // 芯片 = 中文 + 括号里的线名（未知类型只显示线名）。
                label: Text(observerTypeLabel(t)),
                selected: !_excluded.contains(t),
                onSelected: (bool selected) => setState(() {
                  if (selected) {
                    _excluded.remove(t);
                  } else {
                    _excluded.add(t);
                    // 刚被藏起来的那条不再展开（否则下方挂着一行看不见来源的全文）。
                    if (_expanded?.type == t) _expanded = null;
                  }
                }),
              ),
          ],
        ),
        Padding(
          padding: const EdgeInsets.only(top: Space.s1),
          child: Text(_filterSummary(types), style: muted),
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
              // **没有 itemExtent**（T3）：行高不定，长文本换行而不是省略号。
              itemCount: records.length,
              itemBuilder: (BuildContext context, int i) {
                final ObserverRecord r = records[records.length - 1 - i];
                return InkWell(
                  onTap: () => setState(
                    () => _expanded = identical(_expanded, r) ? null : r,
                  ),
                  child: Padding(
                    padding: const EdgeInsets.only(top: OpticalNudge.thin),
                    child: Text(
                      '${formatObserverTime(r.at)} [${r.source}] ${r.type} · ${r.text}',
                      style: theme.textTheme.bodySmall,
                    ),
                  ),
                );
              },
            ),
          ),
        if (_expanded != null) _expandedBlock(theme, colors, muted),
      ],
    );
  }

  /// 过滤摘要（T3）：「全部类型」这个说法已退役——默认档写「当前：默认」，
  /// 其余档**列出正在看的中文名**（用户不必回头数芯片）。
  String _filterSummary(List<String> types) {
    // 默认档 = 恰好「只差」口型时钟与段结束（不用 setEquals：material 不导出它）。
    final bool isDefault =
        _excluded.length == kObserverDefaultDeselectedTypes.length &&
        _excluded.containsAll(kObserverDefaultDeselectedTypes);
    if (isDefault) {
      return '过滤：当前：默认（口型时钟与段结束不列出）';
    }
    final List<String> shown = <String>[
      for (final String t in types)
        if (!_excluded.contains(t)) observerTypeName(t),
    ];
    if (shown.isEmpty) return '过滤：当前：一个都没勾';
    return '过滤：当前：${shown.join('、')}';
  }

  /// 点开的记录：在**列表下方**展开全文（再点一次收起）。
  ///
  /// 不新开路由、不做左右滚动（T3 执行单的原话）——所以只是列表下面一块
  /// 有描边的文本。
  Widget _expandedBlock(
    ThemeData theme,
    AppColors colors,
    TextStyle? muted,
  ) {
    final ObserverRecord r = _expanded!;
    return Padding(
      padding: const EdgeInsets.only(top: Space.s1),
      child: Container(
        width: double.infinity,
        padding: const EdgeInsets.all(Space.s2),
        decoration: BoxDecoration(
          border: Border.all(color: colors.hairline),
          borderRadius: BorderRadius.circular(AppRadius.xs),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              '${formatObserverTime(r.at)} [${r.source}] ${observerTypeLabel(r.type)}',
              style: muted,
            ),
            Text(r.text, style: theme.textTheme.bodySmall),
          ],
        ),
      ),
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
