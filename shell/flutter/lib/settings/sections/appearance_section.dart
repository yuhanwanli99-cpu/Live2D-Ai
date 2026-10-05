/// 「外观与互动」分区（规格 §4.1.5）。
///
/// 三组：**外观**（配色主题 + 舞台背景图）、**舞台与口型**、**互动**——
/// 全是**纯本地 `DisplayPrefs`**，不需要草稿，也没有保存按钮。
///
/// # 文件名的来历（原名 `actions_section.dart`，2026-09-11 改名）
///
/// **这个文件从来就不是「动作」分区**——它的枚举项一直叫「外观与互动」，
/// 装的是配色主题、舞台背景图、模型缩放、口型灵敏度、互动开关。
/// 只因为当年它多带了一组**只读的「动作记录」**，文件名被叫成了 actions。
///
/// 用户裁定 LLM **无工具、只做对话**后，动作子系统（`live2d_perform_action`
/// 工具、`action_state` 帧、动作日志与手动触发入口）整个移出成品，
/// 那一组随之删除；文件名改为 `appearance_section.dart` 以名实相符。
/// 留这段是因为**下一个人很可能再按旧名误判一次**：`actions_section.dart`
/// 一出现就让人以为删掉它不影响外观设置，实际会整套删掉主题与口型。
///
/// # 行数（≤1000 豁免，理由写在这里）
///
/// 2026-09-28（Stage B · R6-b 第 0 步）：背景域那 1011 行抽到了
/// `appearance_background.dart`，本文件 1489 → 约 700 行。**仍超 ≤500**：
/// 剩下的是这四块的字段编排（外观 / 舞台单图轮播 / 舞台与口型 / 互动，
/// 外加服务端的动作幅度），而抽取的任务口径是「只搬不改」——任务书明写
/// 「不必强行压到 ≤500，那是 Stage C3 的事」。C3 继续按「舞台与口型」
/// 与「互动」两域拆即可，字段清单与顺序在那之前不动。
library;

import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../../api/settings_models.dart';
import '../../ui/section_header.dart';

import '../../design/background_item.dart';
import '../../design/theme_id.dart';
import '../../design/tokens.dart';
import '../../settings/display_prefs.dart';
import '../../ui/background_logic.dart';
import '../../ui/background_patterns.dart';
import '../../ui/field_row.dart';
import '../../ui/group_card.dart';
import '../../ui/shell_backdrop.dart';
import '../../ui/emphasized_text.dart';
import '../../ui/theme.dart';
import '../../ui/theme_picker.dart';

part 'appearance_background.dart';
part 'appearance_background_library.dart';
part 'appearance_background_style.dart';

class AppearanceSection extends StatelessWidget {
  const AppearanceSection({
    required this.prefs,
    required this.onPrefsChanged,
    this.devMode = false,
    this.onPickStageImage,
    this.onClearStageImage,
    this.stageImageMessage,
    this.stageImageFailed = false,
    this.onPickShellImage,
    this.onClearShellImage,
    this.shellImageMessage,
    this.shellImageFailed = false,
    this.onAddToPlaylist,
    this.onClearPlaylist,
    this.action,
    this.onHeadScaleChanged,
    this.onBodyScaleChanged,
    this.onExpressionScaleChanged,
    this.modelOverrideEnabled = false,
    this.onModelOverrideEnabledChanged,
    this.onModelHeadScaleChanged,
    this.onModelBodyScaleChanged,
    this.onModelExpressionScaleChanged,
    this.onResetModelOverride,
    this.modelOverrideMessage,
    this.modelOverrideFailed = false,
    this.onRemoveBackground,
    this.onRemoveBackgrounds,
    this.onReorderBackground,
    this.onPreviewBackground,
    this.onAddPattern,
    super.key,
  });

  /// 本地显示偏好（纯本地，`localStorage`，**不走设置草稿**）。
  final DisplayPrefs prefs;
  final ValueChanged<DisplayPrefs> onPrefsChanged;

  final bool devMode;

