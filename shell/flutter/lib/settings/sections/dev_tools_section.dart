/// 「Mod」「诊断」「模型库」「开发模式」四个分区的实现。
///
/// 合在一个文件里：它们共享同一套「列表 + 动作 + 内联结果」的骨架，
/// 拆成四个文件只会让同一段列表渲染代码出现四遍。每个类都短、
/// 职责单一，找起来靠类名即可。
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../api/api_client.dart';
import '../../api/diagnostics_api.dart';
import '../../api/models_api.dart';
import '../../api/mods_api.dart';
import '../../design/tokens.dart';
import '../mods/mod_panel.dart';
import '../mods/mod_panels.dart';
import '../../ui/emphasized_text.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../../ui/inline_notice.dart';
import '../../ui/theme.dart';

/// 列表里的一行（四个分区共用）。
class AdminRow extends StatelessWidget {
  const AdminRow({
    required this.title,
    required this.subtitle,
    this.badges = const <String>[],
    this.trailing,
    super.key,
  });

  final String title;
  final String subtitle;

  /// 文字徽标（**不靠颜色表意**）。
  final List<String> badges;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: Space.s2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(title, style: theme.textTheme.titleSmall),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: colors.contentMuted,
                  ),
                ),
                if (badges.isNotEmpty) ...<Widget>[
                  const SizedBox(height: Space.s1),
                  Wrap(
                    spacing: Space.s1,
                    children: <Widget>[
                      for (final String b in badges)
                        DecoratedBox(
                          decoration: BoxDecoration(
                            // 槽位从 ColorScheme 取（AppColors 里没有这一项）。
                            color: Theme.of(context)
                                .colorScheme
                                .surfaceContainerHighest,
                            borderRadius: BorderRadius.circular(AppRadius.xs),
                          ),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: Space.s2,
                              vertical: 1,
                            ),
                            child: Text(b, style: theme.textTheme.labelSmall),
                          ),
                        ),
                    ],
                  ),
                ],
              ],
            ),
          ),
          ?trailing,
        ],
      ),
    );
  }
}

/// 空态（**空白面板会被读成「坏了」**）。
class AdminEmpty extends StatelessWidget {
  const AdminEmpty({required this.icon, required this.title, required this.hint, super.key});

  final IconData icon;
  final String title;
  final String hint;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: Space.s5),
      child: Column(
        children: <Widget>[
          Icon(icon, size: 28, color: appColorsOf(context).contentFaint),
          const SizedBox(height: Space.s2),
          Text(title, style: theme.textTheme.titleSmall),
          const SizedBox(height: Space.s1),
          Text(
            hint,
            textAlign: TextAlign.center,
            style: theme.textTheme.bodySmall?.copyWith(
              color: appColorsOf(context).contentMuted,
            ),
          ),
        ],
      ),
    );
  }
}

// ────────────────────────────────────────────────────────────── 模型库

class ModelsSection extends StatelessWidget {
  const ModelsSection({
    required this.models,
    required this.loading,
    this.error,
    this.devMode = false,
    this.onActivate,
    this.onReload,
    this.onImport,
    this.busyId,
    this.activateMessage,
    super.key,
  });

  final List<ModelInfo> models;
  final bool loading;
  final String? error;
  final bool devMode;
  final Future<void> Function(String id)? onActivate;
  final Future<void> Function()? onReload;

  /// 导入一个**已在磁盘上**的模型目录（`assets/models/<id>/`）。
  ///
  /// 后端不做文件上传（ZIP 上传是后置项且**曾被我方能力快照谎报为 supported**）：
  /// 用户把模型放进目录，这里登记 + 校验。没有这个入口，「导入 → 激活 → 换皮」
  /// 这条 DoD 主路径在界面上就没有起点。
  final Future<void> Function(String id)? onImport;

