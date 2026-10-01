/// 「Mod」「诊断」「模型库」「开发模式」四个分区的实现。
///
/// 合在一个文件里：它们共享同一套「列表 + 动作 + 内联结果」的骨架，
/// 拆成四个文件只会让同一段列表渲染代码出现四遍。每个类都短、
/// 职责单一，找起来靠类名即可。
library;

import 'dart:async';

import 'package:flutter/foundation.dart' show ValueListenable;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../api/api_client.dart';
import '../../api/diagnostics_api.dart';
import '../../api/models_api.dart';
import '../../api/mods_api.dart';
import '../../api/settings_models.dart';
import '../../design/tokens.dart';
import '../../live2d/live2d_stage.dart'
    show PresetStatus, kDefaultExpressionIntensity, kDefaultPresetIntensity;
import '../mods/mod_panel.dart';
import 'director_observer_section.dart';
import '../mods/mod_panels.dart';
import '../preset_labels.dart';
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
  const AdminEmpty({
    required this.icon,
    required this.title,
    required this.hint,
    super.key,
  });

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
            hint:
                '把模型放到 assets/models/<id>/ 下，然后在下面填目录名导入。'
                '列表为空是正常的——项目刻意不捆绑模型。',
          )
        else
          for (final ModelInfo m in models)
            AdminRow(
              title: m.displayName.isEmpty ? m.id : m.displayName,
              subtitle:
                  '${m.id} · v${m.version} · ${m.humanSize} · '
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
    this.restartNotice,
    this.onDismissRestart,
    this.activeSessionId,
    this.onModChanged,
    super.key,
  });

  final List<ModInfo> mods;
  final bool loading;
  final String? error;

  /// 统一的「需重新点火 / 重启后生效」提示（L1 基座）。
  ///
  /// 只要本次会话里发生过 Mod 变更（启停 / 保存配置 / 导入卡 / 导入或清空记忆）
  /// 就一直挂在 Mod 分区顶部，直到用户关掉它——**不随一次 SnackBar 消失**，
  /// 因为「我到底重启了没有」这件事只有用户自己能回答。
  final String? restartNotice;

  /// 关掉上面那条提示。
  final VoidCallback? onDismissRestart;

  /// 当前活动会话 id（L1 会话绑定）：透传给各 Mod 产品面板。
  final String? activeSessionId;

  /// 面板动作成功后的统一通知（宿主据此弹重启提示）。
  final ValueChanged<String>? onModChanged;
  final Future<void> Function(String id, bool enabled)? onToggle;

  /// 保存某个 Mod 的配置（`POST /api/v1/mods/{id}/config`）。
  ///
  /// 为 null 时展开的表单只读展现、保存按钮禁用——**不假装能保存**。
  final Future<ModConfigResult> Function(
    String id,
    Map<String, Object?> config,
  )?
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
          description:
              '扩展能力走 Mod 边界隔离，默认全部停用。'
              '核心只提供接口，不把功能堆进来。',
        ),
        // L1 基座：统一的「需重新点火 / 重启后生效」提示。常驻在本分区顶部，
        // 直到用户主动关掉——它回答的是「我重启了没有」，只有用户知道答案。
        if (restartNotice != null)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s2),
            child: InlineNotice(
              message: restartNotice!,
              severity: NoticeSeverity.warning,
              onDismiss: onDismissRestart,
            ),
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
                // **按 id 认身份**：列表顺序变了（他处启停 / 重载后服务端换了
                // 顺序）时，Element 复用会把 A 的草稿画到 B 身上。
                key: ValueKey<String>(m.id),
                mod: m,
                busy: busyId != null,
                onToggle: onToggle,
                onSaveConfig: onSaveConfig,
                onLoadState: onLoadState,
                activeSessionId: activeSessionId,
                onModChanged: onModChanged,
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

/// 两个 config 的**内容**是否相同（键集 + 值递归比较）。
///
/// 用内容而不是 `identical` 是 F-0003-6 的核心：宿主保存后会 `_loadAdmin()`
/// 重取列表，服务端每次回的都是**新的 Map 实例**、内容却可能一字不差——
/// 按对象身份判断就等于「每次后台刷新都把用户正在填的草稿重灌一遍」。
///
/// 值可能嵌套（`settings_spec` 允许对象/数组形态的默认值），所以浅比较不够：
/// 同一个嵌套对象重新解析出来也是新实例。
bool _sameConfig(Map<String, Object?> a, Map<String, Object?> b) {
  if (a.length != b.length) return false;
  for (final MapEntry<String, Object?> e in a.entries) {
    if (!b.containsKey(e.key)) return false;
    if (!_sameValue(e.value, b[e.key])) return false;
  }
  return true;
}

bool _sameValue(Object? a, Object? b) {
  if (identical(a, b)) return true;
  if (a is Map && b is Map) {
    if (a.length != b.length) return false;
    for (final Object? k in a.keys) {
      if (!b.containsKey(k)) return false;
      if (!_sameValue(a[k], b[k])) return false;
    }
    return true;
  }
  if (a is List && b is List) {
    if (a.length != b.length) return false;
    for (int i = 0; i < a.length; i++) {
      if (!_sameValue(a[i], b[i])) return false;
    }
    return true;
  }
  return a == b;
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
    this.activeSessionId,
    this.onModChanged,
    super.key,
  });

  final ModInfo mod;
  final bool busy;
  final Future<void> Function(String id, bool enabled)? onToggle;
  final Future<ModConfigResult> Function(String id, Map<String, Object?> config)?
      onSaveConfig;
  final ModStateLoader? onLoadState;
  final String? activeSessionId;
  final ValueChanged<String>? onModChanged;

  @override
  State<_ModConfigTile> createState() => _ModConfigTileState();
}