  /// 选/清舞台背景图。为 `null` 时按钮禁用（**不是**点了没反应）。
  final VoidCallback? onPickStageImage;
  final VoidCallback? onClearStageImage;

  /// 上次选图的结果（超限时是**错误态**：图太大记不住）。
  final String? stageImageMessage;
  final bool stageImageFailed;

  /// 壳全局背景的选 / 清图（2026-09-14，rc.5）。同步开着时它们改的是
  /// **舞台那张图**（共用一份真相）；关掉才改壳自己的。
  final VoidCallback? onPickShellImage;
  final VoidCallback? onClearShellImage;
  final String? shellImageMessage;
  final bool shellImageFailed;

  /// 舞台背景轮播列表：把**当前**舞台图追加进列表 / 清空列表。
  ///
  /// 列表由用户手动维护（`DisplayPrefs.stagePlaylist`），图源全部来自本机
  /// 偏好；增删 / 排序在 [_StagePlaylistEditor] 里做，不依赖任何 Mod。
  final VoidCallback? onAddToPlaylist;
  final VoidCallback? onClearPlaylist;
  // ── 动作幅度（2026-09-16，服务端产品设置） ──
  //
  // 与上面那些**纯本地 DisplayPrefs** 不同：这三项存在 `live2d-ai.toml` 的
  // `[action]` 段，走设置草稿 + 保存按钮（PATCH /api/v1/settings）。
  // 为 null 时整块不渲染（宿主还没拿到服务端设置）。
  final ActionSettingsView? action;
  final ValueChanged<double>? onHeadScaleChanged;
  final ValueChanged<double>? onBodyScaleChanged;
  final ValueChanged<double>? onExpressionScaleChanged;

  // ── 本模型覆盖（阶段5 D40，2026-09-26） ──
  //
  // 与上面三条**全局**滑条的区别：全局值走设置草稿 + 「保存」；
  // 本模型覆盖走**直接 PATCH**（`[action.models.<id>]`），改完即写盘、
  // 不需要保存，所以这里只上报，不持有草稿。
  //
  // 没有覆盖 / 未识别模型时，三条滑条仍是全局值、仍走原来的草稿回调
  // （旧语义一字不改，见 build 里的 `modelOverrideOn`）。
  final bool modelOverrideEnabled;
  final ValueChanged<bool>? onModelOverrideEnabledChanged;
  final ValueChanged<double>? onModelHeadScaleChanged;
  final ValueChanged<double>? onModelBodyScaleChanged;
  final ValueChanged<double>? onModelExpressionScaleChanged;

  /// 「恢复跟随全局」：删掉本模型覆盖（`models.<id> = null`）。
  final VoidCallback? onResetModelOverride;

  /// 覆盖 PATCH 的结果（失败时带服务端给的 code：message）。
  final String? modelOverrideMessage;
  final bool modelOverrideFailed;

  /// 从背景库移除第 N 项。
  final ValueChanged<int>? onRemoveBackground;

  /// 批量移除（**升序**的下标集合）。
  final ValueChanged<List<int>>? onRemoveBackgrounds;

  /// 拖动排序（旧下标 → 新下标）。
  final void Function(int oldIndex, int newIndex)? onReorderBackground;

  /// 点缩略图 = 切到这一张（仅本次会话，不落盘）。
  final ValueChanged<int>? onPreviewBackground;

  /// 往背景库加一个内置图案。
  final ValueChanged<int>? onAddPattern;

