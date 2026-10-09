part of 'dev_tools_section.dart';

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

  /// 面板声明的「**产品面完全不渲染**」key（2026-10-08；见 [ModPanel.hiddenKeys]）。
  ///
  /// 它们**不进**_plainFields、也**不进**「高级」——用户看不懂的旋钮不该以任何
  /// 形式出现在产品界面上（键本身仍被解析，既有值由 [_buildConfig] 原样带走）。
  Set<String> get _hiddenKeys =>
      modPanelFor(widget.mod.id)?.hiddenKeys ?? const <String>{};

  /// 面板声明的「**只在开发模式里画**」key（见 [ModPanel.devKeys]）。
  Set<String> get _devKeys =>
      modPanelFor(widget.mod.id)?.devKeys ?? const <String>{};

  /// 这一次渲染**真正不画**的 key 集合：
  ///
  /// ```text
  /// effectiveHidden = hiddenKeys ∪ (devMode ? ∅ : devKeys)
  /// ```
  ///
  /// 渲染（[_plainFields] / [_advancedFields]）与保存（[_buildConfig]）
  /// 读的是**同一份**判据——只改渲染不改保存，会把没画出来的键写成默认值。
  Set<String> get _effectiveHidden => widget.devMode
      ? _hiddenKeys
      : <String>{..._hiddenKeys, ..._devKeys};

  List<ModSettingField> get _plainFields => <ModSettingField>[
    for (final ModSettingField f in _spec.fields)
      if (!_advancedKeys.contains(f.key) && !_effectiveHidden.contains(f.key)) f,
  ];

  List<ModSettingField> get _advancedFields => <ModSettingField>[
    for (final ModSettingField f in _spec.fields)
      if (_advancedKeys.contains(f.key) && !_effectiveHidden.contains(f.key)) f,
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
  ///
  /// **面板声明为隐藏的键原样不写**（2026-10-08，见 [ModPanel.hiddenKeys]）：
  /// 它们不在界面上，用户没机会改；写回只会把「当前值」再写一遍（或把 spec
  /// 默认值灌进 `mods.json`）。跳过 = 磁盘上那份严格不变。
  ///
  /// 判据是 [_effectiveHidden]（不是只看 `hiddenKeys`）：开发模式关着时
  /// `devKeys` 同样没画出来，保存时也必须原样带走。
  Map<String, Object?> _buildConfig() {
    final Map<String, Object?> next = Map<String, Object?>.of(
      widget.mod.config,
    );
    for (final ModSettingField f in _spec.fields) {
      if (_effectiveHidden.contains(f.key)) continue;
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

  /// 成功文案（2026-10-09 口径）：只说「配置生效 / 未启用」，
  /// **不写「进程已启动」**——`restarted` 是服务端的自述，前端看不到子进程，
  /// 声称进程起来了就是把推断当事实。
  String _okMessage(ModConfigResult r) {
    if (r.restarted) return '已保存并生效';
    if (r.enabled == false) return '已保存，未启用';
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
        // 副标题只在开发者模式画（2026-10-08）；空串 = 整行不画。
        subtitle: widget.devMode
            ? '${m.id} · v${m.version} · api v${m.apiVersion}'
            : '',
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
            // 2026-10-09「两类 TTS」：这个按钮是**所有 Mod 共用**的，它的语义是
            // 「保存配置并让改动生效」（服务端在已启用时 restart）。旧的「保存」
            // 让人以为写完文件就完了——`local-tts` 的 `on_apply` 正是只在这一次
            // 真的拉起进程。忙碌文案保持「保存中…」。
            label: Text(_saving ? '保存中…' : '保存并应用'),
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

  /// **没有专用面板的 Mod** 的兜底运行态面：把 `GET /api/v1/mods/{id}/state`
  /// 如实铺开。
  ///
  /// # 为什么有专用面板就整块不渲染（2026-10-06 裁决，R4-T2 / T2-B2）
  ///
  /// 在册 5 个 Mod 各有自己的产品面板，**运行态与读取失败都由那张面板承担**
  /// （逐个核过：`settings/mods/*_panel.dart` 里 `ctx.state` 与 `ctx.stateError`
  /// 都有自己的呈现——external-input / persona / voice-input / memory 四张
  /// 面板都有「读数 + 读不到」两面，director 面板自己讲结论）。通用块再铺一
  /// 遍只会把「一个键一行」的原始表压在产品面板上：导演那张卡最典型
  /// （十几个键，含 `presets: 8 项`）。所以判据是「**没有**专用面板才渲染」
  /// ——`modPanelFor(id) == null`。
  ///
  /// 反过来，**社区 Mod 没有面板时这里仍是唯一**的运行态面与错误面
  /// （`state_unavailable` 之类要看得见才谈得上如实）。
  ///
  /// 字段名→中文标签的映射见 [`kModStateLabels`]；未知字段不隐藏。
  /// `window` 的休眠文案（「窗口未开（原生壳休眠），此面仅状态」）由
  /// [`formatWindowState`] 给出——**软闭环的可见终点**。
  Widget _stateBlock() {
    // 有专用面板 ⇒ 运行态由那张面板承担（见头注）；通用兜底块不掺和。
    if (modPanelFor(widget.mod.id) != null) {
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
        // 开发模式与卡文件读取器：与副标题同一个开关，一次传下来。
        devMode: widget.devMode,
        pickCardFile: widget.pickCardFile,
        // 2026-10-09：动作幅度旋钮随这条上下文进 director 卡片。
        actionScales: widget.actionScales,
        // 2026-10-09：表情/动作调试 + 临时幅度（原核心「开发模式」页那块）
        // 也随这条上下文进 director 卡片。
        directorDebug: widget.directorDebug,
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
    final ModCommandSender? send = widget.onCommand;
    if (send != null) {
      final ModCommandResult result = await send(widget.mod.id, command, args);
      unawaited(_loadState());
      return result;
    }
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
    // 2026-10-09：三级功能介绍全删（含面板声明的 fieldHelp——那条呈现口子
    // 已随之退役，ModPanel.fieldHelp 恒为空）。**只有两个例外**：
    //   1. 密钥输入的「留空表示不修改（服务端不回传密钥）」——那是操作规则；
    //   2. 带 min/max 的数字项只留一行范围（数从 spec 上的上下限抄）。
    final String? range = (f.min != null && f.max != null)
        ? '${_trimNumber(f.min!)}-${_trimNumber(f.max!)}'
        : null;
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
          // secret 那行有它自己的固定说明（「留空 = 不修改」）。
          description: f.secret ? '留空表示不修改（服务端不回传密钥）' : null,
          onChanged: (String v) => _edit(f.key, v),
        );
      case ModFieldKind.number:
        return NumberField(
          label: f.label,
          icon: Icons.tag,
          value: _intValue(f),
          description: range,
          // 尊重 spec 的 min/max：NumberField 对越界输入**不回调**。
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

  /// 数字项的 min-max 文案：整数不拖小数点。
  static String _trimNumber(num value) {
    final double d = value.toDouble();
    if (d == d.roundToDouble() && d.abs() < 1e15) return d.toInt().toString();
    return d.toString();
  }
}

// ────────────────────────────────────────────────────────────── 诊断