  /// 正在激活的模型 id（禁用该行按钮，防重复提交）。
  final String? busyId;
  final String? activateMessage;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '模型库',
          description: '模型由你合法导入，本仓库不捆绑任何模型二进制。',
        ),
        if (error != null)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s2),
            // 同上：统一走 `InlineNotice`（2026-09-11，P1-5）。
            child: InlineNotice(message: error!),
          ),
        if (activateMessage != null)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s2),
            child: EmphasizedText(
              activateMessage!,
              style: theme.textTheme.bodySmall,
            ),
          ),
        if (loading && models.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: Space.s4),
            child: Center(child: CircularProgressIndicator()),
          )
        else if (models.isEmpty)
          const AdminEmpty(
            icon: Icons.view_in_ar_outlined,
            title: '还没有导入模型',
            hint: '把模型放到 assets/models/<id>/ 下，然后在下面填目录名导入。'
                '列表为空是正常的——项目刻意不捆绑模型。',
          )
        else
          for (final ModelInfo m in models)
            AdminRow(
              title: m.displayName.isEmpty ? m.id : m.displayName,
              subtitle: '${m.id} · v${m.version} · ${m.humanSize} · '
                  '${m.textureCount} 张贴图',
              badges: <String>[
                if (m.active) '当前激活',
                if (m.hasPhysics) '含物理',
                if (m.hasDisplayInfo) '有显示配置',
              ],
              trailing: m.active
                  ? null
                  : FilledButton.tonal(
                      onPressed: busyId != null || onActivate == null
                          ? null
                          : () => onActivate!(m.id),
                      child: Text(busyId == m.id ? '激活中…' : '激活'),
                    ),
            ),
        if (onImport != null) ...<Widget>[
          const SizedBox(height: Space.s3),
          _ImportModelField(onImport: onImport!, busy: busyId != null),
        ],
        if (onReload != null) ...<Widget>[
          const SizedBox(height: Space.s3),
          Align(
            alignment: Alignment.centerLeft,
            child: OutlinedButton.icon(
              onPressed: () => onReload!(),
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('重新载入列表'),
            ),
          ),
        ],
        if (devMode)
          const Padding(
            padding: EdgeInsets.only(top: Space.s3),
            child: Text(
              '开发者提示：导入只接受 assets/models/ 下已存在的目录名；'
              '删除激活中的模型会被服务端拒绝（409 model_active）。',
            ),
          ),
      ],
    );
  }
}

/// 「导入」输入行：填 `assets/models/` 下的目录名 → `POST /models/import`。
///
/// 有状态只是为了拿住 `TextEditingController`；导入本身由宿主执行
/// （它负责刷新列表与提示），成功后清空输入框。
class _ImportModelField extends StatefulWidget {
  const _ImportModelField({required this.onImport, required this.busy});

  final Future<void> Function(String id) onImport;
  final bool busy;

  @override
  State<_ImportModelField> createState() => _ImportModelFieldState();
}

class _ImportModelFieldState extends State<_ImportModelField> {
  final TextEditingController _id = TextEditingController();

  @override
  void dispose() {
    _id.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final String id = _id.text.trim();
    if (id.isEmpty) return;
    await widget.onImport(id);
    if (mounted) _id.clear();
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      children: <Widget>[
        Expanded(
          child: TextField(
            controller: _id,
            enabled: !widget.busy,
            decoration: const InputDecoration(
              isDense: true,
              labelText: '导入模型（目录名）',
              hintText: '例如 bai',
            ),
            onSubmitted: (_) => unawaited(_submit()),
          ),
        ),
        const SizedBox(width: Space.s2),
        FilledButton.tonal(
          onPressed: widget.busy ? null : () => unawaited(_submit()),
          child: const Text('导入'),
        ),
      ],
    );
  }
}

// ──────────────────────────────────────────────── Mod 运行态（Wave 3）

/// 运行态读取器：`GET /api/v1/mods/{id}/state`（Wave 3 起前端消费）。
///
/// 用回调而不是直接传 `ModsApi`：与同文件的 `onSaveConfig` 同款——宿主接线一次，
/// widget 测试注入 fake 即可覆盖「改配置 → 重取 state → 字段跟着变」。
typedef ModStateLoader = Future<ModStateResult> Function(String id);

/// `state_json().window.reason` 的稳定值：原生壳休眠、窗口未开。
///
/// **必须逐字等于** Rust 侧 `WINDOW_REASON_NATIVE_SHELL_DORMANT`
///（`live2d-ai-mod-pet-desktop`）；守卫在 `test/pet_desktop_state_test.dart`
/// 与 Rust 的 `window_reason_string_is_stable_and_ascii`。
const String kWindowReasonNativeShellDormant = 'native_shell_dormant';

/// 通用运行态字段的中文标签（**兜底表**）。
///
/// 产品级加强波次起，**每个 Mod 自己的字段标签住在它的面板文件里**
///（`settings/mods/*_panel.dart` 的 `stateLabels`），由 `modStateLabelsFor` 汇总。
/// 这张表只留**跨 Mod 通用**的字段名（含历史遗留的 pet-desktop 字段——该 Mod 已封存，
/// 但通用渲染仍认识这些 key）。
///
/// 未知 key **不隐藏**（回落原始 key）——未来 Mod / 新字段不能因为前端不认识
/// 就从界面上消失（「看得见才谈得上如实」）。
const Map<String, String> kModStateLabels = <String, String>{
  'always_on_top': '总在最前',
  'click_through': '点击穿透',
  'opacity': '不透明度',
  'voice_active': '语音活跃',
  'window': '窗口',
};