  @override
  Widget build(BuildContext context) {
    // 这些参数实时下发给渲染面：**没有「保存」按钮**。
    // 理由：拖滑杆时就想看到舞台反应，多一步保存会毁掉这个手感。
    // 主题同理——点一下立刻整套换掉，正是它该有的手感。
    // 三个**组卡片**（2026-09-27）：分组从「一条分隔线」变成「一张面」。
    // 字段清单与顺序**一字未动**——本轮只换外层容器，
    // 分区 id / label / 字段的语义全部保持原样（可访问性测试依赖它们）。
    //
    // 本模型覆盖（阶段5 D40）：覆盖**开启**（本模型有覆盖）时，三条滑条
    // 编辑的是「逐键 override[key] ?? global[key]」的有效值，回调直接 PATCH；
    // 关闭时三条滑条就是原来的全局值 + 草稿回调（旧语义不动）。
    //
    // 两个轮播块**按来源互斥**（DEC-2）：这一位同时决定下面那张
    // 「舞台单图轮播」卡出不出来。
    final bool librarySource =
        prefs.backgroundSource == DisplayPrefs.backgroundSourceLibrary;
    final ActionSettingsView? actionView = action;
    final bool modelOverrideOn =
        actionView != null &&
        modelOverrideEnabled &&
        actionView.activeModelId.isNotEmpty;
    final ActionSettingsView? effective = actionView?.effectiveForActiveModel;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        GroupCard(
          title: '外观',
          description: '配色与材质。**只影响本机显示**，选完立刻生效，不需要保存。',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              ThemePickerField(
                value: prefs.theme,
                onChanged: (AppThemeId id) =>
                    onPrefsChanged(prefs.copyWith(theme: id)),
              ),
              // 「圆角幅度」在 2026-09-27 被删掉了（用户口径：「本身 web 端
              // 无需繁杂设置」）——圆角改为全仓库**固定一个值**，
              // 取值入口是 `AppMaterial.kFixedRadiusScale`。
              _CommitSliderField(
                label: '描边强度',
                icon: Icons.line_weight,
                value: prefs.edgeStrength,
                min: DisplayPrefs.minEdgeStrength,
                max: DisplayPrefs.maxEdgeStrength,
                onCommit: (double v) =>
                    onPrefsChanged(prefs.copyWith(edgeStrength: v)),
                description: '分隔线与卡片的边线粗细；调低更轻盈，层级改由面的深浅承担',
                minLabel: '轻盈',
                maxLabel: '清晰',
              ),
              _BackgroundBlock(
                prefs: prefs,
                // 两条消息**合并成一条**：现在只有一个「背景」，两个来源的
                // 消息轮流出现会让用户以为刚才那条是上一张图的。
                message: shellImageMessage ?? stageImageMessage,
                failed: shellImageFailed || stageImageFailed,
                onAddImage: onPickShellImage,
                onClearLibrary: onClearShellImage,
                onPickStageImage: onPickStageImage,
                onClearStageImage: onClearStageImage,
                onRemoveItem: onRemoveBackground,
                onAddPattern: onAddPattern,
                onChanged: onPrefsChanged,
                onReorderItem: onReorderBackground,
                onRemoveMany: onRemoveBackgrounds,
                onPreviewItem: onPreviewBackground,
              ),
            ],
          ),
        ),
        // ── 舞台单图轮播（DEC-2 的分工那一半）──
        //
        // 它与上面那块「壳背景轮播」**按来源互斥**：来源 = 舞台那张时只有这块
        // （走渲染面 stage-bg 帧），来源 = 背景库时只有那块（Flutter 层换图）。
        // 两套列表、两套预算、两条下发通道，同时摆出来用户只会以为它们是同一个
        // 开关——rc.5 §9.2 记的就是这个重叠。
        //
        // 名字分清「舞台单图轮播」vs「壳背景轮播」：这一块管的是**舞台那一张图**
        // 的历史列表，不是背景库（背景库那套预算与它无关）。
        if (!librarySource)
          GroupCard(
            title: '舞台单图轮播',
            description:
                '把**当前**舞台那张图追加进列表（Wave 2/3）。'
                '这份列表只住本机偏好，经渲染面 stage-bg 帧下发；'
                '与上面「壳背景轮播」（背景库、壳自己画的）是**两套通道**，'
                '所以两者按「背景来源」互斥显示。',
            child: _StageImageView(
              hasImage: prefs.stageImage != null,
              playlist: prefs.stagePlaylist,
              currentImage: prefs.stageImage,
              message: stageImageMessage,
              failed: stageImageFailed,
              onPick: onPickStageImage,
              onClear: onClearStageImage,
              onAddToPlaylist: onAddToPlaylist,
              onClearPlaylist: onClearPlaylist,
              // 删除 / 上移下移只改**本地列表**：宿主 `_updatePrefs` 是唯一落点。
              onPlaylistChanged: (List<String> next) =>
                  onPrefsChanged(prefs.copyWith(stagePlaylist: next)),
            ),
          ),
        GroupCard(
          title: '舞台与口型',
          description: '这些参数立刻下发到渲染面，**不需要保存**。',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              _CommitSliderField(
                label: '模型缩放',
                icon: Icons.zoom_out_map,
                value: prefs.scale,
                // **区间从这里读，不写字面量**（2026-09-11 修）：
                // 这两个数在 `DisplayPrefs` 里已经声明过一次（`clampScale` 也读它们），
                // 在 UI 里再写一遍就是第二个真源。回归见 `display_prefs_test.dart`。
                min: DisplayPrefs.minScale,
                max: DisplayPrefs.maxScale,
                onCommit: (double v) =>
                    onPrefsChanged(prefs.copyWith(scale: v)),
                minLabel: '更远',
                maxLabel: '更近',
              ),
              _CommitSliderField(
                label: '口型灵敏度',
                icon: Icons.graphic_eq,
                value: prefs.mouthSensitivity,
                min: DisplayPrefs.minMouthSensitivity,
                max: DisplayPrefs.maxMouthSensitivity,
                onCommit: (double v) =>
                    onPrefsChanged(prefs.copyWith(mouthSensitivity: v)),
                description: '1.00 为标定值；觉得嘴动得太小就调大',
                minLabel: '克制',
                maxLabel: '夸张',
              ),
              ToggleField(
                label: '口型同步',
                icon: Icons.record_voice_over_outlined,
                value: prefs.lipSync,
                onChanged: (bool v) =>
                    onPrefsChanged(prefs.copyWith(lipSync: v)),
                description: '关掉后声音照放，但嘴不动',
              ),
              ToggleField(
                label: '待机小动作',
                icon: Icons.self_improvement,
                value: prefs.idleEnabled,
                onChanged: (bool v) =>
                    onPrefsChanged(prefs.copyWith(idleEnabled: v)),
                description: '呼吸 / 眨眼 / 微表情',
              ),
              if (devMode)
                SegmentedField<int>(
                  label: '渲染档位',
                  icon: Icons.speed,
                  value: prefs.tier,
                  options: const <FieldOption<int>>[
                    FieldOption<int>(value: 4096, label: '4K'),
                    FieldOption<int>(value: 8192, label: '8K'),
                    FieldOption<int>(value: 16384, label: '16K'),
                  ],
                  onChanged: (int v) => onPrefsChanged(prefs.copyWith(tier: v)),
                  description: '性能与画质的权衡；需要档位知识，所以进开发者层',
                ),
            ],
          ),
        ),
        if (actionView != null) ...[
          const Divider(),
          const SectionHeader(
            title: '动作幅度',
            description: '拖动**立刻在舞台上生效**（无需先保存）；'
                '「保存」才把值写进服务端（live2d-ai.toml 的 [action] 段），'
                '重开也还在。放弃改动 / 重新加载会立刻回到磁盘上的值。'
                '出厂 head 75% / body 80% / expression 100%。'
                '头摆太大就调小 head，身摆太小就调大 body。',
          ),
          // 本模型覆盖块（阶段5 D40）：模型名 + 开关 + 覆盖 PATCH 的结果。
          _ModelOverrideHeader(
            activeModelId: actionView.activeModelId,
            enabled: modelOverrideEnabled,
            onEnabledChanged: onModelOverrideEnabledChanged,
            message: modelOverrideMessage,
            failed: modelOverrideFailed,
          ),
          // 三条滑条：覆盖开启时编辑本模型覆盖值（直接 PATCH），
          // 关闭时就是原来的全局值 + 草稿回调。**不是两套滑条**——
          // 关闭态的语义与改动前逐字一致（保留草稿 / 保存）。
          SliderField(
            label: '头部摆幅',
            icon: Icons.face_retouching_natural,
            value: modelOverrideOn
                ? effective!.headScale
                : actionView.headScale,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: modelOverrideOn
                ? onModelHeadScaleChanged != null
                : onHeadScaleChanged != null,
            onChanged: modelOverrideOn
                ? (onModelHeadScaleChanged ?? (double _) {})
                : (onHeadScaleChanged ?? (double _) {}),
            description: modelOverrideOn
                ? '本模型覆盖：直接写 [action.models.${actionView.activeModelId}]，不需要点保存'
                : 'ParamAngle* 的倍率；出厂 75%',
          ),
          SliderField(
            label: '身体摆幅',
            icon: Icons.accessibility_new,
            value: modelOverrideOn
                ? effective!.bodyScale
                : actionView.bodyScale,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: modelOverrideOn
                ? onModelBodyScaleChanged != null
                : onBodyScaleChanged != null,
            onChanged: modelOverrideOn
                ? (onModelBodyScaleChanged ?? (double _) {})
                : (onBodyScaleChanged ?? (double _) {}),
            description: modelOverrideOn
                ? '本模型覆盖：未覆盖的键逐键回落全局'
                : 'ParamBodyAngle* 的倍率；出厂 80%，身/头比约 0.35',
          ),
          SliderField(
            label: '表情幅度',
            icon: Icons.mood,
            value: modelOverrideOn
                ? effective!.expressionScale
                : actionView.expressionScale,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: modelOverrideOn
                ? onModelExpressionScaleChanged != null
                : onExpressionScaleChanged != null,
            onChanged: modelOverrideOn
                ? (onModelExpressionScaleChanged ?? (double _) {})
                : (onExpressionScaleChanged ?? (double _) {}),
            description: modelOverrideOn
                ? '本模型覆盖：口 / 眉 / 眼的倍率'
                : '口 / 眉 / 眼 的倍率；出厂 100%',
          ),
          // 「恢复跟随全局」只在覆盖开启时可点（没有覆盖时禁用，不是点了没反应）。
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton(
              onPressed: modelOverrideOn ? onResetModelOverride : null,
              child: const Text('恢复跟随全局'),
            ),
          ),
        ],
        GroupCard(
          title: '互动',
          description:
              '「允许拖动与缩放」管的是**拖拽 / 滚轮 / 双击复位**，'
              '与「点击角色有没有反应」无关。',
          child: ToggleField(
            label: '允许拖动与缩放',
            icon: Icons.pan_tool_outlined,
            value: prefs.allowDragZoom,
            onChanged: (bool v) =>
                onPrefsChanged(prefs.copyWith(allowDragZoom: v)),
            description: '关掉后模型不因拖动/滚轮移动；缩放仍可用舞台右下角的按钮',
          ),
        ),
      ],
    );
  }
}


