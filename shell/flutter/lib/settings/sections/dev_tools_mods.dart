part of 'dev_tools_section.dart';

class ModsSection extends StatelessWidget {
  const ModsSection({
    required this.mods,
    required this.loading,
    this.devMode = false,
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
    this.onCommand,
    this.pickCardFile,
    this.actionScales,
    this.directorDebug,
    super.key,
  });

  final List<ModInfo> mods;
  final bool loading;

  /// 开发者模式：只决定卡片上**多不多一行** ID · 版本 · 协议版本
  ///（2026-10-08）。启用开关、卡片名与产品面板与它无关。
  final bool devMode;
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

  /// 注入给各 Mod 面板的**一次性命令**通道（`POST /api/v1/mods/{id}/command`）。
  ///
  /// 为 null 时卡片自己造一个同源 `ModsApi()`（理由同 `onLoadState`）。
  /// 它让 widget 测试能像别的面板测试那样注入 fake，从而断言「点了什么 →
  /// 送了什么」而不碰网络（persona 的导入回归就靠它）。
  final ModCommandSender? onCommand;

  /// 角色卡文件读取器（组合根注入；透传给需要它的面板）。
  final PersonaCardFilePicker? pickCardFile;

  /// 动作幅度旋钮的接线（2026-10-09）：只有 director 面板消费它；
  /// null = 不渲染那块（宿主还没接线 / 纯 widget 测试）。
  final ActionScalesWiring? actionScales;

  /// 导演卡片里「表情调试 / 动作调试 / 临时幅度」的接线（2026-10-09 从核心
  /// 「开发模式」页搬来）；同样只有 director 面板消费，null = 不渲染那块。
  final DirectorDebugWiring? directorDebug;

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
        // 标题跟导航枚举走（2026-10-08：`SettingsSection.mods.label` = 「扩展」）。
        // 底层仍是 Mod 边界，但那是架构词，不该当分区标题。
        const SectionHeader(title: '扩展'),
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
                subtitle: devMode
                    ? '${m.id} · v${m.version} · api v${m.apiVersion}'
                    : '',
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
                devMode: devMode,
                busy: busyId != null,
                onToggle: onToggle,
                onSaveConfig: onSaveConfig,
                onLoadState: onLoadState,
                activeSessionId: activeSessionId,
                onModChanged: onModChanged,
                onCommand: onCommand,
                pickCardFile: pickCardFile,
                actionScales: actionScales,
                directorDebug: directorDebug,
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

/// 发一条 Mod 命令（`POST /api/v1/mods/{id}/command`）。
///
/// 与 [ModStateLoader] 同款：宿主接线一次，widget 测试注入 fake 即可覆盖
/// 「点了什么 → 送了什么 → 显示了什么」，面板自己不认识 base URL。
typedef ModCommandSender =
    Future<ModCommandResult> Function(
      String id,
      String command,
      Map<String, Object?> args,
    );

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
    required this.devMode,
    required this.busy,
    required this.onToggle,
    required this.onSaveConfig,
    required this.onLoadState,
    this.activeSessionId,
    this.onModChanged,
    this.onCommand,
    this.pickCardFile,
    this.actionScales,
    this.directorDebug,
    super.key,
  });

  final ModInfo mod;

  /// 见 [ModsSection.devMode]：只决定卡片副标题画不画。
  final bool devMode;
  final bool busy;
  final Future<void> Function(String id, bool enabled)? onToggle;
  final Future<ModConfigResult> Function(String id, Map<String, Object?> config)?
      onSaveConfig;
  final ModStateLoader? onLoadState;
  final String? activeSessionId;
  final ValueChanged<String>? onModChanged;
  final ModCommandSender? onCommand;
  final PersonaCardFilePicker? pickCardFile;

  /// 见 [ModsSection.actionScales]：透传给 ModPanelContext。
  final ActionScalesWiring? actionScales;

  /// 见 [ModsSection.directorDebug]：透传给 ModPanelContext。
  final DirectorDebugWiring? directorDebug;

  @override
  State<_ModConfigTile> createState() => _ModConfigTileState();
}