/// 某 Mod 的标签表 = 该 Mod 面板声明的标签（在前）+ 通用兜底表。
///
/// `modId` 为 null / 没有专用面板 → 只有兜底表。纯函数。
Map<String, String> modStateLabelsForMod(String? modId) {
  final Map<String, String> panel = modId == null
      ? const <String, String>{}
      : modStateLabelsFor(modId);
  if (panel.isEmpty) return kModStateLabels;
  return <String, String>{...panel, ...kModStateLabels};
}

/// 展示顺序：已知字段按「面板声明序 → 兜底表序」在前，其余按 key 字典序在后。纯函数。
List<String> orderedModStateKeys(Map<String, Object?> state, {String? modId}) {
  final Map<String, String> labels = modStateLabelsForMod(modId);
  final List<String> known = <String>[
    for (final String k in labels.keys)
      if (state.containsKey(k)) k,
  ];
  final List<String> rest = <String>[
    for (final String k in state.keys)
      if (!labels.containsKey(k)) k,
  ]..sort();
  return <String>[...known, ...rest];
}

/// 字段标签：已知字段中文，未知字段回落原始 key。纯函数。
///
/// `modId` 给定时优先用该 Mod 面板声明的标签（面板优先于通用兜底表）。
String modStateLabel(String key, {String? modId}) =>
    modStateLabelsForMod(modId)[key] ?? key;

/// 把一条 `(key, value)` 渲染成一行文字（**不靠颜色**）。纯函数，可单测。
///
/// `window` 子对象特判——软闭环的那句文案就在 [`formatWindowState`]。
String formatModStateValue(String key, Object? value) {
  if (key == 'window' && value is Map) {
    return formatWindowState(_stringKeyed(value));
  }
  if (value is bool) return value ? '开' : '关';
  if (value is num) {
    return value is int ? '$value' : _trimNumber(value);
  }
  if (value is String) return value.isEmpty ? '（空）' : value;
  if (value is Map) return '${value.length} 项';
  if (value is List) return '${value.length} 项';
  if (value == null) return '—';
  return '$value';
}

/// `window` 子对象的展示。
///
/// 休眠态呈现成「窗口未开（原生壳休眠），此面仅状态」——用户看到的是
/// **「这是刻意的」**，而不是「窗口本该开着但坏了」。纯函数，可单测。
String formatWindowState(Map<String, Object?> window) {
  if (window['opened'] == true) return '已打开';
  final Object? reason = window['reason'];
  if (reason == kWindowReasonNativeShellDormant) {
    return '窗口未开（原生壳休眠），此面仅状态';
  }
  final String r = reason is String ? reason : '';
  return r.isEmpty ? '未打开' : '未打开（$r）';
}

String _trimNumber(num value) {
  final double d = value.toDouble();
  if (d == d.roundToDouble() && d.abs() < 1e15) return d.toInt().toString();
  return '$value';
}

Map<String, Object?> _stringKeyed(Map<Object?, Object?> raw) {
  final Map<String, Object?> out = <String, Object?>{};
  raw.forEach((Object? k, Object? v) {
    if (k is String) out[k] = v;
  });
  return out;
}

// ────────────────────────────────────────────────────────────── Mod

class ModsSection extends StatelessWidget {
  const ModsSection({
    required this.mods,
    required this.loading,
    this.error,
    this.onToggle,
    this.onSaveConfig,
    this.onLoadState,
    this.onReload,
    this.busyId,
    super.key,
  });

  final List<ModInfo> mods;
  final bool loading;
  final String? error;
  final Future<void> Function(String id, bool enabled)? onToggle;

  /// 保存某个 Mod 的配置（`POST /api/v1/mods/{id}/config`）。
  ///
  /// 为 null 时展开的表单只读展现、保存按钮禁用——**不假装能保存**。
  final Future<ModConfigResult> Function(String id, Map<String, Object?> config)?
      onSaveConfig;

  /// 展开卡片时读该 Mod 的运行态（`GET /api/v1/mods/{id}/state`）。
  ///
  /// 为 null 时**卡片自己**造一个同源 `ModsApi()` 兜底（理由见
  /// `_ModConfigTileState._loader` 头注）——所以真机展开就能看到运行态，
  /// 不依赖宿主接线；测试注入 fake 则完全隔离网络。
  final ModStateLoader? onLoadState;