class _ModConfigTileState extends State<_ModConfigTile> {
  /// 每个字段的当前值：服务端 config 优先，缺该键回落 spec 默认值。
  late Map<String, Object?> _values = _initialValues();

  /// **用户改过、但还没有成功保存**的字段键（F-0003-6）。
  ///
  /// 它是「草稿」与「服务端值」的分界线：外部 `config` 变化时这些键
  /// 一律不动——用户正在输入的东西不能被一次后台重取抹掉。
  final Set<String> _edited = <String>{};

  /// 上一次对齐过的服务端 config 的**内容快照**（不是对象身份，见
  /// [didUpdateWidget]）。宿主每保存一次都会重取列表，服务端回的是新
  /// `Map` 实例；按身份比较 = 每次刷新都把草稿重灌。
  Map<String, Object?> _baseline = const <String, Object?>{};

  /// PageStorage 里的草稿只恢复一次（在 `didChangeDependencies`，首帧之前）。
  bool _draftRestored = false;

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

  /// 面板声明的「高级」key（没面板 / 没声明 → 空集，全部平铺）。
  Set<String> get _advancedKeys =>
      modPanelFor(widget.mod.id)?.advancedKeys ?? const <String>{};

  List<ModSettingField> get _plainFields => <ModSettingField>[
    for (final ModSettingField f in _spec.fields)
      if (!_advancedKeys.contains(f.key)) f,
  ];

  List<ModSettingField> get _advancedFields => <ModSettingField>[
    for (final ModSettingField f in _spec.fields)
      if (_advancedKeys.contains(f.key)) f,
  ];

  /// 「高级」折叠：默认收起。旧的平铺观感只在**没有**高级字段时保持。
  Widget _advancedBlock() => ExpansionTile(
    tilePadding: EdgeInsets.zero,
    childrenPadding: const EdgeInsets.only(left: Space.s2),
    expandedCrossAxisAlignment: CrossAxisAlignment.stretch,
    title: Text('高级', style: Theme.of(context).textTheme.titleSmall),
    children: <Widget>[
      for (final ModSettingField f in _advancedFields) _field(f),
    ],
  );

  /// 服务端 config 里该字段的值（缺该键 → spec 默认值）。
  Object? _remoteValue(ModSettingField f) => widget.mod.config.containsKey(f.key)
      ? widget.mod.config[f.key]
      : f.defaultValue;

  Map<String, Object?> _initialValues() {
    final Map<String, Object?> out = <String, Object?>{};
    for (final ModSettingField f in _spec.fields) {
      out[f.key] = _remoteValue(f);
    }
    return out;
  }

  // ── 草稿的跨「离开分区」存活（F-0003-6 后半） ─────────────────────
  //
  // 分区切换会把整个 pane 子树卸掉（`shell_settings.dart` 的 `switch` 每次
  // 建新的 pane 类型）⇒ 卡片 State 连同草稿、连同带错误码的保存结果一起
  // 消失。而 `PageStorage` 是**路由级**的存储桶（每个 `ModalRoute` 自带一个，
  // 见 `routes.dart`），子树死了它还在——正是「切走再切回来」要的东西。
  //
  // 为什么不用一个模块级全局缓存：那会把测试之间、甚至同一进程里多个
  // 宿主之间的草稿串起来（现有测试里同一个 mod id 被反复泵），而路由桶
  // 天然是「一条界面的生命周期」，天然隔离。
  static String _draftId(String modId) => 'mod-config-draft/$modId';

  /// host 保存失败时留在屏幕上的那些错误码文案，也要跟着草稿一起活下来
  /// （用户拿码去日志里搜——码不能因为切了个分区就没了）。
  void _persistDraft() {
    final PageStorageBucket? bucket = PageStorage.maybeOf(context);
    if (bucket == null) return;
    // **密钥字段不进存储桶**：草稿只住这条路由的内存里，但「secret 值不写
    // 任何持久化面」是本项目的一贯口径（`.env` 是唯一真源），不为一次分区
    // 切换破例——密钥草稿在离开分区后按「未填」处理（仍可不修改地保存）。
    final Set<String> secret = <String>{
      for (final ModSettingField f in _spec.fields)
        if (f.kind == ModFieldKind.string && f.secret) f.key,
    };
    bucket.writeState(
      context,
      <String, Object?>{
        'values': <String, Object?>{
          for (final MapEntry<String, Object?> e in _values.entries)
            if (!secret.contains(e.key)) e.key: e.value,
        },
        'edited': _edited
            .where((String k) => !secret.contains(k))
            .toList(growable: false),
        'message': _message,
        'messageIsError': _messageIsError,
      },
      identifier: _draftId(widget.mod.id),
    );
  }

  void _restoreDraft() {
    final PageStorageBucket? bucket = PageStorage.maybeOf(context);
    if (bucket == null) return;
    final Object? raw = bucket.readState(
      context,
      identifier: _draftId(widget.mod.id),
    );
    if (raw is! Map) return;
    final Object? rawValues = raw['values'];
    final Object? rawEdited = raw['edited'];
    if (rawValues is! Map || rawEdited is! List) return;

    final Set<String> specKeys = <String>{
      for (final ModSettingField f in _spec.fields) f.key,
    };
    // 从**当前** spec 出发：spec 变过之后，已经删掉的字段不许从旧草稿里
    // 复活（`_buildConfig` 只发 spec 里的字段，留着只会让人误以为它还在）。
    final Map<String, Object?> restored = _initialValues();
    for (final ModSettingField f in _spec.fields) {
      if (rawValues.containsKey(f.key)) restored[f.key] = rawValues[f.key];
    }
    _values = restored;
    _edited
      ..clear()
      ..addAll(<String>[
        for (final Object? k in rawEdited)
          if (k is String && specKeys.contains(k)) k,
      ]);
    final Object? message = raw['message'];
    if (message is String) {
      _message = message;
      _messageIsError = raw['messageIsError'] == true;
    }
  }

