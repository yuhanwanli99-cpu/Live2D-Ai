
/// persona 的产品面板（产品级加强波次 + L1 会话绑定）。
///
/// 职责：把「导入一张角色卡 → 启用 → 对话人设变化 → 停用还原」这条链**尽量在 UI 内**
/// 走完，并把两件容易踩的事说清楚：坏卡失败怎么处置、与「记忆」Mod 的
/// last-writer-wins 关系。本文件由 persona 轨道独占，其他轨道不要改。
///
/// # 面板没有「保存配置」的通道，所以走一次性命令
///
/// ModPanelContext 只给了 onCommand（POST /api/v1/mods/persona/command），
/// 没有 POST /mods/persona/config。所以导入**不是**往配置表单里塞文本，而是：
/// import_card 命令在 Mod 侧解析 → 规范化落盘 → **立刻**生效
///（见 crates/live2d-ai-mod-persona/src/command.rs 头注）。
/// 面板只做两件事：把用户粘贴的文本 / 选到的文件送出去，**如实**报结果。
///
/// # 两个作用域，两个按钮（L1 会话绑定，2026-09-15）
///
/// - 「导入并生效」默认绑定 **ctx.activeSessionId**（当前会话）：只影响这个会话，
///   换会话不串卡、回原会话仍在。没有活动会话时它**禁用**并说清降级语义——
///   **绝不**偷偷改成全局导入（那正是这一波要根除的串人设）。
/// - 「导入为全局人设（所有会话）」是**旧的全局行为**（写主链 persona.system_prompt），
///   文案里写明它是全局的，不让人误以为绑定了会话。
/// - 运行态里的 sessions / active_session 由 Mod 的 state_json 给出：面板显示
///   「已绑定 N 个会话」，并标出服务端记录的活动会话。
///
/// # 粘贴是主路径，文件选择要宿主接线
///
/// 粘贴框是**始终可用**的那条路（卡本来就是 JSON）；「选择 PNG 角色卡文件」
/// 按钮只在宿主注入了 PersonaCardFilePicker 时出现——面板**不**直接 import
/// app/browser_io.dart，理由见那个 typedef 的注释（那会把 package:web
/// 拖进 flutter test 的编译面，连带弄红别人轨道的测试）。没接线时面板
/// 就只给粘贴框 + 一句说明，不摆一个按不动的按钮。
///
/// # 顺序：先打开开关、再导入
///
/// 命令通道与运行态共用 runtime 锁：Mod 未启用时 host 回 503
/// command_unavailable。所以停用时导入按钮是**禁用**的（而不是让用户点一下
/// 才吃一个错误），并就地说明原因；导入成功后**不必**再启停一次——人设已经生效。
///
/// # 三条提示都走文字
///
/// 1. 状态 failed / error → 「角色卡没被接受，修好再打开开关」+ 日志 mod: persona；
/// 2. 导入失败 → 带**错误码**（command_failed / unsupported_command / command_unavailable…）；
/// 3. 与 memory 共存 → last-writer-wins（后写覆盖，不做仲裁）；会话绑定的卡
///    不进主链，因此不受这条影响。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../ui/inline_notice.dart';
import '../../ui/section_header.dart';
import '../../ui/theme.dart';
import 'mod_panel.dart';

/// 选卡文件的读取器：**由宿主注入**（缺省 null = 这个构建没接文件入口）。
///
/// 为什么不在这里直接 import '../../app/browser_io.dart'：那个文件用的是
/// package:web，只在 Web 目标上能编译；而 settings/mods/*_panel.dart 会被
/// sections/dev_tools_section.dart 拉进 flutter test（Dart VM）的**编译面**——
/// 一 import 就让所有碰 Mod 管理区的测试在编译期变红（本轮实测）。
/// 所以文件选择只能从组合根注入，与 app/shell_prefs.dart 拿 pickImageDataUrl
/// 是同一条纪律；接线之前，**粘贴 JSON 是本面板的主路径**。
typedef PersonaCardFilePicker =
    Future<({String? dataUrl, String? error})> Function();