  final Future<void> Function()? onReload;
  final String? busyId;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: 'Mod',
          description: '扩展能力走 Mod 边界隔离，默认全部停用。'
              '核心只提供接口，不把功能堆进来。',
        ),
        if (error != null)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s2),
            // 同上：统一走 `InlineNotice`（2026-09-11，P1-5）。
            child: InlineNotice(message: error!),
          ),
        if (loading && mods.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: Space.s4),
            child: Center(child: CircularProgressIndicator()),
          )
        else if (mods.isEmpty)
          const AdminEmpty(
            icon: Icons.extension_outlined,
            title: '没有注册任何 Mod',
            hint: 'Mod 通过 live2d-ai-mod-system 的 trait 注册中心接入。',
          )
        else
          for (final ModInfo m in mods)
            // 有 spec 的 Mod 展开后按 spec 渲表单；旧 Mod（无 spec）保持
            // 「一行 + 开关」的现状——这是 M2 明确要求的兼容面。
            if (m.settingsSpec == null || m.settingsSpec!.fields.isEmpty)
              AdminRow(
                title: m.name.isEmpty ? m.id : m.name,
                subtitle: '${m.id} · v${m.version} · api v${m.apiVersion}',
                // **状态用文字**（「运行中」/「已停用」），不靠颜色。
                badges: <String>[m.statusLabel],
                trailing: Switch(
                  value: m.enabled,
                  onChanged: busyId != null || onToggle == null
                      ? null
                      : (bool v) => onToggle!(m.id, v),
                ),
              )
            else
              _ModConfigTile(
                mod: m,
                busy: busyId != null,
                onToggle: onToggle,
                onSaveConfig: onSaveConfig,
                onLoadState: onLoadState,
              ),
        if (onReload != null) ...<Widget>[
          const SizedBox(height: Space.s3),
          Align(
            alignment: Alignment.centerLeft,
            child: OutlinedButton.icon(
              onPressed: () => onReload!(),
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('重新载入'),
            ),
          ),
        ],
      ],
    );
  }
}

/// 一个有 `settings_spec` 的 Mod：展开后按 spec 渲染配置表单。
///
/// # 为什么表单状态住在这里而不是宿主
///
/// 宿主只负责「把保存请求发出去」；每个 Mod 的字段草稿、保存中的禁用、
/// 成功/失败文案都是**这一张卡片**的局部状态。放进宿主会让「保存了哪个
/// Mod 的哪一项」与全局的 `_adminMessage` 缠在一起，也给不出逐卡片的结果。
class _ModConfigTile extends StatefulWidget {
  const _ModConfigTile({
    required this.mod,
    required this.busy,
    required this.onToggle,
    required this.onSaveConfig,
    required this.onLoadState,
  });

  final ModInfo mod;
  final bool busy;
  final Future<void> Function(String id, bool enabled)? onToggle;
  final Future<ModConfigResult> Function(String id, Map<String, Object?> config)?
      onSaveConfig;
  final ModStateLoader? onLoadState;

  @override
  State<_ModConfigTile> createState() => _ModConfigTileState();
}

class _ModConfigTileState extends State<_ModConfigTile> {
  /// 每个字段的当前值：服务端 config 优先，缺该键回落 spec 默认值。
  late Map<String, Object?> _values = _initialValues();

  bool _saving = false;
  String? _message;
  bool _messageIsError = false;

  /// 运行态快照：展开时懒加载，保存成功后重取（Wave 3 软闭环）。
  ModStateResult? _state;
  String? _stateError;
  bool _stateLoading = false;

  /// host 没接线时自建的读取器（惰性；见 [_loader]）。
  ModsApi? _ownedApi;

  /// 运行态读取器：宿主传了 `onLoadState` 就用它，否则**自己造一个同源
  /// `ModsApi()`**。
  ///
  /// 为什么默认自建：`ModsSection` 的宿主接线在 `app/shell_settings.dart`，
  /// 那个文件不属本轨所有权（Wave 3 并行轨的文件边界，见
  /// `REGISTER-pet-desktop-v1.md` §2.1）。而 `ModsApi()` 的 base 解析
  ///（`API_BASE` → `Uri.base.origin`）与 `main.dart` 里那个**同源**，
  /// 所以真机展开卡片就能看到运行态，不依赖额外接线；测试注入 fake 则零网络。
  ModStateLoader get _loader => widget.onLoadState ?? _ownedLoad;