  @override
  void initState() {
    super.initState();
    _baseline = Map<String, Object?>.of(widget.mod.config);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_draftRestored) return;
    _draftRestored = true;
    _restoreDraft();
  }

  @override
  void didUpdateWidget(_ModConfigTile oldWidget) {
    super.didUpdateWidget(oldWidget);
    // 列表重排 / 换数据源时 Element 可能被复用给**另一个** Mod（草稿会跟着
    // 串到别人身上）。`ModsSection` 现在按 id 挂了 ValueKey，正常不会走到
    // 这里；留下这段是兜底：换 id = 整份重建，不迁移草稿。
    if (oldWidget.mod.id != widget.mod.id) {
      _edited.clear();
      _message = null;
      _messageIsError = false;
      _baseline = Map<String, Object?>.of(widget.mod.config);
      _values = _initialValues();
      _restoreDraft();
      return;
    }
    final Map<String, Object?> remote = widget.mod.config;
    if (_sameConfig(_baseline, remote)) {
      // **新对象、同内容** = 只是一次后台重取（宿主保存后 `_loadAdmin()`、
      // 他处刷新）。一律不动草稿——旧的 `identical` 判断正是在这里把用户
      // 正在输入的东西整份重灌回服务端值（F-0003-6 的主症状）。
      _baseline = Map<String, Object?>.of(remote);
      return;
    }
    _reconcile(remote);
  }

  /// 服务端 config **内容真的变了** 时的对齐（F-0003-6）。
  ///
  /// - 没有未提交草稿 → 服务端是唯一真相，整份重灌（与改动前一致）；
  /// - 有未提交草稿 → **只采用用户没碰过的字段**，草稿字段原样保留。
  ///   「保存失败后宿主又重取了一次列表」不该把用户刚填的内容吃掉。
  void _reconcile(Map<String, Object?> remote) {
    if (_edited.isEmpty) {
      _values = _initialValues();
    } else {
      for (final ModSettingField f in _spec.fields) {
        if (_edited.contains(f.key)) continue;
        _values[f.key] = _remoteValue(f);
      }
    }
    _baseline = Map<String, Object?>.of(remote);
    _persistDraft();
  }

  /// 改一个字段 = 记一笔「未提交草稿」，并落进路由桶（切分区也还在）。
  void _edit(String key, Object? value) {
    setState(() {
      _values[key] = value;
      _edited.add(key);
    });
    _persistDraft();
  }

  bool _boolValue(ModSettingField f) =>
      _values[f.key] is bool ? _values[f.key]! as bool : f.defaultValue == true;

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
    final Map<String, Object?> next = Map<String, Object?>.of(
      widget.mod.config,
    );
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
      final Map<String, Object?> before = _baseline;
      final ModConfigResult result = await save(widget.mod.id, _buildConfig());
      if (!mounted) return;
      if (result.ok) {
        // 保存成功 = 草稿已经落到服务端：清掉「未提交」标记，此后的外部
        // config 变化按服务端真相回填。
        //
        // 但**不能无条件**用当前 remote 重灌：宿主可能根本没重取列表
        // （保存回调只回了个 ok），那样会把用户刚刚保存的值从表单里抹掉
        // ——「保存结果离开分区即丢」有一半是这么来的。判据是「保存期间
        // 服务端 config 的内容真的变了」：变了（= 宿主 `_loadAdmin()` 拿回
        // 归一化后的值）就采用它，没变就保留刚提交的那些值。
        final bool remoteChanged = !_sameConfig(before, widget.mod.config);
        _edited.clear();
        if (remoteChanged) _values = _initialValues();
        _baseline = Map<String, Object?>.of(widget.mod.config);
      }
      setState(() {
        _saving = false;
        _messageIsError = !result.ok;
        _message = result.ok ? _okMessage(result) : '保存失败：服务端返回 ok=false';
      });
      // 草稿与保存结果都进路由桶：切走分区再进来，值和文案都还在
      // （F-0003-6 后半）。
      _persistDraft();
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
      // 失败时**草稿不丢**（`_edited` 原样留着）：宿主紧接着重取列表也会
      // 被 `_reconcile` 挡在草稿之外；带码的文案同样进桶，切走再回来还能
      // 拿这个码去日志里搜。
      _persistDraft();
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _messageIsError = true;
        _message = '保存失败：$e';
      });
      _persistDraft();
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
        // 面板声明「收进高级」的字段（见 ModPanel.advancedKeys）单独折叠；
        // 其余平铺——旧 Mod 没声明 → 与改动前逐字一致。
        for (final ModSettingField f in _plainFields) _field(f),
        if (_advancedFields.isNotEmpty) _advancedBlock(),
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
    // 面板自己把运行态讲得更清楚时（导演），通用块整块不渲染——避免
    // 「一个键一行」把面板淹没（用户裁决：主面板只留必要信息）。
    if (modPanelFor(widget.mod.id)?.showRuntimeState == false) {
      return const SizedBox.shrink();
    }
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
        // L1 会话绑定 + 统一重启提示：面板拿到活动会话与变更通知回调。
        activeSessionId: widget.activeSessionId,
        onModChanged: widget.onModChanged,
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
          onChanged: (bool v) => _edit(f.key, v),
        );
      case ModFieldKind.string:
        return TextFieldRow(
          label: f.label,
          icon: f.secret ? Icons.key_outlined : Icons.text_fields,
          value: _stringValue(f),
          obscure: f.secret,
          description: f.secret ? '留空表示不修改（服务端不回传密钥）' : null,
          onChanged: (String v) => _edit(f.key, v),
        );
      case ModFieldKind.number:
        return NumberField(
          label: f.label,
          icon: Icons.tag,
          value: _intValue(f),
          // 尊重 spec 的 min/max：`NumberField` 对越界输入**不回调**。
          min: f.min?.toInt(),
          max: f.max?.toInt(),
          onChanged: (int v) => _edit(f.key, v),
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
          onChanged: (String v) => _edit(f.key, v),
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
        ReadonlyField(label: '实时通道', icon: Icons.cable, text: wsStatusLabel),
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
          text:
              '${capabilities.app} ${capabilities.version}'
              '（schema v${capabilities.schemaVersion}，WS 协议 v${capabilities.wsProtocolVersion}）',
        ),
        ReadonlyField(
          label: '服务端音频后端',
          icon: Icons.speaker_outlined,
          text:
              '${(status['audio'] as Map<String, Object?>?)?['backend'] ?? '未知'}'
              '（sample_rate=${(status['audio'] as Map<String, Object?>?)?['sample_rate'] ?? '?'}）',
          description: 'Web 模式下音频单源是**浏览器**，服务端没有声卡是预期的',
        ),
        ReadonlyField(
          label: 'LLM / TTS 配置',
          icon: Icons.hub_outlined,
          text:
              'LLM: ${(llm as Map<String, Object?>?)?['model'] ?? '未配置'}'
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
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.contentMuted,
            ),
          )
        else if (logs.isEmpty)
          Text(
            '（暂无日志）',
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.contentMuted,
            ),
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
    this.onApplyPreset,
    this.presetStatus,
    this.productScales,
    this.pinnedScales,
    this.onApplyScales,
    this.onClearScales,
    this.presetLabels = PresetLabelTable.empty,
    this.clock,
    super.key,
  });

  final bool devMode;
  final ValueChanged<bool> onDevModeChanged;

  /// **动作调试**（P0-3）：直发一条预设到渲染面（`none` = 归零）。
  ///
  /// 回调带 `intensity`（渲染面钳 `[0,3]`）：调试面板的滑条直接透传，
  /// 缺省 [kDefaultPresetIntensity]（1.2，肉眼明显）。
  final PresetApply? onApplyPreset;

  /// 舞台的本地预设状态（`Live2DStageState.presetStatus`）。
  final ValueListenable<PresetStatus?>? presetStatus;

  /// 服务端**产品设置**里的动作幅度（显示 + 作为「恢复」目标）。
  final ActionSettingsView? productScales;

  /// 当前**临时幅度覆盖**（ActionScalesSyncer.pinned；null = 没有）。
  ///
  /// 只读传入、**不落盘**。面板滑条按 `pinnedScales ?? productScales` 播种：
  /// 临时覆盖生效期间离开 Developer 分区再回来，滑条必须还是渲染面正在用的
  /// 那组临时值，而不是产品值——否则面板与舞台 / HUD 各说各话（T4 实测）。
  final Map<String, double>? pinnedScales;

  /// 临时幅度覆盖（**不落盘**，只发渲染面）；产品设置才是真源。
  final PresetScaleApply? onApplyScales;

  /// 清掉临时幅度覆盖并**强制**写回产品值（W7：面板上的「恢复产品设置」）。
  final VoidCallback? onClearScales;

  /// 预设 id → 中文展示名（读 `assets/actions/preset_labels.json`）。
  ///
  /// 缺省空表 = 显示稳定 id（**不手写第二套中文标签**）。
  final PresetLabelTable presetLabels;

  /// 可注入时钟（表情到点计时用）；null = [DateTime.now]。
  ///
  /// **不在 widget 里写死 `DateTime.now()`**：flutter test 传一个假时钟就能
  /// 把「到点后 UI 回到『无（已到点）』、点手势不再重发」钉死（W3 验收）。
  final DebugClock? clock;

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
        // P0-3：动作调试**只在开发模式显示**（产品面不放调试按钮）。
        if (devMode) ...<Widget>[
          const SizedBox(height: Space.s4),
          DebugPanels(
            onApplyPreset: onApplyPreset,
            status: presetStatus,
            productScales: productScales,
            pinnedScales: pinnedScales,
            onApplyScales: onApplyScales,
            onClearScales: onClearScales,
            labels: presetLabels,
            clock: clock,
          ),
          // ── 导演可观测（阶段5 W5a，D40–D43）────────────────────────────
          // 四栏只读观测（A 决策参数 / B 事件流 / C 传参对照 / D 送 TTS 文本）。
          // **只在 devMode 下渲染**——off 时整块不在语义树（本 if 块一起消失）。
          // 数据经单例 DirectorObserverFeed 注入（不改 shell_settings.dart）。
          const DirectorObserverSection(),
        ],
      ],
    );
  }
}