/// 「本模型覆盖」子块（阶段5 D40，2026-09-26）：模型名 + 开关 + 结果。
///
/// 三条理由写在这里，避免下一个人再按「第二个全局滑条」改错：
/// 1. 开关的**值**由宿主给（= 本模型是否已有覆盖）；这里不持有状态，
///    所以「开 / 关」不会与磁盘分叉；
/// 2. `active_model_id` 为空时**如实说「未识别当前模型」并禁用开关**——
///    假装能开会在写回时被服务端拒（模型 id 非法）；
/// 3. 覆盖改走**直接 PATCH**（不需要「保存」），所以这里只上报，不碰草稿。
class _ModelOverrideHeader extends StatelessWidget {
  const _ModelOverrideHeader({
    required this.activeModelId,
    required this.enabled,
    required this.onEnabledChanged,
    required this.message,
    required this.failed,
  });

  final String activeModelId;
  final bool enabled;
  final ValueChanged<bool>? onEnabledChanged;
  final String? message;
  final bool failed;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final AppPalette palette = appPaletteOf(context);
    final bool known = activeModelId.isNotEmpty;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          known ? '当前模型：$activeModelId' : '未识别当前模型',
          style: theme.textTheme.labelLarge,
        ),
        const SizedBox(height: Space.s1),
        EmphasizedText(
          known
              ? '「本模型覆盖」开启后，下面三条滑条编辑的就是**这个模型**的幅度'
                    '（`[action.models.$activeModelId]`）。未覆盖的键**逐键回落全局**；'
                    '「恢复跟随全局」删掉本模型的整份覆盖。'
              : '服务端还没给出当前模型 id（`active_model_id`）——'
                    '先激活一个模型，再回来设本模型覆盖。',
          style: theme.textTheme.labelSmall?.copyWith(
            color: colors.contentMuted,
          ),
        ),
        ToggleField(
          label: '本模型覆盖',
          icon: Icons.tune,
          value: enabled,
          enabled: known && onEnabledChanged != null,
          onChanged: onEnabledChanged ?? (bool _) {},
          description: known
              ? (enabled ? '正在用本模型的覆盖值' : '关闭 = 跟随全局')
              : '未识别当前模型，无法开启',
        ),
        if (message != null)
          Padding(
            padding: const EdgeInsets.only(top: Space.s1),
            child: EmphasizedText(
              message!,
              style: theme.textTheme.labelSmall?.copyWith(
                color: failed ? palette.warning : colors.contentMuted,
              ),
            ),
          ),
      ],
    );
  }
}