  Future<ModStateResult> _ownedLoad(String id) =>
      (_ownedApi ??= ModsApi()).state(id);

  ModSettingsSpec get _spec => widget.mod.settingsSpec!;

  Map<String, Object?> _initialValues() {
    final Map<String, Object?> out = <String, Object?>{};
    for (final ModSettingField f in _spec.fields) {
      out[f.key] = widget.mod.config.containsKey(f.key)
          ? widget.mod.config[f.key]
          : f.defaultValue;
    }
    return out;
  }

  @override
  void didUpdateWidget(_ModConfigTile oldWidget) {
    super.didUpdateWidget(oldWidget);
    // 宿主保存后重取列表 → 服务端的 config 是新的，用它回填草稿。
    // （保存成功那一刻，服务端归一化后的值才是真相。）
    if (!identical(oldWidget.mod.config, widget.mod.config)) {
      _values = _initialValues();
    }
  }

  bool _boolValue(ModSettingField f) => _values[f.key] is bool
      ? _values[f.key]! as bool
      : f.defaultValue == true;

  int _intValue(ModSettingField f) {
    final Object? raw = _values[f.key];
    if (raw is num) return raw.toInt();
    if (raw is String) return int.tryParse(raw) ?? 0;
    return 0;
  }

  String _stringValue(ModSettingField f) {
    final Object? raw = _values[f.key];
    return raw is String ? raw : '';
  }

  String _selectValue(ModSettingField f) {
    final String raw = _stringValue(f);
    if (f.options.any((ModSelectOption o) => o.value == raw)) return raw;
    return f.options.first.value;
  }

  /// 组装要提交的 config。
  ///
  /// 从服务端已有 config 出发、只覆盖本 Mod 在 spec 里声明的字段：
  /// spec 之外的既有键（未来扩展）不该被一次「保存」顺手抹掉。
  Map<String, Object?> _buildConfig() {
    final Map<String, Object?> next = Map<String, Object?>.of(widget.mod.config);
    for (final ModSettingField f in _spec.fields) {
      // secret 留空 = 「不修改」：服务端不回值，发空串会把已存的密钥清掉。
      if (f.kind == ModFieldKind.string &&
          f.secret &&
          _stringValue(f).isEmpty) {
        continue;
      }
      next[f.key] = switch (f.kind) {
        ModFieldKind.bool => _boolValue(f),
        ModFieldKind.number => _intValue(f),
        ModFieldKind.select => _selectValue(f),
        ModFieldKind.string => _stringValue(f),
      };
    }
    return next;
  }