/// 「动作调试」下发一条预设：id + 强度（none = 归零，强度被忽略）。
typedef PresetApply = void Function(String id, double intensity);

/// 临时覆盖三项幅度倍率（head / body / expression）。
typedef PresetScaleApply = void Function(double head, double body, double expression);

/// 可注入时钟（表情到点计时用；不要在 widget 里写死 `DateTime.now()`）。
typedef DebugClock = DateTime Function();

// 表情的**到点时刻不在这里写死**：权威值在渲染面 preset 的 `duration_ms`
// （表情 2600ms；`crates/l2d-wasm-demo/src/preset/mod.rs` 的 `EXPRESSION_MS`），
// 经 `live2d_stage.dart` 的显示用 ttl 投影到 [PresetStatus.ttl]。面板只消费
// 舞台那个 `PresetStatus` 通知器——**不在 Dart 里再抄一份时长**（RESEARCH §4 E7；
// 也避开 design_tokens_lint 的「协议时长只能走豁免路径」红线）。

/// 「全部扫一遍」的步进间隔（每条演一小会儿，肉眼能分清）。
///
/// 取 UI 动效 4 档里的 [AppDurations.reveal]（600ms）：**不新增裸时长**——
/// 门禁 design_tokens_lint_test 只允许 UI 时长来自那 4 档。
const Duration kDebugSweepInterval = AppDurations.reveal;

