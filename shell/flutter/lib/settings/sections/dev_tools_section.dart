/// 「Mod」「诊断」「模型库」「开发模式」四个分区的实现。
///
/// 合在**一个库**里：它们共享同一套「列表 + 动作 + 内联结果」的骨架，
/// 拆成四个库只会让同一段列表渲染代码出现四遍。每个类都短、
/// 职责单一，找起来靠类名即可。
///
/// # 行数拆分（2026-10-06，R4-T2）
///
/// 原本单文件 **2109 行**。按**顶层类边界**拆成库 + 4 个 part（`part` 继承
/// 库的 import，零可见性改动）：
///
/// | 内容 | 去处 | 行数 |
/// | --- | --- | --- |
/// | `ModsSection` / `_ModConfigTile` | `dev_tools_mods.dart` | 213 |
/// | `_ModConfigTileState`（配置表单 + 运行态读取） | `dev_tools_mod_config.dart` | 627 |
/// | `DiagnosticsSnapshot` / `DiagnosticsSection` | `dev_tools_diagnostics.dart` | 163 |
/// | `DeveloperSection` / `DebugPanels` | `dev_tools_developer.dart` | 678 |
///
/// **类不能跨 part**（一个类体必须落在同一个文件里）⇒ 切割线只能落在顶层
/// 类边界上，这是这种拆分唯一的硬约束。
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

part 'dev_tools_mods.dart';
part 'dev_tools_mod_config.dart';
part 'dev_tools_diagnostics.dart';
part 'dev_tools_developer.dart';

/// 列表里的一行（四个分区共用）。
class AdminRow extends StatelessWidget {
  const AdminRow({
    required this.title,
    this.subtitle = '',
    this.badges = const <String>[],
    this.trailing,
    super.key,
  });

  final String title;

  /// 副标题。**空串 = 整行不画**（2026-10-08：ID · 版本 · 协议版本只在
  /// 开发者模式里显示；产品面只剩卡片名与启用开关）。
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
                if (subtitle.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 2),
                  Text(
                    subtitle,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: colors.contentMuted,
                    ),
                  ),
                ],
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
        const SectionHeader(title: '模型库'),
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
            // 2026-10-08：不再写 assets/models/<id>/ 这种仓库内路径（用户看不懂，
            // 也不该背目录约定）。空态只说下一步做什么。
            hint: '在下面填皮套的文件夹名。仓库不自带皮套。',
          )
        else
          for (final ModelInfo m in models)
            AdminRow(
              title: m.displayName.isEmpty ? m.id : m.displayName,
              subtitle:
                  '${m.id} · v${m.version} · ${m.humanSize} · '
                  '${m.textureCount} 张贴图',
              // 徽章 2026-10-08：只剩「当前激活」是用户要看的；
              // 「含物理」「有显示配置」是模型工程属性（开发者排障用）。
              badges: <String>[
                if (m.active) '当前激活',
                if (devMode && m.hasPhysics) '含物理',
                if (devMode && m.hasDisplayInfo) '有显示配置',
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
/// # 跨语言那一半已经**冻结**（2026-10-05，D6 注释纠偏）
///
/// 它原本**必须逐字等于** Rust 侧 `WINDOW_REASON_NATIVE_SHELL_DORMANT`
///（`live2d-ai-mod-pet-desktop`），由 Dart 侧的 `pet_desktop_state_test.dart`
/// 与 Rust 的 `window_reason_string_is_stable_and_ascii` 两侧对守。
/// 该 crate 已于 W2-A **物理删除**（`ARCHIVED-mods.md` §2.4）⇒ 跨语言守卫
/// 随 crate 一起冻结在台账里，**不再有活的生产者**。
///
/// 常量本身留着：`window` 是**通用协议键**（任意 Mod 的 `state_json` 都可以
/// 报它），`formatWindowState` 仍在产品路径上——由
/// `test/mod_state_surface_test.dart` 按通用形态守。
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
  // 2026-10-09「两类 TTS」：拉起进程类 Mod（`local-tts`）的通用字段。
  // 它没有专用面板，这几行就是用户在「扩展」里看到的**唯一**标题。
  'pid': '进程号',
  'child_running': '子进程在跑',
  'exit_code': '退出码',
  'base_url': '声明地址',
  'launch_mode': '拉起方式',
  'wait_error': '等待错误',
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