  Future<void> _save() async {
    final Future<ModConfigResult> Function(String, Map<String, Object?>)? save =
        widget.onSaveConfig;
    if (save == null) return;
    setState(() {
      _saving = true;
      _message = null;
    });
    try {
      final ModConfigResult result = await save(widget.mod.id, _buildConfig());
      if (!mounted) return;
      setState(() {
        _saving = false;
        _messageIsError = !result.ok;
        _message = result.ok ? _okMessage(result) : '保存失败：服务端返回 ok=false';
      });
      // 热更新（Wave 3）：配置改了 → 立刻重取运行态，让「字段跟着变」可见。
      if (result.ok) unawaited(_loadState());
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _messageIsError = true;
        // 带上错误码：用户要拿界面上的码去日志里搜（项目错误契约）。
        _message = '保存失败：$e';
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _messageIsError = true;
        _message = '保存失败：$e';
      });
    }
  }

  String _okMessage(ModConfigResult r) {
    if (r.restarted) return '已保存，服务端已重启';
    if (r.enabled == false) return '已保存，Mod 已停用';
    return '已保存';
  }

  @override
  void dispose() {
    // 只关自己造的那个客户端；宿主注入的由宿主管生命周期。
    _ownedApi?.dispose();
    super.dispose();
  }

  /// 读一次运行态。失败**不吞**：错误码上屏（`state_unavailable` 与
  /// `not_found` 的处置完全不同，见 [_stateErrorMessage]）。
  Future<void> _loadState() async {
    if (_stateLoading) return;
    setState(() {
      _stateLoading = true;
      _stateError = null;
    });
    try {
      final ModStateResult result = await _loader(widget.mod.id);
      if (!mounted) return;
      setState(() {
        _stateLoading = false;
        _state = result;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _stateLoading = false;
        _state = null;
        _stateError = _stateErrorMessage(e);
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _stateLoading = false;
        _state = null;
        _stateError = '运行态读取失败：$e';
      });
    }
  }

  /// host 的两种失败码 → **可处置**的一句话（不谎报成「不存在」）。
  String _stateErrorMessage(ApiException e) {
    if (e.code == 'state_unavailable') {
      return '运行态暂时读不到（这个 Mod 没启用 / 该 Mod 未实现 state_json / '
          'worker 正忙，503 state_unavailable）';
    }
    if (e.code == 'not_found') {
      return '这个 Mod 不在服务端注册表（404 not_found）';
    }
    return '运行态读取失败：$e';
  }

  @override
  Widget build(BuildContext context) {
    final ModInfo m = widget.mod;
    return ExpansionTile(
      // 展开时懒加载运行态（只展开一次；之后用块里的「刷新运行态」或保存后重取）。
      onExpansionChanged: (bool open) {
        if (open && !_stateLoading && _state == null && _stateError == null) {
          unawaited(_loadState());
        }
      },
      // 头部复用 AdminRow：标题/副标题/状态徽标与无 spec 的 Mod 完全一致。
      title: AdminRow(
        title: m.name.isEmpty ? m.id : m.name,
        subtitle: '${m.id} · v${m.version} · api v${m.apiVersion}',
        badges: <String>[m.statusLabel],
        trailing: Switch(
          value: m.enabled,
          onChanged: widget.busy || widget.onToggle == null
              ? null
              : (bool v) => widget.onToggle!(m.id, v),
        ),
      ),
      childrenPadding: const EdgeInsets.only(
        left: Space.s3,
        right: Space.s3,
        bottom: Space.s2,
      ),
      expandedCrossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final ModSettingField f in _spec.fields) _field(f),
        const SizedBox(height: Space.s2),
        Align(
          alignment: Alignment.centerLeft,
          child: OutlinedButton.icon(
            onPressed: _saving || widget.onSaveConfig == null
                ? null
                : () => unawaited(_save()),
            icon: _saving
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.save_outlined, size: 16),
            label: Text(_saving ? '保存中…' : '保存'),
          ),
        ),
        if (_message != null)
          Padding(
            padding: const EdgeInsets.only(top: Space.s2),
            child: InlineNotice(
              message: _message!,
              severity: _messageIsError
                  ? NoticeSeverity.danger
                  : NoticeSeverity.info,
              dense: true,
            ),
          ),
        _stateBlock(),
        _panel(),
      ],
    );
  }

  /// 「运行态（只读）」块：把 `GET /api/v1/mods/{id}/state` 如实铺开。
  ///
  /// 字段名→中文标签的映射见 [`kModStateLabels`]；未知字段不隐藏。
  /// `window` 的休眠文案（「窗口未开（原生壳休眠），此面仅状态」）由
  /// [`formatWindowState`] 给出——**软闭环的可见终点**。
  Widget _stateBlock() {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: colors.contentMuted,
    );
    final ModStateResult? state = _state;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SizedBox(height: Space.s3),
        Row(
          children: <Widget>[
            Expanded(
              child: Text('运行态（只读）', style: theme.textTheme.titleSmall),
            ),
            TextButton.icon(
              onPressed: _stateLoading ? null : () => unawaited(_loadState()),
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('刷新运行态'),
            ),
          ],
        ),
        if (_stateLoading)
          Text('读取中…', style: muted)
        else if (_stateError != null)
          InlineNotice(message: _stateError!, dense: true)
        else if (state != null)
          if (state.state.isEmpty)
            Text('（该 Mod 未上报运行态字段）', style: muted)
          else
            for (final String key in orderedModStateKeys(
              state.state,
              modId: widget.mod.id,
            ))
              Padding(
                padding: const EdgeInsets.only(top: OpticalNudge.thin),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    SizedBox(
                      width: 72,
                      child: Text(
                        '${modStateLabel(key, modId: widget.mod.id)}：',
                        style: muted,
                      ),
                    ),
                    Expanded(
                      child: Text(
                        formatModStateValue(key, state.state[key]),
                        style: theme.textTheme.bodySmall,
                      ),
                    ),
                  ],
                ),
              ),
      ],
    );
  }

  /// 该 Mod 的**产品面板**（产品级加强波次）：按 id 从 `settings/mods/mod_panels.dart` 取。
  ///
  /// 没有专用面板 / 面板返回 null → 渲染空 widget（旧 Mod 的观感一行不变）。
  /// 面板拿到的是**已经取到的**运行态快照与两个回调，自己不做网络。
  Widget _panel() {
    final ModPanel? panel = modPanelFor(widget.mod.id);
    if (panel == null) return const SizedBox.shrink();
    final Widget? built = panel.build(
      context,
      ModPanelContext(
        mod: widget.mod,
        state: _state?.state,
        stateLoading: _stateLoading,
        stateError: _stateError,
        onRefreshState: _loadState,
        onCommand: _command,
      ),
    );
    return built ?? const SizedBox.shrink();
  }

  /// `POST /api/v1/mods/{id}/command`：一次性动作（清空 / 导出 / 自检……）。
  ///
  /// 失败**不吞**：`ApiException` 原样抛给面板，由面板显示带错误码的文案
  ///（与 `_stateErrorMessage` 同一纪律）。成功后立刻重取运行态，让计数跟上。
  Future<ModCommandResult> _command(
    String command, [
    Map<String, Object?> args = const <String, Object?>{},
  ]) async {
    final ModsApi api = _ownedApi ??= ModsApi();
    final ModCommandResult result = await api.command(
      widget.mod.id,
      command,
      args: args,
    );
    unawaited(_loadState());
    return result;
  }

  Widget _field(ModSettingField f) {
    switch (f.kind) {
      case ModFieldKind.bool:
        return ToggleField(
          label: f.label,
          icon: Icons.toggle_on,
          value: _boolValue(f),
          onChanged: (bool v) => setState(() => _values[f.key] = v),
        );
      case ModFieldKind.string:
        return TextFieldRow(
          label: f.label,
          icon: f.secret ? Icons.key_outlined : Icons.text_fields,
          value: _stringValue(f),
          obscure: f.secret,
          description: f.secret ? '留空表示不修改（服务端不回传密钥）' : null,
          onChanged: (String v) => setState(() => _values[f.key] = v),
        );
      case ModFieldKind.number:
        return NumberField(
          label: f.label,
          icon: Icons.tag,
          value: _intValue(f),
          // 尊重 spec 的 min/max：`NumberField` 对越界输入**不回调**。
          min: f.min?.toInt(),
          max: f.max?.toInt(),
          onChanged: (int v) => setState(() => _values[f.key] = v),
        );
      case ModFieldKind.select:
        return DropdownField<String>(
          label: f.label,
          icon: Icons.list_alt,
          value: _selectValue(f),
          options: <FieldOption<String>>[
            for (final ModSelectOption o in f.options)
              FieldOption<String>(value: o.value, label: o.label),
          ],
          onChanged: (String v) => setState(() => _values[f.key] = v),
        );
    }
  }
}