/// 开发模式的**两块调试**（2026-09-23 从「万能动作调试」拆开）。
///
/// - **表情调试**：只列 `channel=expression` 的包（**标签表派生**），点按只发
///   Face 槽；独立滑条「基础表情强度」（缺省 [kDefaultExpressionIntensity] = 1.0）。
/// - **动作调试**：只列 `channel=motion` 的包，滑条「动作强度」
///   （缺省 [kDefaultPresetIntensity] = 1.2）；另有「叠加基础表情」开关
///   （**缺省关**，2026-09-21 W3）——开着且基础表情**仍在有效期内**时，播放手势
///   会先把这份表情写进 Face 槽，再写手势（Face → Gesture 与渲染面同帧写入顺序
///   一致，共享通道由手势赢）。
///
/// 两块共享同一份「基础表情」状态，所以「动作扫一遍」自动带上当前基础表情。
/// 基础表情按渲染面同口径（[PresetStatus.ttl]，表情 2600ms）**到点即视为 none**：
/// UI 显示「无（已到点）」，叠加与重发都不再认它。真正的执行者仍是渲染面的双槽
/// 状态机（表情 2600ms / 手势 900–1000ms，各按 ttl 撤销）。
class DebugPanels extends StatefulWidget {
  const DebugPanels({
    this.onApplyPreset,
    this.status,
    this.productScales,
    this.pinnedScales,
    this.onApplyScales,
    this.onClearScales,
    this.labels = PresetLabelTable.empty,
    this.clock,
    super.key,
  });

  /// 直发一条预设（none = 归零）。为空时按钮禁用（不假装能点）。
  final PresetApply? onApplyPreset;

  /// 舞台的本地状态（Live2DStageState.presetStatus）。
  final ValueListenable<PresetStatus?>? status;

  /// 产品设置里的动作幅度（显示 + 「恢复」目标）；null = 未加载。
  final ActionSettingsView? productScales;

  /// 当前临时覆盖（ActionScalesSyncer.pinned）；null = 没有。
  ///
  /// 滑条初值 = `pinnedScales ?? productScales`，所以临时覆盖期间离开分区
  /// 再回来，面板显示的仍是渲染面正在用的那组值（T4 实测症状的根因）。
  final Map<String, double>? pinnedScales;

  /// 临时覆盖（不落盘）；null 时滑条禁用。
  final PresetScaleApply? onApplyScales;

  /// 清掉临时覆盖并写回产品值（null 时「恢复产品设置」禁用）。
  final VoidCallback? onClearScales;

  /// 预设 id → 展示名（读共享标签表；空表回落到 id）。
  final PresetLabelTable labels;

  /// 可注入时钟（表情到点计时）；null = [DateTime.now]。
  final DebugClock? clock;

  @override
  State<DebugPanels> createState() => _DebugPanelsState();
}

class _DebugPanelsState extends State<DebugPanels> {
  /// 当前「基础表情」包（面板**最近一次真正下发**的那条）；none = 没有。
  ///
  /// 它只记「最近点了哪条」；**是否还在演**由 [_expressionLive] 用注入时钟 +
  /// 渲染面回执的 [PresetStatus.ttl] 判定——到点后这里仍留着 id 供文案说
  /// 「已到点」，但行为上一律按 none 对待（不叠加、不重发）。
  String _expressionId = 'none';

  /// 基础表情的**到点时刻**；null = 还没拿到渲染面回执（或不生效）。
  DateTime? _expressionExpiresAt;

  /// 已经消费过的 [PresetStatus.startedAt]，避免舞台 200ms 一次的等价刷新
  /// 把到点时刻反复往后推。
  DateTime? _statusStartConsumed;

  /// 基础表情到点时触发一次重建（UI 从「某条」切到「无（已到点）」）。
  Timer? _faceExpiryTimer;

  /// 基础表情强度：表情调试用它，动作调试「叠加基础表情」也用它。
  double _faceIntensity = kDefaultExpressionIntensity;

  /// 手势强度。
  double _actionIntensity = kDefaultPresetIntensity;

  /// 播放手势时是否先把基础表情写进 Face 槽。
  ///
  /// **缺省 false**（2026-09-21 W3，症状①）：调试面板不再默认替用户接管表情槽，
  /// 否则「点手势总带着上一张老脸」。开关仍在、语义不变，只是要用户显式打开。
  bool _overlayFace = false;

  Timer? _faceSweepTimer;
  Timer? _gestureSweepTimer;
  int _faceSweepIndex = 0;
  int _gestureSweepIndex = 0;

  /// 时钟（**可注入**；测试用假时钟钉住到点行为）。
  DateTime _now() => (widget.clock ?? DateTime.now)();

  /// 表情 / 手势按钮清单的**唯一来源**：标签表的 `channel` 字段（空表 → 空列表）。
  List<String> get _expressionIds => widget.labels.idsForChannel('expression');
  List<String> get _gestureIds => widget.labels.idsForChannel('motion');

  /// 「当前基础表情」是否仍在演（到点时刻 = 渲染面回执的 [PresetStatus.ttl]）。
  ///
  /// 拿不到回执（`_expressionExpiresAt == null`）时**不算在演**：没有渲染面
  /// 权威 ttl 就不假装知道它演多久——这也保证「到点后不重发」是真的。
  bool get _expressionLive {
    final DateTime? until = _expressionExpiresAt;
    return _expressionId != 'none' && until != null && _now().isBefore(until);
  }

