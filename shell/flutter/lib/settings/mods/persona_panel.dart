/// persona 的产品面板（2026-10-08：角色卡导入回到扩展卡片）。
///
/// # 为什么导入在这里
///
/// T6（2026-10-07）把导入接回了「人设」页，于是同一个动作两套入口。2026-10-08
/// 维护者口径：底座 = 系统提示词 + 记住几轮；**角色卡属扩展**——导入、清除、
/// PNG 选择全部回到本卡片，「人设」页只留主链两项。
///
/// # 命令契约（与 Rust `command.rs` 的头注逐字对应）
///
/// | 按钮 | 命令 | args |
/// | --- | --- | --- |
/// | 导入并生效 | `import_card` | `card_json` 或 `data_base64` + 当前 `session_id` |
/// | 导入为全局人设 | `import_card` | 只给卡，**不给** `session_id` |
/// | 清除当前会话的卡 | `clear_import` | 带 `session_id` |
/// | 清除全局导入卡 | `clear_import` | 不带 `session_id` |
///
/// 三条不变量：
/// 1. **没有活动会话时绝不偷偷全局导入**：按钮禁用 + 就地写明原因，代码里还有
///    一道守卫（见 [_importForSession]）；
/// 2. 命令通道走 [ModPanelContext.onCommand]（宿主接线一次，测试注入 fake）；
/// 3. PNG 选择走 [ModPanelContext.pickCardFile]——组合根注入，
///    本文件**不** import `app/browser_io.dart`（那会把 `package:web` 拖进
///    dart VM 的测试编译面）。
///
/// # 8 个配置键经 [hiddenKeys] 从产品面整体消失
///
/// 与 `crates/live2d-ai-mod-persona/src/factory.rs` 的
/// `persona_settings_spec()` **逐键对应**（一个不多、一个不少）。它们不进
/// 「高级」折叠，也不渲染；键仍被解析、`mods.json` 里已有的值保存时原样带走
/// （见 `dev_tools_mod_config.dart` 的 `_buildConfig`）。
///
/// 本文件由 persona 轨道独占，其他轨道不要改。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../api/api_client.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../../ui/inline_notice.dart';
import 'mod_panel.dart';

export 'mod_panel.dart' show PersonaCardFilePicker;

/// 卡片开头那句话。
const String kPersonaTakeoverNotice =
    // 2026-10-09：设置页一级从「人设」改名「模型对话」——这句指回那一页，
    // 必须跟着改（按钮「导入为全局人设」是另一个词，保持不变）。
    '打开后，用导入的角色卡说话。关掉会回到「模型对话」里写的提示词。';

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
  const PersonaPanel();

  @override
  String get modId => 'persona';

  /// 8 个配置键：产品界面不渲染，**也不进「高级」**。
  ///
  /// 与 `persona_settings_spec()` 逐键对应：卡文件路径 / 卡 JSON / 附加纪律 /
  /// 开场白 / 四个覆盖项。卡走的是下面的导入命令，这四个覆盖项由导入的
  /// 卡本身提供——都不该再让用户手填。
  @override
  Set<String> get hiddenKeys => const <String>{
    'card_path',
    'card_json',
    'include_discipline',
    'say_first_mes',
    'name',
    'description',
    'personality',
    'scenario',
  };

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
      _PersonaBody(ctx: ctx);
}

/// 卡片本体：开头一句话 + 导入区 + 「当前卡」一句 + 就地结果。
class _PersonaBody extends StatefulWidget {
  const _PersonaBody({required this.ctx});

  final ModPanelContext ctx;

  @override
  State<_PersonaBody> createState() => _PersonaBodyState();
}

class _PersonaBodyState extends State<_PersonaBody> {
  /// 粘贴框（粘贴是主路径：卡本来就是 JSON）。
  final TextEditingController _card = TextEditingController();

  /// 有一条命令正在飞（按钮全禁用，防重复提交）。
  bool _busy = false;

  /// 最近一条**看得见**的结果（成功 / 失败都留在这儿，不弹 toast）。
  String? _message;
  bool _isError = false;

  ModPanelContext get _ctx => widget.ctx;

  @override
  void dispose() {
    _card.dispose();
    super.dispose();
  }

  bool get _hasSession {
    final String? id = _ctx.activeSessionId;
    return id != null && id.isNotEmpty;
  }

  /// 会话导入：把 `session_id` 拼进入参；没有活动会话就**就地拒绝**。
  ///
  /// 守卫与禁用的按钮**都**在：按钮禁用是体验，这里是契约——将来接线错了也
  /// **不会**静默退化成全局导入（那正是这一波要根除的串人设）。
  Future<void> _importForSession(
    Map<String, Object?> args, {
    String command = 'import_card',
  }) async {
    final String? session = _ctx.activeSessionId;
    if (session == null || session.isEmpty) {
      setState(() {
        _isError = true;
        _message = '还没有会话：先发一条消息（会自动建会话）再导入；'
            '要让所有会话都换人设，请用「导入为全局人设」。';
      });
      return;
    }
    await _run(
      <String, Object?>{...args, 'session_id': session},
      session: true,
      command: command,
    );
  }