/// 一次导入里 dataURL 的字符上限。
///
/// **比 Mod 侧的字节上限更宽**（16 MiB 原始字节 ≈ 22.4 M base64 字符）：客户端
/// 只拦「明显是选错了文件」的那一档，真正的判定与原因仍在 Mod 侧（它会回一条
/// 带上限说明的 command_failed）。不设会更糟——一个几百 MB 的文件会被
/// 整个读成字符串再发出去。
const int kPersonaCardDataUrlMaxChars = 24000000;

/// 卡来源的稳定取值 → 中文（Mod 的 state_json.card_source）。**纯函数**。
String personaCardSourceLabel(String source) => switch (source) {
  'imported' => '界面导入',
  'config_json' => '配置 card_json',
  'config_path' => '配置 card_path',
  'overrides' => '仅手工覆盖项',
  _ => '未配置',
};

/// 卡格式的稳定取值 → 中文（Mod 的 state_json.card_format）。**纯函数**。
String personaCardFormatLabel(String format) => switch (format) {
  'v1' => 'V1 卡',
  'v2' => 'V2 卡',
  'manual' => '手工覆盖',
  _ => '未知',
};

/// 当前人设作用域的稳定取值 → 中文（Mod 的 state_json.scope）。**纯函数**。
String personaScopeLabel(String scope) => switch (scope) {
  'session' => '会话绑定（只对当前会话生效）',
  'global' => '全局（所有会话）',
  _ => '未知',
};

/// 运行状态是不是「启动失败」。
///
/// **两个值都要认**：GET /api/v1/mods 序列化的是 ModStatus::as_str() 的
/// failed，而 ModInfo.statusLabel 认识的历史写法是 error。
/// 只认一个，失败提示就永远不会出现——「界面看起来没事」的那类缺陷。
bool personaStatusIsFailed(String status) =>
    status == 'failed' || status == 'error';

/// persona 的产品面板（注册在 settings/mods/mod_panels.dart）。
class PersonaPanel extends ModPanel {
  const PersonaPanel({this.pickCardFile});

  /// 测试注入用；null = 真浏览器路径（browser_io.dart 的文件选择）。
  ///
  /// 公开而不是私有：命名参数不能以下划线开头（Dart 语言限制），
  /// 而这一个必须能在测试里被替换。
  final PersonaCardFilePicker? pickCardFile;

  @override
  String get modId => 'persona';

  @override
  Map<String, String> get stateLabels => const <String, String>{
    'ready': '人设已生效',
    'card_source': '卡来源',
    'card_name': '角色卡名称',
    'card_format': '卡格式',
    'applied_chars': '人设字数',
    'has_base_snapshot': '基线已记录',
    'include_discipline': '附加对话纪律',
    'say_first_mes': '朗读开场白',
    'config_has_card': '配置里有卡',
    'session_bound': '会话绑定可用',
    'sessions': '已绑定会话',
    'active_session': '当前活动会话',
    'scope': '当前作用域',
  };

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) =>
      PersonaCardPanel(ctx: ctx, pickCardFile: pickCardFile);
}

/// 面板本体：会话绑定说明 + 状态摘要 + 两个作用域的导入 + 三条提示。
///
/// 只有**局部**状态（粘贴框、进行中、最近一条结果）——它不改配置、不发列表请求，
/// 结果的真相在 Mod 侧（命令返回值 + state_json）。
class PersonaCardPanel extends StatefulWidget {
  const PersonaCardPanel({required this.ctx, this.pickCardFile, super.key});

  final ModPanelContext ctx;
  final PersonaCardFilePicker? pickCardFile;

  @override
  State<PersonaCardPanel> createState() => _PersonaCardPanelState();
}

class _PersonaCardPanelState extends State<PersonaCardPanel> {
  final TextEditingController _json = TextEditingController();