// ────────────────────────────────────────────────────────────── 诊断

/// 诊断快照（一键复制用）。
class DiagnosticsSnapshot {
  const DiagnosticsSnapshot({required this.lines});

  final List<String> lines;

  String get text => lines.join('\n');
}

class DiagnosticsSection extends StatelessWidget {
  const DiagnosticsSection({
    required this.status,
    required this.capabilities,
    required this.wsStatusLabel,
    this.logs = const <LogLine>[],
    this.logsError,
    this.devMode = false,
    this.onReload,
    this.onCopy,
    this.copied = false,
    super.key,
  });

  final Map<String, Object?> status;
  final AppCapabilities capabilities;
  final String wsStatusLabel;
  final List<LogLine> logs;
  final String? logsError;
  final bool devMode;
  final Future<void> Function()? onReload;
  final Future<void> Function(DiagnosticsSnapshot)? onCopy;
  final bool copied;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final Object? llm = status['llm'];
    final Object? tts = status['tts'];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '诊断',
          description: '当前连接、代次与能力快照。**只读**——这里没有会改状态的按钮。',
        ),
        ReadonlyField(
          label: '实时通道',
          icon: Icons.cable,
          text: wsStatusLabel,
        ),
        ReadonlyField(
          label: '激活模型',
          icon: Icons.view_in_ar_outlined,
          text: '${status['active_model_id'] ?? '（未设置）'}',
        ),
        ReadonlyField(
          label: '当前代次（epoch）',
          icon: Icons.tag,
          text: '${status['current_epoch'] ?? 0}',
          description: '服务端推进 epoch = 上一轮作废（停止 / 抢占）',
        ),
        ReadonlyField(
          label: '服务端版本',
          icon: Icons.info_outline,
          text: '${capabilities.app} ${capabilities.version}'
              '（schema v${capabilities.schemaVersion}，WS 协议 v${capabilities.wsProtocolVersion}）',
        ),
        ReadonlyField(
          label: '服务端音频后端',
          icon: Icons.speaker_outlined,
          text: '${(status['audio'] as Map<String, Object?>?)?['backend'] ?? '未知'}'
              '（sample_rate=${(status['audio'] as Map<String, Object?>?)?['sample_rate'] ?? '?'}）',
          description: 'Web 模式下音频单源是**浏览器**，服务端没有声卡是预期的',
        ),
        ReadonlyField(
          label: 'LLM / TTS 配置',
          icon: Icons.hub_outlined,
          text: 'LLM: ${(llm as Map<String, Object?>?)?['model'] ?? '未配置'}'
              '；TTS: ${(tts as Map<String, Object?>?)?['base_url'] ?? '未配置'}',
        ),
        const SizedBox(height: Space.s3),
        Row(
          children: <Widget>[
            OutlinedButton.icon(
              onPressed: onReload == null ? null : () => onReload!(),
              icon: const Icon(Icons.refresh, size: 16),
              label: const Text('刷新'),
            ),
            const SizedBox(width: Space.s2),
            FilledButton.tonalIcon(
              onPressed: onCopy == null
                  ? null
                  : () => onCopy!(DiagnosticsSnapshot(lines: _snapshotLines())),
              icon: Icon(copied ? Icons.check : Icons.copy, size: 16),
              label: Text(copied ? '已复制' : '复制诊断快照'),
            ),
          ],
        ),
        const SizedBox(height: Space.s3),
        const SectionHeader(
          title: '日志',
          description: '需要先打开「开发模式」；只读最后一屏，不做实时跟随。',
        ),
        if (logsError != null)
          Text(
            logsError!,
            style: theme.textTheme.bodySmall?.copyWith(color: colors.contentMuted),
          )
        else if (logs.isEmpty)
          Text(
            '（暂无日志）',
            style: theme.textTheme.bodySmall?.copyWith(color: colors.contentMuted),
          )
        else
          // 必须虚拟化：日志是长期运行会累积的东西。
          SizedBox(
            height: 220,
            child: ListView.builder(
              itemExtent: 20,
              itemCount: logs.length,
              itemBuilder: (BuildContext context, int i) => Text(
                logs[logs.length - 1 - i].asText,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall,
              ),
            ),
          ),
      ],
    );
  }

  /// 快照内容：**把排查需要的东西一次给全**，而不是让用户来回截图。
  ///
  /// **绝不包含密钥**（前端本来也拿不到）；只含可公开的连接与版本信息。
  List<String> _snapshotLines() => <String>[
    '# Live2D Ai 诊断快照',
    'ws=$wsStatusLabel',
    'app=${capabilities.app} version=${capabilities.version} '
        'schema=${capabilities.schemaVersion} ws_proto=${capabilities.wsProtocolVersion}',
    'active_model=${status['active_model_id'] ?? ''}',
    'epoch=${status['current_epoch'] ?? 0}',
    'uptime_s=${status['uptime_s'] ?? '?'}',
    'audio=${status['audio']}',
    'llm=${status['llm']}',
    'tts=${status['tts']}',
    'dev_mode=${status['dev_mode']}',
  ];
}