/// 舞台背景图那一行。
///
/// 三件事必须同时说清（否则用户会以为功能坏了）：
/// 1. **不是二选一**——背景图盖在纯色底上，清掉图底色就回来；
/// 2. 图太大时**本次有效但不记住**（`failed` 用警告色，不是静默成功）；
/// 3. 按钮是**文字**，不是图标（用户裁决「尽量少用图片用文字做按钮」）。
///
/// 追加一行**背景轮播列表**的最小操作：「加入轮播」把当前这张图 append 进
/// 列表、「清空轮播」清空，并显示当前张数。这是**用户手动维护**的图库，
/// 有**三条预算**（每张 / 总长 / 项数）——超限时按钮仍然可按，由宿主给出
/// 一句可读反馈（**不弹异常**、不静默膨胀）。
///
/// 另有**增删 / 排序闭环**：每张一行「上移 / 下移 / 删除」（见
/// [_StagePlaylistEditor]）。仍**不做**拖拽排序与缩略图——删除 / 移动本身
/// 不会让列表超预算（只会变小或重排），所以这里的操作**不需要**预算校验。
class _StageImageView extends StatelessWidget {
  const _StageImageView({
    required this.hasImage,
    required this.playlist,
    required this.currentImage,
    required this.message,
    required this.failed,
    required this.onPick,
    required this.onClear,
    required this.onAddToPlaylist,
    required this.onClearPlaylist,
    required this.onPlaylistChanged,
  });