  /// 是否已到点（到点后 UI 说「无（已到点）」、叠加不再认它）。
  bool get _expressionExpired {
    final DateTime? until = _expressionExpiresAt;
    return _expressionId != 'none' && until != null && !_now().isBefore(until);
  }

  /// 叠加 / 展示用的**有效**表情 id：到点后就是 none（不叠加、不重发）。
  String get _liveExpressionId => _expressionLive ? _expressionId : 'none';

  late double _head;
  late double _body;
  late double _expression;

  bool get _enabled => widget.onApplyPreset != null;
  bool get _scalesEnabled => widget.onApplyScales != null;
  bool get _clearEnabled => widget.onClearScales != null;

  /// 是否有临时覆盖：真源 = 宿主只读传进来的 [DebugPanels.pinnedScales]。
  bool get _hasPin => widget.pinnedScales != null;

  @override
  void initState() {
    super.initState();
    _syncScalesFromEffective();
    widget.status?.addListener(_onStatusChanged);
  }

  @override
  void didUpdateWidget(DebugPanels oldWidget) {
    super.didUpdateWidget(oldWidget);
    // pinnedScales 变化也要重新播种：宿主清了临时覆盖（或换了一组临时值）时，
    // 滑条必须跟着回到渲染面的有效值，而不是停在上一组。
    if (widget.productScales != oldWidget.productScales ||
        widget.pinnedScales != oldWidget.pinnedScales) {
      _syncScalesFromEffective();
    }
    // 舞台重建会换一个 presetStatus 通知器：跟着换监听，别听死那个旧的。
    if (widget.status != oldWidget.status) {
      oldWidget.status?.removeListener(_onStatusChanged);
      widget.status?.addListener(_onStatusChanged);
    }
  }

  /// 滑条初值 = **渲染面当前有效值**：临时覆盖优先，否则产品值。
  ///
  /// 不能只看产品值：临时覆盖生效期间离开分区再回来，滑条会掉回产品值，
  /// 与舞台 / HUD 上真正生效的临时值分叉（T4 实测症状）。
  void _syncScalesFromEffective() => _seedScales(widget.pinnedScales);

  /// 播下三项滑条值：`pinned` 非空就用它，否则用产品值 / 出厂默认。
  void _seedScales(Map<String, double>? pinned) {
    final ActionSettingsView? p = widget.productScales;
    _head =
        pinned?['head'] ?? p?.headScale ?? ActionSettingsView.defaultHeadScale;
    _body =
        pinned?['body'] ?? p?.bodyScale ?? ActionSettingsView.defaultBodyScale;
    _expression = pinned?['expression'] ??
        p?.expressionScale ??
        ActionSettingsView.defaultExpressionScale;
  }

  void _applyScales() => widget.onApplyScales?.call(_head, _body, _expression);

  /// 「恢复产品设置」：滑条回到产品值，并让宿主**清掉临时覆盖**
  /// （不能走 [_applyScales]——那会把当前临时值再钉一次）。
  ///
  /// 这里**只**按产品值播种（`_seedScales(null)`）：宿主清 pin 与随后的重建
  /// 排在这一帧之后，浮层宿主（medium 底部浮层）甚至不会因这次点击重建——
  /// 若仍读旧 pin，滑条会停在刚被清掉的那组临时值上。
  void _resetScales() {
    setState(() => _seedScales(null));
    widget.onClearScales?.call();
  }

  @override
  void dispose() {
    widget.status?.removeListener(_onStatusChanged);
    _faceSweepTimer?.cancel();
    _gestureSweepTimer?.cancel();
    _faceExpiryTimer?.cancel();
    super.dispose();
  }

  /// 只发 Face 槽；none = 两槽同清（渲染面协议）。
  void _applyExpression(String id) {
    widget.onApplyPreset?.call(id, id == 'none' ? 0 : _faceIntensity);
  }

  /// 记录「面板刚把哪条基础表情发下去」（none = 清掉计时）。
  ///
  /// **不在这里写死 ttl**：到点时刻等渲染面回执 [PresetStatus.ttl]（权威）来定，
  /// 见 [_onStatusChanged]。**到点只是重建 UI**：[_expressionId] 保留最后一条
  /// 供文案说「已到点」，真正的有效判据始终是 [_expressionLive]（读注入时钟），
  /// 所以即使定时器晚到、甚至测试里根本不推进定时器，只要时钟过了到点时刻就
  /// 不会再重发。
  void _setBaseExpression(String id) {
    _faceExpiryTimer?.cancel();
    _faceExpiryTimer = null;
    _statusStartConsumed = null;
    setState(() {
      _expressionId = id;
      _expressionExpiresAt = null;
    });
  }

  /// 舞台回执驱动到点时刻：只认**与当前基础表情同 id** 的那条 [PresetStatus]，
  /// 用它的 [PresetStatus.ttl]（渲染面 `duration_ms` 的投影）起算。
  ///
  /// 用 [PresetStatus.startedAt] 去重：舞台每 200ms 重发一个等价对象，不能让它
  /// 把到点时刻一路往后推——那正是「老表情续期」的翻版。
  void _onStatusChanged() {
    final PresetStatus? s = widget.status?.value;
    if (s == null || _expressionId == 'none' || s.id != _expressionId) return;
    if (_statusStartConsumed == s.startedAt) return;
    _statusStartConsumed = s.startedAt;
    _armExpressionExpiry(_now().add(s.ttl));
  }