  /// 有一条命令正在飞（按钮全禁用，防重复提交）。
  bool _busy = false;

  /// 最近一条**看得见**的结果（成功 / 失败都留在这儿，不弹 toast）。
  String? _message;
  bool _isError = false;

  @override
  void dispose() {
    _json.dispose();
    super.dispose();
  }

  bool get _enabled => widget.ctx.enabled;

  /// 跑一条命令，并把结果如实转成一句话（失败带**错误码**）。
  ///
  /// 成功后通知宿主（统一的重启提示）并刷一次运行态：会话数 / 作用域会变，
  /// 不刷的话界面还停在旧数字上。
  Future<void> _run(
    String command, [
    Map<String, Object?> args = const <String, Object?>{},
  ]) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _message = null;
    });
    try {
      final ModCommandResult result = await widget.ctx.onCommand(command, args);
      if (!mounted) return;
      setState(() {
        _busy = false;
        _isError = !result.ok;
        _message = result.ok
            ? _successMessage(command, result)
            : '命令 $command 返回了 ok=false（服务端没有说明原因）';
      });
      if (result.ok) {
        widget.ctx.notifyChanged(_changedMessage(command, result));
        unawaited(widget.ctx.onRefreshState());
      }
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _isError = true;
        _message = _errorMessage(command, e);
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _isError = true;
        _message = '命令 $command 失败：$e';
      });
    }
  }

  /// 会话作用域动作：把 session_id 拼进入参；没有活动会话就地拒绝。
  ///
  /// 守卫与禁用的按钮**都**在：按钮禁用是体验，这里的守卫是契约——即便将来
  /// 有人接线错了，也**不会**静默退化成全局导入。
  Future<void> _runForSession(
    String command,
    Map<String, Object?> args,
  ) async {
    final String? session = widget.ctx.activeSessionId;
    if (session == null || session.isEmpty) {
      setState(() {
        _isError = true;
        _message = '还没有会话：先发一条消息（会自动建会话），'
            '或用「导入为全局人设（所有会话）」；为避免串人设，这里不会改走全局。';
      });
      return;
    }
    await _run(command, <String, Object?>{...args, 'session_id': session});
  }

  /// 成功结果 → 一句话（**作用域不同，文案不同**，不许混）。
  String _successMessage(String command, ModCommandResult result) {
    final String scope = _str(result.result['scope']);
    final String session = _str(result.result['session_id']);
    if (command == 'clear_import') {
      final bool isSession = scope == 'session';
      if (result.result['cleared'] != true) {
        return isSession
            ? '会话 $session 本来就没有绑定角色卡，不用清除'
            : '本来就没有导入的角色卡，不用清除';
      }
      if (isSession) {
        return '已清除会话 $session 的角色卡（其他会话不受影响）';
      }
      final String source = personaCardSourceLabel(
        _str(result.result['card_source']),
      );
      return '已清除全局导入卡；主链改由「$source」决定';
    }
    final String name = _str(result.result['card_name']);
    final String format = personaCardFormatLabel(
      _str(result.result['card_format']),
    );
    final String target = scope == 'session'
        ? '并绑定到会话 $session'
        : '并写回全局主链（所有会话生效）';
    if (name.isEmpty) return '已导入这张卡$target';
    return '已导入「$name」（$format）$target';
  }

  /// 成功结果 → 给宿主的变更短语（宿主据此弹统一提示）。
  String _changedMessage(String command, ModCommandResult result) {
    final String scope = _str(result.result['scope']);
    final String session = _str(result.result['session_id']);
    if (command == 'clear_import') {
      return scope == 'session' ? '已清除会话 $session 的角色卡' : '已清除全局导入卡';
    }
    return scope == 'session'
        ? '已导入角色卡并绑定到会话 $session'
        : '已导入角色卡为全局人设（所有会话生效）';
  }

  /// 失败 → 一句话 + **错误码**（用户要拿界面上的码去日志里搜）。
  String _errorMessage(String command, ApiException e) {
    final String action = command == 'clear_import' ? '清除角色卡' : '导入角色卡';
    switch (e.code) {
      case 'command_unavailable':
        return '$action需要 Mod 正在运行（503 command_unavailable）：先打开上面的开关再试';
      case 'unsupported_command':
        return '这个版本的角色卡 Mod 不认识这条命令（409 unsupported_command）';
      case 'not_found':
        return '服务端注册表里没有这个 Mod（404 not_found）';
      case 'command_failed':
        return '$action失败：${e.message}（409 command_failed）';
      default:
        return '$action失败：${e.message}（${e.code}）';
    }
  }

  /// 粘贴框 → import_card(card_json)，**绑定当前会话**（主路径）。
  Future<void> _importPasted() async {
    final String text = _json.text.trim();
    if (text.isEmpty) {
      setState(() {
        _isError = true;
        _message = '先粘贴角色卡 JSON 再点导入';
      });
      return;
    }
    await _runForSession('import_card', <String, Object?>{'card_json': text});
  }

  /// 粘贴框 → import_card(card_json)，**不给 session_id** = 旧的全局行为。
  Future<void> _importPastedGlobally() async {
    final String text = _json.text.trim();
    if (text.isEmpty) {
      setState(() {
        _isError = true;
        _message = '先粘贴角色卡 JSON 再点导入';
      });
      return;
    }
    await _run('import_card', <String, Object?>{'card_json': text});
  }

  /// 清除**当前会话**绑定的角色卡（其他会话不受影响）。
  Future<void> _clearSession() =>
      _runForSession('clear_import', const <String, Object?>{});

  /// 选一张 PNG 卡 → import_card(data_base64)，同样绑定当前会话（前缀由 Mod 剥）。
  Future<void> _pickAndImport() async {
    final PersonaCardFilePicker? pick = widget.pickCardFile;
    if (pick == null) {
      setState(() {
        _isError = true;
        _message = '这个构建没有接文件选择入口：请把卡 JSON 粘贴到上面的框里';
      });
      return;
    }
    final ({String? dataUrl, String? error}) picked;
    try {
      picked = await pick();
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _isError = true;
        _message = '选择文件失败：$e';
      });
      return;
    }
    if (!mounted) return;
    final String? error = picked.error;
    if (error != null) {
      setState(() {
        _isError = true;
        _message = error;
      });
      return;
    }
    final String? dataUrl = picked.dataUrl;
    if (dataUrl == null) return; // 取消：不弹东西（把取消说成失败会让人以为坏了）
    if (dataUrl.length > kPersonaCardDataUrlMaxChars) {
      setState(() {
        _isError = true;
        _message =
            '文件太大（编码后 ${dataUrl.length ~/ 1024} KB，上限 '
            '${kPersonaCardDataUrlMaxChars ~/ 1024} KB）：角色卡 PNG 通常远小于此，'
            '请改用配置里的 card_path 指向它';
      });
      return;
    }
    await _runForSession('import_card', <String, Object?>{
      'data_base64': dataUrl,
    });
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    final ModPanelContext ctx = widget.ctx;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s3),
        const SectionHeader(
          title: '角色卡（人设）',
          description: '导入一张卡，之后打开开关即生效，关闭开关会还原成主链基线的 '
              'system_prompt。默认绑定当前会话；也可以选择写入全局人设（所有会话）。',
        ),
        if (personaStatusIsFailed(ctx.mod.status)) _failedNotice(),
        _sessionBlock(ctx),
        if (_currentCardLine(ctx) case final String line)
          Padding(
            padding: const EdgeInsets.only(top: Space.s2),
            child: Text(line, style: muted),
          ),
        const SizedBox(height: Space.s2),
        _importControls(),
        if (_message != null)
          Padding(
            padding: const EdgeInsets.only(top: Space.s2),
            child: InlineNotice(
              message: _message!,
              severity: _isError
                  ? NoticeSeverity.danger
                  : NoticeSeverity.info,
              dense: true,
            ),
          ),
        _memoryNotice(),
      ],
    );
  }

  /// 失败态提示：**可处置**的一句话 + 去哪儿看原因。
  Widget _failedNotice() => const Padding(
    padding: EdgeInsets.only(top: Space.s2),
    child: InlineNotice(
      severity: NoticeSeverity.warning,
      message: '角色卡没被接受，修好再打开开关。（服务端状态：启动失败）'
          '原因看日志里 mod: persona 的行：诊断 → 日志（需要先打开开发者模式）。',
    ),
  );

  /// 顶部「会话绑定」块：当前会话 / 降级语义 / 运行态的会话数与活动会话。
  ///
  /// 降级语义必须**写死在界面上**：没有 activeSessionId 时「导入并生效」是禁用的，
  /// 用户唯一能走的全局路是那个明说「所有会话」的按钮。
  Widget _sessionBlock(ModPanelContext ctx) {
    final List<String> lines = _sessionLines(ctx);
    final bool degraded = ctx.activeSessionId == null;
    return Padding(
      padding: const EdgeInsets.only(top: Space.s2),
      child: InlineNotice(
        severity: degraded ? NoticeSeverity.warning : NoticeSeverity.info,
        dense: true,
        message: lines.join('\n'),
      ),
    );
  }

  List<String> _sessionLines(ModPanelContext ctx) {
    final String? active = ctx.activeSessionId;
    final Map<String, Object?>? state = ctx.state;
    final List<String> sessions = _stringList(state?['sessions']);
    final bool? bound = state?['session_bound'] is bool
        ? state!['session_bound'] as bool
        : null;
    final String runActive = _str(state?['active_session']);
    final String scope = _str(state?['scope']);
    return <String>[
      if (active != null)
        '会话绑定：当前会话 $active——导入的卡只对它生效，'
            '切换到别的会话不会串人设；回到这个会话时人设仍在。'
      else
        '还没有会话：先发一条消息（会自动建会话），'
            '或用「导入为全局人设」对所有会话生效。',
      if (sessions.isNotEmpty)
        '已绑定 ${sessions.length} 个会话：${sessions.join('、')}。'
      else if (state != null && bound == true)
        '运行态：还没有任何会话绑定角色卡。',
      if (bound == false)
        '本进程没有会话绑定能力（host 未注入会话表）：'
            '「导入并生效」不可用，请用「导入为全局人设」。',
      if (runActive.isNotEmpty && runActive != active)
        '服务端记录的活动会话是 $runActive（与本页当前会话可能不同步）。',
      if (scope.isNotEmpty) '当前人设作用域：${personaScopeLabel(scope)}。',
    ];
  }

  /// 「当前卡来源 / 名称」——运行态里那段（state_json）。
  ///
  /// 没有可说的 → null（**不占位、不糊一句废话**）。未启用时也返回 null：
  /// 「读不到」的原因下面那条说明已经讲了，而宿主给的那句 stateError 是给
  /// 「在跑但读不到」准备的（它提到 worker 正忙），搬到这里读起来像坏了。
  String? _currentCardLine(ModPanelContext ctx) {
    if (ctx.stateLoading) return '运行态读取中…';
    final Map<String, Object?>? state = ctx.state;
    if (state == null) {
      if (!ctx.enabled) return null;
      final String? error = ctx.stateError;
      return error == null ? '运行态还读不到，稍后刷新重试' : '运行态暂时读不到：$error';
    }
    if (state['ready'] != true) {
      final List<String> sessions = _stringList(state['sessions']);
      if (sessions.isNotEmpty) {
        return '当前全局人设保持主链基线；会话绑定见上方（${sessions.length} 个会话）。';
      }
      return '当前没有生效的角色卡，人设保持主链基线';
    }
    final String name = _str(state['card_name']);
    final String format = personaCardFormatLabel(_str(state['card_format']));
    final String source = personaCardSourceLabel(_str(state['card_source']));
    return '当前全局角色卡：${name.isEmpty ? '（未命名）' : name} · $format · 来源：$source';
  }

  /// 导入区：粘贴框（主路径）+ 两个作用域的导入 + 两个作用域的清除。
  Widget _importControls() {
    final ModPanelContext ctx = widget.ctx;
    final bool base = _enabled && !_busy;
    final bool sessionReady = base && ctx.activeSessionId != null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _json,
          enabled: base,
          minLines: 3,
          maxLines: 8,
          decoration: const InputDecoration(
            isDense: true,
            border: OutlineInputBorder(),
            labelText: '粘贴角色卡 JSON（主路径）',
            hintText: '把 .json 卡整段粘到这里',
          ),
        ),
        const SizedBox(height: Space.s2),
        Wrap(
          spacing: Space.s2,
          runSpacing: Space.s1,
          children: <Widget>[
            FilledButton.tonal(
              onPressed: sessionReady ? () => unawaited(_importPasted()) : null,
              child: Text(_busy ? '处理中…' : '导入并生效'),
            ),
            OutlinedButton(
              onPressed: base
                  ? () => unawaited(_importPastedGlobally())
                  : null,
              child: const Text('导入为全局人设（所有会话）'),
            ),
            if (widget.pickCardFile != null)
              OutlinedButton(
                onPressed: sessionReady ? () => unawaited(_pickAndImport()) : null,
                child: const Text('选择 PNG 角色卡文件'),
              ),
            TextButton(
              onPressed: sessionReady ? () => unawaited(_clearSession()) : null,
              child: const Text('清除当前会话的卡'),
            ),
            TextButton(
              onPressed: base ? () => unawaited(_run('clear_import')) : null,
              child: const Text('清除全局导入卡'),
            ),
          ],
        ),
        const Padding(
          padding: EdgeInsets.only(top: Space.s1),
          child: InlineNotice(
            severity: NoticeSeverity.info,
            dense: true,
            message: '「导入并生效」只绑定当前会话；「导入为全局人设」写主链、对所有会话生效。',
          ),
        ),
        if (widget.pickCardFile == null)
          const Padding(
            padding: EdgeInsets.only(top: Space.s1),
            child: InlineNotice(
              severity: NoticeSeverity.info,
              dense: true,
              message: '这个构建没有接文件选择入口：把卡 JSON 粘贴到上面的框里'
                  '（粘贴是主路径，JSON 卡与带 chara 的 PNG 卡都能用）。',
            ),
          ),
        if (!_enabled)
          const Padding(
            padding: EdgeInsets.only(top: Space.s1),
            child: InlineNotice(
              severity: NoticeSeverity.info,
              dense: true,
              message: '先打开上面的开关再导入：命令通道要 Mod 正在运行'
                  '（停用时服务端回 503 command_unavailable）。',
            ),
          ),
      ],
    );
  }

  /// 与「记忆」Mod 的关系：**last-writer-wins，没有仲裁**（与 memory 文档同口径）。
  ///
  /// L1 起只有**全局**模式才会写主链，会话绑定的卡进的是宿主会话表——
  /// 这条共存规则因此只在全局模式下相关（面板必须把这一点说清）。
  Widget _memoryNotice() => const Padding(
    padding: EdgeInsets.only(top: Space.s3),
    child: InlineNotice(
      severity: NoticeSeverity.info,
      dense: true,
      message: '只有「导入为全局人设」会写主链 persona.system_prompt：'
          '后写覆盖、不做仲裁——想稳定用全局人设就别同时开「记忆」的注入开关。'
          '会话绑定的卡不进主链，不受这条影响。',
    ),
  );
}

List<String> _stringList(Object? value) {
  if (value is List) {
    return <String>[for (final Object? v in value) if (v is String) v];
  }
  return const <String>[];
}

String _str(Object? value) => value is String ? value : '';