  final bool hasImage;

  /// 轮播列表（`DisplayPrefs.stagePlaylist`）。
  final List<String> playlist;

  /// 当前舞台那张图（高亮「当前」用；判据见 `stagePlaylistIndexOf`）。
  final String? currentImage;

  final String? message;
  final bool failed;
  final VoidCallback? onPick;
  final VoidCallback? onClear;
  final VoidCallback? onAddToPlaylist;
  final VoidCallback? onClearPlaylist;

  /// 列表编辑（删除 / 上移下移）后的**新列表**。
  final ValueChanged<List<String>> onPlaylistChanged;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final AppPalette palette = appPaletteOf(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text('舞台背景图', style: theme.textTheme.labelLarge),
          const SizedBox(height: Space.s1),
          EmphasizedText(
            '背景图**盖在纯色底上**：有图时看得到图，清掉后回到主题的纯色舞台。'
            '超过 ${(kStageImageMaxChars / 1024).round()} KB 的图只在本次会话有效。',
            style: theme.textTheme.labelSmall?.copyWith(
              color: colors.contentMuted,
            ),
          ),
          const SizedBox(height: Space.s2),
          Wrap(
            spacing: Space.s2,
            runSpacing: Space.s2,
            children: <Widget>[
              FilledButton.tonal(
                onPressed: onPick,
                child: Text(hasImage ? '换一张背景图' : '选择背景图'),
              ),
              if (hasImage)
                TextButton(onPressed: onClear, child: const Text('清除背景图')),
            ],
          ),
          if (message != null)
            Padding(
              padding: const EdgeInsets.only(top: Space.s1),
              child: EmphasizedText(
                message!,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: failed ? palette.warning : colors.contentMuted,
                ),
              ),
            ),
          const SizedBox(height: Space.s3),
          Text('背景轮播列表', style: theme.textTheme.labelLarge),
          const SizedBox(height: Space.s1),
          EmphasizedText(
            '这份列表由你手动维护，**只在本机保存**。'
            '当前 **${playlist.length}** 张，上限 $kStagePlaylistMaxItems 张、'
            '合计 ${(kStagePlaylistMaxChars / 1024).round()} KB。'
            '「加入轮播」把**当前这张**追加进列表；下面每张可**上移 / 下移 / 删除**。',
            style: theme.textTheme.labelSmall?.copyWith(
              color: colors.contentMuted,
            ),
          ),
          const SizedBox(height: Space.s2),
          Wrap(
            spacing: Space.s2,
            runSpacing: Space.s2,
            children: <Widget>[
              // 没有当前图 / 到达上限时仍然可点（点了会得到一句原因），
              // 但两个回调都为 null（没接线）时禁用——不假装能操作。
              FilledButton.tonal(
                onPressed: onAddToPlaylist,
                child: const Text('加入轮播'),
              ),
              if (playlist.isNotEmpty)
                TextButton(
                  onPressed: onClearPlaylist,
                  child: const Text('清空轮播'),
                ),
            ],
          ),
          const SizedBox(height: Space.s2),
          _StagePlaylistEditor(
            playlist: playlist,
            currentImage: currentImage,
            onChanged: onPlaylistChanged,
          ),
        ],
      ),
    );
  }
}