  /// 按 [until] 定下到点时刻并排一个一次性重建定时器（到点后不重发）。
  void _armExpressionExpiry(DateTime until) {
    _faceExpiryTimer?.cancel();
    final Duration left = until.difference(_now());
    _faceExpiryTimer = Timer(left.isNegative ? Duration.zero : left, () {
      _faceExpiryTimer = null;
      if (mounted) setState(() {});
    });
    if (mounted) setState(() => _expressionExpiresAt = until);
  }

  /// 发手势：**先** Face（可选叠加）再 Gesture。
  ///
  /// 叠加基础表情（W3 症状①修复）：叠加的输入只认「**仍然有效**」的那条表情
  /// [_liveExpressionId]——它按渲染面 preset 的 duration_ms（表情 2600ms）到点，
  /// 到点后就是 none。于是：
  ///
  /// - 点手势**不会**把一张演完的老脸重新点亮 / 续期——过期即不发；
  /// - 表情要再看一次，只能由用户重新点那条表情按钮（那才是「发一条新表情」，
  ///   会重新起算它自己的 ttl），手势绝不承担「把旧表情重发一遍来保活」的角色。
  ///
  /// 手势本身始终只是一条 Gesture 槽指令，不带任何表情状态。
  void _applyGesture(String id) {
    final String face = _liveExpressionId;
    if (_overlayFace && face != 'none') {
      widget.onApplyPreset?.call(face, _faceIntensity);
    }
    widget.onApplyPreset?.call(id, _actionIntensity);
  }

  /// 表情扫一遍：只扫表情包；**不**在结尾送 none（免得误清正在演的手势槽），
  /// 最后一条按自己的表情 ttl 到点撤销。
  void _sweepExpressions() {
    final List<String> ids = _expressionIds;
    _faceSweepTimer?.cancel();
    _faceSweepIndex = 0;
    _faceSweepTimer = Timer.periodic(kDebugSweepInterval, (Timer t) {
      if (_faceSweepIndex >= ids.length) {
        t.cancel();
        _faceSweepTimer = null;
        if (mounted) setState(() {});
        return;
      }
      _applyExpression(ids[_faceSweepIndex]);
      if (mounted) setState(() => _faceSweepIndex++);
    });
  }

  /// 手势扫一遍：每一步都带当前基础表情（若开关开着且仍在有效期内）。
  void _sweepGestures() {
    final List<String> ids = _gestureIds;
    _gestureSweepTimer?.cancel();
    _gestureSweepIndex = 0;
    _gestureSweepTimer = Timer.periodic(kDebugSweepInterval, (Timer t) {
      if (_gestureSweepIndex >= ids.length) {
        t.cancel();
        _gestureSweepTimer = null;
        if (mounted) setState(() {});
        return;
      }
      _applyGesture(ids[_gestureSweepIndex]);
      if (mounted) setState(() => _gestureSweepIndex++);
    });
  }

  /// 取不到标签表（或表里没有该 channel）时**如实说明**，而不是静默渲染一个
  /// 空按钮组——「空按钮组」会被读成「这个功能不存在」，与事实相反。
  Widget _missingPresetNotice(String channel, String kind) {
    final String why = widget.labels.length == 0
        ? '预设标签表未加载（GET /actions/preset_labels.json 取不到）'
        : '预设标签表里没有 channel=$channel 的条目';
    return InlineNotice(
      message:
          '$why，因此不列出$kind按钮——这不是「没有$kind」，'
          '请检查静态路由后刷新页面。',
      severity: NoticeSeverity.warning,
      dense: true,
    );
  }