/// 复制到剪贴板（**复制是唯一的导出方式**，v1 不做上传）。
Future<void> copySnapshot(DiagnosticsSnapshot snapshot) =>
    Clipboard.setData(ClipboardData(text: snapshot.text));

// ────────────────────────────────────────────────────────── 开发模式

class DeveloperSection extends StatelessWidget {
  const DeveloperSection({
    required this.devMode,
    required this.onDevModeChanged,
    required this.forcedByLaunchFlag,
    super.key,
  });

  final bool devMode;
  final ValueChanged<bool> onDevModeChanged;

  /// 是否由启动参数（`--dev-mode`）强制开启。
  ///
  /// **不谎报成功**：强制开启时开关要显示为「已由启动参数开启」且不可关，
  /// 而不是让用户点一下、看起来关了、其实没关（规格 §13.1-11）。
  final bool forcedByLaunchFlag;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '开发模式',
          description: '打开后才显示高级参数、日志与诊断细节（渐进披露的第二层）。',
        ),
        ToggleField(
          label: '开发者模式',
          icon: Icons.terminal_outlined,
          value: devMode,
          enabled: !forcedByLaunchFlag,
          description: forcedByLaunchFlag
              ? '当前由启动参数强制开启，无法在界面里关闭'
              // 文案必须与**实际行为**一致（2026-09-11 修）：分区本身**不再**
              // 随 dev_mode 隐藏——藏起来就没人能再打开它。
              : '关闭后各分区里的「开发者选项」与诊断细节一起隐藏；'
                    '**本分区始终可见**，否则就再也打不开它了',
          onChanged: onDevModeChanged,
        ),
      ],
    );
  }
}