/// 轮播列表的**最小编辑器**（Wave 3）：逐张给「上移 / 下移 / 删除」。
///
/// # 为什么是文字按钮而不是拖拽
///
/// 拖拽排序要靠 gesture + 重排动画 + 落点判定，几乎全是不可单测的代码；
/// 而列表上限只有 16 张、编辑频率极低——文字按钮已经把「删哪张 / 移到哪」
/// 变成 `display_prefs.dart` 里两个**可 VM 断言**的纯函数。
/// 首尾两端按钮**禁用**（不是点了没反应）：第一张不能上移、最后一张不能下移。
///
/// 面板里**不渲染缩略图**（性能与体积都不值得），只显示序号、大小与「当前」标记。
class _StagePlaylistEditor extends StatelessWidget {
  const _StagePlaylistEditor({
    required this.playlist,
    required this.currentImage,
    required this.onChanged,
  });

  final List<String> playlist;
  final String? currentImage;
  final ValueChanged<List<String>> onChanged;

  @override
  Widget build(BuildContext context) {
    if (playlist.isEmpty) return const SizedBox.shrink();
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final int currentIndex = stagePlaylistIndexOf(playlist, currentImage);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (int i = 0; i < playlist.length; i++)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s1),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    '第 ${i + 1} 张 · ${_imageKb(playlist[i])}'
                    '${i == currentIndex ? ' · 当前' : ''}',
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: i == currentIndex
                          ? theme.colorScheme.primary
                          : colors.contentMuted,
                    ),
                  ),
                ),
                TextButton(
                  onPressed: i == 0
                      ? null
                      : () => onChanged(moveStagePlaylist(playlist, i, i - 1)),
                  child: const Text('上移'),
                ),
                TextButton(
                  onPressed: i == playlist.length - 1
                      ? null
                      : () => onChanged(moveStagePlaylist(playlist, i, i + 1)),
                  child: const Text('下移'),
                ),
                TextButton(
                  onPressed: () =>
                      onChanged(removeStagePlaylistAt(playlist, i)),
                  child: const Text('删除'),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

/// 列表项大小的可读文案（dataURL 字符数 → KB）。
String _imageKb(String dataUrl) => '约 ${(dataUrl.length / 1024).round()} KB';