  /// 全局导入：**不带** `session_id`（服务端按全局作用域处理）。
  Future<void> _importGlobally(Map<String, Object?> args) =>
      _run(args, session: false);

  /// 粘贴框里的文本；空 → 就地报错（不发请求）。
  String? _pastedCard() {
    final String text = _card.text.trim();
    if (text.isEmpty) {
      setState(() {
        _isError = true;
        _message = '先粘贴角色卡 JSON 再点导入';
      });
      return null;
    }
    return text;
  }

  /// 粘贴框 → `import_card(card_json)`，**绑定当前会话**（主路径）。
  Future<void> _importPasted() async {
    final String? text = _pastedCard();
    if (text == null) return;
    await _importForSession(<String, Object?>{'card_json': text});
  }

  /// 粘贴框 → `import_card(card_json)`，**不给 session_id** = 全局行为。
  Future<void> _importPastedGlobally() async {
    final String? text = _pastedCard();
    if (text == null) return;
    await _importGlobally(<String, Object?>{'card_json': text});
  }

  /// 清除**当前会话**绑定的角色卡（其他会话不受影响）。
  Future<void> _clearSession() =>
      _importForSession(const <String, Object?>{}, command: 'clear_import');

  /// 清除**全局**导入卡（主链回到它自己的基线来源）。
  Future<void> _clearGlobal() =>
      _run(const <String, Object?>{}, session: false, command: 'clear_import');

  /// 选一张 PNG 卡 → `import_card(data_base64)`，同样绑当前会话（前缀由 Mod 剥）。
  Future<void> _pickAndImport() async {
    final PersonaCardFilePicker? pick = _ctx.pickCardFile;
    if (pick == null) {
      setState(() {
        _isError = true;
        _message = '这个构建没有接文件选择入口：把卡 JSON 粘贴到上面的框里';
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
            '${kPersonaCardDataUrlMaxChars ~/ 1024} KB）：角色卡 PNG 通常远小于此';
      });
      return;
    }
    await _importForSession(<String, Object?>{'data_base64': dataUrl});
  }