  String _statusLine(PresetStatus? s) {
    if (s == null) return '当前没有预设（已归零 / 已到点）';
    final Duration left = s.remaining(_now());
    return '最近触发：${s.id} · 来源 ${s.source} · 约剩 ${left.inMilliseconds} ms';
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final TextStyle? muted = theme.textTheme.bodySmall?.copyWith(
      color: appColorsOf(context).contentMuted,
    );
    final bool sweepingFace = _faceSweepTimer != null;
    final bool sweepingGesture = _gestureSweepTimer != null;
    final List<String> expressionIds = _expressionIds;
    final List<String> gestureIds = _gestureIds;
    // 到点后**不再声称在演**（无（已到点））；还没拿到渲染面回执时按「刚下发」
    // 显示，不谎报「已到点」。
    final String baseExpression = _expressionId == 'none'
        ? '无（none）'
        : (_expressionExpired
              ? '无（已到点）'
              : widget.labels.display(_expressionId));
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // ── 表情调试（只发 Face 槽） ──
        SectionHeader(
          title: '表情调试',
          description: '只发 Face 槽：点一条表情包即可确认基础表情强度'
              '（约 2.6s 保持，到点自动撤）。'
              '这里只列标签表里 channel=expression 的包，不混手势。',
        ),
        SliderField(
          label: '基础表情强度',
          icon: Icons.mood,
          value: _faceIntensity,
          min: 0.5,
          max: 3.0,
          divisions: 10,
          percentage: false,
          suffix: ' 倍',
          enabled: _enabled,
          onChanged: (double v) => setState(() => _faceIntensity = v),
        ),
        if (expressionIds.isEmpty)
          _missingPresetNotice('expression', '表情')
        else
          Wrap(
            spacing: Space.s1,
            runSpacing: Space.s1,
            children: <Widget>[
              for (final String id in expressionIds)
                OutlinedButton(
                  onPressed: _enabled
                      ? () {
                          _setBaseExpression(id);
                          _applyExpression(id);
                        }
                      : null,
                  child: Text(widget.labels.display(id)),
                ),
            ],
          ),
        const SizedBox(height: Space.s1),
        Row(
          children: <Widget>[
            TextButton(
              onPressed: _enabled && !sweepingFace && expressionIds.isNotEmpty
                  ? _sweepExpressions
                  : null,
              child: Text(
                sweepingFace
                    ? '表情扫一遍中…（第 $_faceSweepIndex 条）'
                    : '表情扫一遍',
              ),
            ),
            const SizedBox(width: Space.s1),
            TextButton(
              // 语义与 W4 无关：这里只是「两槽同清」的调试入口，原样保留。
              onPressed: _enabled
                  ? () {
                      _setBaseExpression('none');
                      _applyExpression('none');
                    }
                  : null,
              child: const Text('归零（none，两槽同清）'),
            ),
          ],
        ),
        const Divider(),
        // ── 动作调试（只发 Gesture 槽，可叠加基础表情） ──
        const SectionHeader(
          title: '动作调试',
          description: '只发 Gesture 槽：手势约 0.9–1.0s 起落（标签表里 '
              'channel=motion 的包），带动半身，摇头是左右多周期。',
        ),
        SliderField(
          label: '动作强度',
          icon: Icons.face_retouching_natural,
          value: _actionIntensity,
          min: 0.5,
          max: 3.0,
          divisions: 10,
          percentage: false,
          suffix: ' 倍',
          enabled: _enabled,
          onChanged: (double v) => setState(() => _actionIntensity = v),
        ),
        ToggleField(
          label: '叠加基础表情',
          icon: Icons.layers_outlined,
          value: _overlayFace,
          enabled: _enabled,
          description: '当前基础表情：$baseExpression，强度 '
              '${_faceIntensity.toStringAsFixed(1)} 倍。'
              '**缺省关**；打开后，只要基础表情**仍在有效期内**，'
              '每次点手势都会把当前基础表情**重新起算**（2.6s 重新计时）——'
              '到点后不再叠加，也不会重发。',
          onChanged: (bool v) => setState(() => _overlayFace = v),
        ),
        if (gestureIds.isEmpty)
          _missingPresetNotice('motion', '手势')
        else
          Wrap(
            spacing: Space.s1,
            runSpacing: Space.s1,
            children: <Widget>[
              for (final String id in gestureIds)
                OutlinedButton(
                  onPressed: _enabled ? () => _applyGesture(id) : null,
                  child: Text(widget.labels.display(id)),
                ),
            ],
          ),
        const SizedBox(height: Space.s1),
        Row(
          children: <Widget>[
            TextButton(
              onPressed: _enabled && !sweepingGesture && gestureIds.isNotEmpty
                  ? _sweepGestures
                  : null,
              child: Text(
                sweepingGesture
                    ? '动作扫一遍中…（第 $_gestureSweepIndex 条）'
                    : '动作扫一遍',
              ),
            ),
            const SizedBox(width: Space.s1),
            TextButton(
              onPressed: _enabled ? () => _applyExpression('none') : null,
              child: const Text('归零（none，两槽同清）'),
            ),
          ],
        ),
        const SizedBox(height: Space.s1),
        // 当前状态：本地镜像（最近触发）＋指向渲染面 HUD 的权威行。
        if (widget.status == null)
          Text('当前状态：舞台未挂载，本地状态不可用（看渲染面 HUD 的 preset: 行）', style: muted)
        else
          ValueListenableBuilder<PresetStatus?>(
            valueListenable: widget.status!,
            builder: (BuildContext context, PresetStatus? s, Widget? _) =>
                Text('当前状态：${_statusLine(s)}', style: muted),
          ),
        const SizedBox(height: Space.s1),
        Text(
          'HUD 权威行形如 preset: face=<id> gesture=<id> face_intensity=<倍率> src=… ms；'
          '点了没反应 = 本皮套缺那条参数，渲染面静默 no-op。',
          style: muted,
        ),
        if (widget.productScales != null) ...[
          const Divider(),
          Text('临时幅度覆盖', style: theme.textTheme.labelLarge),
          const SizedBox(height: Space.s1),
          // 两态明示：有临时覆盖时把「面板滑条 = 渲染面有效值」说清，
          // 免得用户看到滑条与「外观与互动」里的产品值不同却不知道为什么。
          if (_hasPin) ...[
            EmphasizedText(
              '**临时覆盖生效中**（不落盘，点「恢复产品设置」清除）：'
              '下面三个滑条显示的就是渲染面当前生效的值，不是产品设置值。',
              style: muted,
            ),
            const SizedBox(height: Space.s1),
          ],
          EmphasizedText(
            '优先级（W7，2026-09-23 起）：**临时覆盖 > 草稿 > 磁盘值**。'
            '「应用到渲染面（临时）」后这份临时值一直生效（**不落盘**，'
            '刷新页面即失效）；拖动「外观与互动 → 动作幅度」的草稿、切换主题、'
            '调音量都**不会**再冲掉它——只有点「恢复产品设置」才清掉临时覆盖'
            '并把产品值写回舞台。',
            style: muted,
          ),
          SliderField(
            label: '临时 head',
            icon: Icons.face_retouching_natural,
            value: _head,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: _scalesEnabled,
            onChanged: (double v) => setState(() => _head = v),
          ),
          SliderField(
            label: '临时 body',
            icon: Icons.accessibility_new,
            value: _body,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: _scalesEnabled,
            onChanged: (double v) => setState(() => _body = v),
          ),
          SliderField(
            label: '临时 expression',
            icon: Icons.mood,
            value: _expression,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: _scalesEnabled,
            onChanged: (double v) => setState(() => _expression = v),
          ),
          Row(
            children: <Widget>[
              TextButton(
                onPressed: _scalesEnabled ? _applyScales : null,
                child: const Text('应用到渲染面（临时）'),
              ),
              const SizedBox(width: Space.s1),
              TextButton(
                onPressed: _clearEnabled ? _resetScales : null,
                child: const Text('恢复产品设置'),
              ),
            ],
          ),
        ],
      ],
    );
  }
}