  /// 跑一条导入 / 清除，并把结果如实转成一句话（失败带**错误码**）。
  ///
  /// 成功后经 [ModPanelContext.notifyChanged] 通知宿主（统一的「重启后生效」
  /// 提示）——人设是运行行为，界面不该假装它立刻无痕生效。
  Future<void> _run(
    Map<String, Object?> args, {
    required bool session,
    String command = 'import_card',
  }) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _message = null;
    });
    try {
      final ModCommandResult result = await _ctx.onCommand(command, args);
      if (!mounted) return;
      if (!result.ok) {
        setState(() {
          _busy = false;
          _isError = true;
          _message = '导入失败：服务端回了 ok=false（没有说明原因）';
        });
        return;
      }
      final String message = command == 'clear_import'
          ? _clearMessage(result, session: session)
          : _successMessage(result, session: session);
      setState(() {
        _busy = false;
        _isError = false;
        _message = message;
      });
      _ctx.notifyChanged(message);
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _isError = true;
        _message = _errorMessage(e, command: command);
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _isError = true;
        _message = '导入角色卡失败：$e';
      });
    }
  }

  /// 成功结果 → 一句话（**作用域不同，文案不同**，不许混）。
  String _successMessage(ModCommandResult result, {required bool session}) {
    final String name = _str(result.result['card_name']);
    final String format = personaCardFormatLabel(
      _str(result.result['card_format']),
    );
    final String card = name.isEmpty ? '这张卡' : '「$name」（$format）';
    if (!session) {
      return '已导入$card为全局人设（所有会话生效）：它改写的是主链提示词';
    }
    final String id = _str(result.result['session_id']);
    final String target = id.isEmpty ? '当前会话' : '会话 $id';
    return '已导入$card并绑定到$target：只对当前会话生效';
  }

  /// 清除成功 → 一句话（**作用域不同，文案不同**，不许混）。
  String _clearMessage(ModCommandResult result, {required bool session}) {
    final bool cleared = result.result['cleared'] == true;
    if (session) {
      final String id = _str(result.result['session_id']);
      final String target = id.isEmpty ? '当前会话' : '会话 $id';
      if (!cleared) return '$target 本来就没有绑定角色卡，不用清除';
      return '已清除$target 的角色卡（其他会话不受影响）';
    }
    if (!cleared) return '本来就没有导入的角色卡，不用清除';
    final String source = personaCardSourceLabel(
      _str(result.result['card_source']),
    );
    return '已清除全局导入卡；主链改由「$source」决定';
  }

  /// 失败 → 一句话 + **错误码**（用户要拿界面上的码去日志里搜）。
  String _errorMessage(ApiException e, {String command = 'import_card'}) {
    final String action = command == 'clear_import' ? '清除角色卡' : '导入角色卡';
    return switch (e.code) {
      'command_unavailable' =>
        '$action失败：persona 没在运行。先在卡片标题行打开它'
            '（503 command_unavailable）',
      'unsupported_command' =>
        '这个版本的 persona Mod 不认识这条命令（409 unsupported_command）',
      'not_found' => '服务端注册表里没有这个 Mod（404 not_found）',
      'command_failed' => '$action失败：${e.message}（409 command_failed）',
      _ => '$action失败：${e.message}（${e.code}）',
    };
  }

  /// 运行态里那张卡的**名字**（没有就不画「当前卡」那一句）。
  String get _cardName {
    final Object? state = _ctx.state;
    if (state is! Map) return '';
    return _str(state['card_name']);
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final bool base = !_busy;
    final bool sessionReady = _hasSession && base;
    final bool hasPicker = _ctx.pickCardFile != null;
    final String cardName = _cardName;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s3),
        const InlineNotice(
          message: kPersonaTakeoverNotice,
          severity: NoticeSeverity.info,
          dense: true,
        ),
        const SizedBox(height: Space.s2),
        Text('角色卡导入', style: theme.textTheme.labelLarge),
        const SizedBox(height: Space.s2),
        TextField(
          key: const Key('persona-card-json'),
          controller: _card,
          enabled: base,
          minLines: 3,
          maxLines: 6,
          decoration: const InputDecoration(
            isDense: true,
            border: OutlineInputBorder(),
            labelText: '粘贴角色卡 JSON',
            hintText: '把 .json 卡整段粘到这里',
          ),
        ),
        const SizedBox(height: Space.s2),
        Wrap(
          spacing: Space.s2,
          runSpacing: Space.s1,
          children: <Widget>[
            FilledButton.tonal(
              key: const Key('persona-import-session'),
              onPressed: sessionReady ? () => unawaited(_importPasted()) : null,
              child: Text(_busy ? '处理中…' : '导入并生效'),
            ),
            OutlinedButton(
              key: const Key('persona-import-global'),
              onPressed: base ? () => unawaited(_importPastedGlobally()) : null,
              child: const Text('导入为全局人设'),
            ),
            if (hasPicker)
              OutlinedButton(
                key: const Key('persona-import-file'),
                onPressed: sessionReady
                    ? () => unawaited(_pickAndImport())
                    : null,
                child: const Text('选择 PNG 角色卡文件'),
              ),
            TextButton(
              key: const Key('persona-clear-session'),
              onPressed: sessionReady ? () => unawaited(_clearSession()) : null,
              child: const Text('清除当前会话的卡'),
            ),
            TextButton(
              key: const Key('persona-clear-global'),
              onPressed: base ? () => unawaited(_clearGlobal()) : null,
              child: const Text('清除全局导入卡'),
            ),
          ],
        ),
        const Padding(
          padding: EdgeInsets.only(top: Space.s1),
          child: InlineNotice(
            severity: NoticeSeverity.info,
            dense: true,
            message: '「导入并生效」只绑定当前会话；'
                '「导入为全局人设」写主链、对所有会话生效。',
          ),
        ),
        if (!_hasSession)
          const Padding(
            padding: EdgeInsets.only(top: Space.s1),
            child: InlineNotice(
              severity: NoticeSeverity.warning,
              dense: true,
              message: '还没有会话：「导入并生效」不可用——先发一条消息会自动建会话。'
                  '要让所有会话都换人设，用「导入为全局人设」。',
            ),
          ),
        if (!hasPicker)
          const Padding(
            padding: EdgeInsets.only(top: Space.s1),
            child: InlineNotice(
              severity: NoticeSeverity.info,
              dense: true,
              message: '这个构建没有接文件选择入口：把卡 JSON 粘贴到上面的框里。',
            ),
          ),
        if (_message != null)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: InlineNotice(
              key: const Key('persona-import-result'),
              message: _message!,
              severity: _isError ? NoticeSeverity.danger : NoticeSeverity.info,
              dense: true,
            ),
          ),
        if (cardName.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: Text(
              '当前卡：「$cardName」· '
              '${personaScopeLabel(_scopeOf(_ctx.state))}',
              style: theme.textTheme.bodySmall,
            ),
          ),
      ],
    );
  }
}

String _scopeOf(Object? state) => state is Map ? _str(state['scope']) : '';

String _str(Object? value) => value is String ? value : '';
