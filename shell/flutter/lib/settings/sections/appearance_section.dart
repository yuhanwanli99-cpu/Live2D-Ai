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
library;

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
              SliderField(
                label: '描边强度',
                icon: Icons.line_weight,
                value: prefs.edgeStrength,
                min: DisplayPrefs.minEdgeStrength,
                max: DisplayPrefs.maxEdgeStrength,
                onChanged: (double v) =>
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
        GroupCard(
          title: '舞台背景轮播',
          description:
              '「加入轮播」把**当前**舞台背景图追加进列表（Wave 2/3）。'
              '这份列表只住本机偏好，与上面的**壳背景库**是两套东西——'
              '它管的是「舞台那张图」的历史列表。',
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
              SliderField(
                label: '模型缩放',
                icon: Icons.zoom_out_map,
                value: prefs.scale,
                // **区间从这里读，不写字面量**（2026-09-11 修）：
                // 这两个数在 `DisplayPrefs` 里已经声明过一次（`clampScale` 也读它们），
                // 在 UI 里再写一遍就是第二个真源。回归见 `display_prefs_test.dart`。
                min: DisplayPrefs.minScale,
                max: DisplayPrefs.maxScale,
                onChanged: (double v) =>
                    onPrefsChanged(prefs.copyWith(scale: v)),
                minLabel: '更远',
                maxLabel: '更近',
              ),
              SliderField(
                label: '口型灵敏度',
                icon: Icons.graphic_eq,
                value: prefs.mouthSensitivity,
                min: DisplayPrefs.minMouthSensitivity,
                max: DisplayPrefs.maxMouthSensitivity,
                onChanged: (double v) =>
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

/// 背景库那一整块（2026-09-27）。
///
/// # 它取代了原来那两段（「舞台背景图」+「壳背景」）
///
/// 旧形态的问题不是文案长，是**结构**：两个字段、两个真相、两种清空方式，
/// 而用户心里只有一件事——「给这个壳换张背景」。现在合成一块：
///
/// 1. **背景库**（图 + 内置图案，有序，可增可删）—— 用户能表达「不止一张」；
/// 2. **铺法 / 位置 / 透明度 / 模糊 / 遮罩**—— 只作用在壳，不碰舞台；
/// 3. **轮播**（间隔 + 随机 + 过渡）；
/// 4. **与舞台同步**（默认开）—— 开时壳画舞台那张，背景库留着不渲染。
///
/// 三条文案纪律与旧的一样：不是二选一、失败要说实话、按钮用文字。
class _BackgroundBlock extends StatelessWidget {
  const _BackgroundBlock({
    required this.prefs,
    required this.message,
    required this.failed,
    required this.onAddImage,
    required this.onClearLibrary,
    required this.onPickStageImage,
    required this.onClearStageImage,
    required this.onRemoveItem,
    required this.onAddPattern,
    required this.onChanged,
    this.onReorderItem,
    this.onRemoveMany,
    this.onPreviewItem,
  });

  final DisplayPrefs prefs;
  final String? message;
  final bool failed;
  final VoidCallback? onAddImage;
  final VoidCallback? onClearLibrary;

  /// 「来源 = 舞台那张」时的那两个按钮（换 / 清）。
  final VoidCallback? onPickStageImage;
  final VoidCallback? onClearStageImage;
  final ValueChanged<int>? onRemoveItem;
  final ValueChanged<int>? onAddPattern;
  final ValueChanged<DisplayPrefs> onChanged;

  /// 拖动排序（旧下标 → 新下标）。
  final void Function(int oldIndex, int newIndex)? onReorderItem;

  /// 批量删除（**升序**的下标集合）。
  final ValueChanged<List<int>>? onRemoveMany;

  /// 点缩略图 = 先看这一张。
  final ValueChanged<int>? onPreviewItem;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final AppPalette palette = appPaletteOf(context);
    final List<BackgroundItem> items = prefs.backgrounds;
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text('背景', style: theme.textTheme.labelLarge),
          const SizedBox(height: Space.s1),
          EmphasizedText(
            '背景铺在**聊天与侧栏背后**（舞台由渲染面自己画，不归这里）。'
            '下面每个滑杆都标了它管的是**哪一层**，并且把算出来的数字写出来。',
            style: theme.textTheme.labelSmall?.copyWith(
              color: colors.contentMuted,
            ),
          ),
          const SizedBox(height: Space.s2),
          SegmentedField<int>(
            label: '背景来源',
            icon: Icons.layers_outlined,
            value: prefs.backgroundSource,
            options: const <FieldOption<int>>[
              FieldOption<int>(
                value: DisplayPrefs.backgroundSourceLibrary,
                label: '背景库',
              ),
              FieldOption<int>(
                value: DisplayPrefs.backgroundSourceStageImage,
                label: '舞台那张',
              ),
            ],
            onChanged: (int v) =>
                onChanged(prefs.copyWith(backgroundSource: v)),
            description:
                prefs.backgroundSource == DisplayPrefs.backgroundSourceLibrary
                ? prefs.backgrounds.isEmpty
                      ? '正在画**背景库**，但库里还是空的（下面加一项，或挑一个内置图案）'
                      : '正在画**背景库**里的 ${prefs.backgrounds.length} 项之一'
                : prefs.stageImage == null
                ? '来源是**舞台背景图**，但还没选过图（背景库里的不参与渲染）'
                : '正在画**舞台背景图**那一张（背景库里的不参与渲染）',
          ),
          const SizedBox(height: Space.s2),
          if (prefs.backgroundSource ==
              DisplayPrefs.backgroundSourceLibrary) ...<Widget>[
            _LibraryManager(
              items: items,
              palette: palette,
              onReorder: onReorderItem ?? _noopReorder,
              onRemove: onRemoveItem,
              onRemoveMany: onRemoveMany,
              onPreview: onPreviewItem,
            ),
            const SizedBox(height: Space.s2),
            Wrap(
              spacing: Space.s1,
              runSpacing: Space.s1,
              children: <Widget>[
                for (final int id in BackgroundPatternId.values)
                  ActionChip(
                    label: Text(backgroundPatternLabel(id)),
                    onPressed: () => onAddPattern?.call(id),
                  ),
              ],
            ),
            const SizedBox(height: Space.s2),
          ],
          // 按钮**跟着来源走**：来源是背景库时，「添加图片」加进的就是
          // 正在被渲染的那个列表。不存在「点了没反应」的按钮。
          if (prefs.backgroundSource ==
              DisplayPrefs.backgroundSourceLibrary) ...<Widget>[
            Wrap(
              spacing: Space.s2,
              runSpacing: Space.s2,
              children: <Widget>[
                FilledButton.tonal(
                  onPressed: onAddImage,
                  child: const Text('添加图片'),
                ),
                if (items.isNotEmpty)
                  TextButton(
                    onPressed: onClearLibrary,
                    child: const Text('清空背景库'),
                  ),
              ],
            ),
          ] else
            Wrap(
              spacing: Space.s2,
              runSpacing: Space.s2,
              children: <Widget>[
                FilledButton.tonal(
                  onPressed: onPickStageImage,
                  child: Text(prefs.stageImage == null ? '选择舞台背景图' : '换一张'),
                ),
                if (prefs.stageImage != null)
                  TextButton(
                    onPressed: onClearStageImage,
                    child: const Text('清除舞台背景图'),
                  ),
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
          // ── 渲染参数（两种同步模式下都生效）──
          const SizedBox(height: Space.s2),
          // 铺法与位置只对**图片**有意义（图案是 `CustomPainter` 画满整块，
          // 没有「原图尺寸」可裁可留边）。当前项是图案时把它们收起来，
          // 而不是留两个按了没区别的控件。
          if (prefs.currentItemIsImage) ...<Widget>[
            SegmentedField<int>(
              label: '铺法（图）',
              icon: Icons.photo_size_select_large_outlined,
              value: prefs.imageFit,
              options: const <FieldOption<int>>[
                FieldOption<int>(value: 0, label: '铺满'),
                FieldOption<int>(value: 1, label: '完整'),
              ],
              onChanged: (int v) => onChanged(prefs.copyWith(imageFit: v)),
              description:
                  '「铺满」裁掉边、「完整」留边。'
                  '（2026-09-27 删掉了「拉伸」：它在渲染层与「铺满」是同一条分支，'
                  '是个按了没区别的选项。）',
            ),
            // 位置**只在「完整」下出现**：「铺满」时图已经铺满整个区域，
            // 对齐没有可见效果——摆在那儿只会让人以为是自己按错了。
            if (prefs.imageFit != 1)
              _AlignPad(
                value: prefs.imageAlign,
                onChanged: (int v) => onChanged(prefs.copyWith(imageAlign: v)),
              ),
          ],
          SliderField(
            label: '不透明度（图）',
            icon: Icons.opacity,
            value: prefs.backgroundOpacity,
            min: DisplayPrefs.minBackgroundOpacity,
            max: DisplayPrefs.maxBackgroundOpacity,
            onChanged: (double v) =>
                onChanged(prefs.copyWith(backgroundOpacity: v)),
            description: '**图**本身有多实。0 = 完全不画背景（回到纯色面）；'
                '拉高时下面的遮罩会跟着加强，聊天文字仍然读得出来',
            minLabel: '看不见',
            maxLabel: '压满',
          ),
          SliderField(
            label: '透明程度（界面）',
            icon: Icons.layers_outlined,
            value: prefs.uiTransparency,
            min: DisplayPrefs.minUiTransparency,
            max: DisplayPrefs.maxUiTransparency,
            onChanged: (double v) =>
                onChanged(prefs.copyWith(uiTransparency: v)),
            // 面板不透明度**直接写出来**：这一项没有别的可见表现，
            // 数字是唯一能让用户确认「它真的动了」的东西。
            description: '**面板**有多透：设置面板、组卡片、聊天面板、顶栏'
                '一起变，当前不透明度 **${(AppColors.panelAlphaFor(prefs.uiTransparency) * 100).round()}%**。'
                '${prefs.hasBackground ? '背后有图，所以能看出差别' : '背后没图时聊天面板与顶栏保持不透明（底下是纯色底，半透只会让界面发灰）'}。'
                '遮罩会自动加强以保证文字可读',
            minLabel: '不透明',
            maxLabel: '最透',
          ),
          SegmentedField<int>(
            label: '遮罩（图上方）',
            icon: Icons.contrast,
            value: prefs.backgroundScrim,
            options: const <FieldOption<int>>[
              FieldOption<int>(value: 0, label: '自动'),
              FieldOption<int>(value: 1, label: '无'),
              FieldOption<int>(value: 2, label: '轻'),
              FieldOption<int>(value: 3, label: '重'),
            ],
            onChanged: (int v) => onChanged(prefs.copyWith(backgroundScrim: v)),
            // **把算出来的强度写出来**：这一项的效果本来就是「更暗一点」，
            // 不给数字的话用户改完看不出区别，只能当成又一个无效控件。
            description:
                '压在背景上的一层主题色，让聊天文字在任何图上都读得出来。'
                '当前强度 **${(scrimAlphaFor(imageOpacity: prefs.backgroundOpacity, level: prefs.backgroundScrim, uiTransparency: prefs.uiTransparency) * 100).round()}%**'
                '（${prefs.hasBackground ? '随图变亮自动加强' : '没有背景时不画'}）',
          ),
          _MoreOptions(
            title: '更多外观',
            // 一句话说明它为什么被折起来：默认用不到，但它与「不透明度」
            // 是一对（一张太花的图通常要「更实」或「更糊」才压得住）。
            summary: '模糊等低频选项',
            children: <Widget>[
              SliderField(
                label: '模糊（图）',
                icon: Icons.blur_on,
                value: prefs.backgroundBlur,
                min: 0,
                max: DisplayPrefs.maxBackgroundBlur,
                percentage: false,
                suffix: ' px',
                onChanged: (double v) =>
                    onChanged(prefs.copyWith(backgroundBlur: v)),
                description: '只作用在壳的背景上，**模糊不到舞台**',
                minLabel: '清晰',
                maxLabel: '弥散',
              ),
            ],
          ),
          const Divider(),
          ToggleField(
            label: '轮播',
            icon: Icons.slideshow,
            value: prefs.slideInterval > 0,
            onChanged: (bool on) =>
                onChanged(prefs.copyWith(slideInterval: on ? 30 : 0)),
            description: prefs.backgrounds.length < 2
                ? '背景库至少要 2 项才会转'
                : '按间隔换下一张',
          ),
          if (prefs.slideInterval > 0) ...<Widget>[
            SliderField(
              label: '轮播间隔',
              icon: Icons.timer_outlined,
              value: prefs.slideInterval.toDouble(),
              min: DisplayPrefs.minSlideIntervalSeconds.toDouble(),
              max: DisplayPrefs.maxSlideIntervalSeconds.toDouble(),
              percentage: false,
              suffix: ' 秒',
              onChanged: (double v) =>
                  onChanged(prefs.copyWith(slideInterval: v.round())),
              minLabel: '快',
              maxLabel: '慢',
            ),
            ToggleField(
              label: '随机顺序',
              icon: Icons.shuffle,
              value: prefs.slideRandom,
              onChanged: (bool v) => onChanged(prefs.copyWith(slideRandom: v)),
              description: '不会连着两次同一张',
            ),
          ],
        ],
      ),
    );
  }
}

/// 背景库**管理面板**（2026-09-27）。
///
/// # 它管的四件事
///
/// | 能力 | 为什么需要 |
/// | --- | --- |
/// **拖动排序** | 轮播按**顺序**走，顺序就是用户对「先放谁」的表达 |
/// **点一下预览** | 加完图先看一眼，而不是改完设置再退回主界面找 |
/// **多选 + 批量删** | 一次清掉试了几张的图，比一张张点 × 快得多 |
/// **占用显示** | 配额是**真的会满**的；不说清楚，用户只会看到「加不进去」 |
///
/// # 为什么不内置「重命名」
///
/// 一张背景图没有天然的标题（dataURL 里没有名字），起名要用户自己想，
/// 于是就变成一个「不知道该起什么」的空输入框。图案有固定名（渐变 / 光晕…），
/// 图没有——所以**不给这个能力**，而不是给一个必然被留空的输入框。
///
/// # 状态住在哪
///
/// **本地 state**（`_LibraryManager`）：选中集合、是否处于管理模式，
/// 都不是用户偏好，刷新后不该留。真正的数据（顺序、增减）在 `DisplayPrefs`。
/// 「更多」的折叠区。
///
/// 2026-09-27 引入：**少露一个控件**比**少一个能力**好——所以低频但有用的
/// 选项（模糊、将来的九宫格微调）折到这里，而不是被删掉。
///
/// 默认收起、标题上带一行摘要（所以用户知道里面还有东西）。
class _MoreOptions extends StatefulWidget {
  const _MoreOptions({
    required this.title,
    required this.children,
    this.summary,
  });

  final String title;
  final String? summary;
  final List<Widget> children;

  @override
  State<_MoreOptions> createState() => _MoreOptionsState();
}

class _MoreOptionsState extends State<_MoreOptions> {
  bool _open = false;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        InkWell(
          onTap: () => setState(() => _open = !_open),
          borderRadius: BorderRadius.circular(AppRadius.sm),
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: Space.s1,
              vertical: Space.s1,
            ),
            child: Row(
              children: <Widget>[
                Icon(
                  _open ? Icons.expand_less : Icons.expand_more,
                  size: 16,
                  color: colors.contentMuted,
                ),
                const SizedBox(width: Space.s1),
                Text(widget.title, style: theme.textTheme.labelLarge),
                const SizedBox(width: Space.s2),
                if (!_open && widget.summary != null)
                  Expanded(
                    child: Text(
                      widget.summary!,
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: colors.contentFaint,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
        if (_open)
          Padding(
            padding: const EdgeInsets.only(top: Space.s2),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: widget.children,
            ),
          ),
      ],
    );
  }
}

/// 未接上排序回调时的占位（空实现，不抛）。
void _noopReorder(int oldIndex, int newIndex) {}

class _LibraryManager extends StatefulWidget {
  const _LibraryManager({
    required this.items,
    required this.palette,
    required this.onReorder,
    required this.onRemove,
    required this.onRemoveMany,
    required this.onPreview,
  });

  final List<BackgroundItem> items;
  final AppPalette palette;
  final void Function(int oldIndex, int newIndex) onReorder;
  final ValueChanged<int>? onRemove;
  final ValueChanged<List<int>>? onRemoveMany;
  final ValueChanged<int>? onPreview;

  @override
  State<_LibraryManager> createState() => _LibraryManagerState();
}

class _LibraryManagerState extends State<_LibraryManager> {
  /// 选中的下标集合。**存下标不存项**：重排之后下标会变，
  /// 存项才能在重排后仍然指对东西——所以每次操作前按当前顺序重新解释。
  final Set<int> _selected = <int>{};

  /// 管理模式：开着才出现复选框与批量按钮。
  bool get _managing => _selected.isNotEmpty || _items >= 2;

  int get _items => widget.items.length;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);

    if (widget.items.isEmpty) {
      return Text(
        '背景库是空的 —— 下面挑一个内置图案，或添加一张自己的图。',
        style: theme.textTheme.labelSmall?.copyWith(color: colors.contentMuted),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        // 占用：一行说清楚「占了多少」，而不是等用户撞到上限。
        _UsageBar(items: widget.items),
        const SizedBox(height: Space.s2),
        // `ReorderableListView` 而不是 `Wrap`：拖动排序是内建能力
        // （自带拖动手柄、键盘可达、`onReorder` 回调），自己用
        // `LongPressDraggable` 拼一个既不完整又没有无障碍。
        ReorderableListView.builder(
          shrinkWrap: true,
          primary: false,
          buildDefaultDragHandles: false,
          padding: EdgeInsets.zero,
          itemCount: widget.items.length,
          onReorderItem: (int oldIndex, int newIndex) =>
              widget.onReorder(oldIndex, newIndex),
          proxyDecorator: _dragProxy,
          itemBuilder: (BuildContext context, int index) {
            final BackgroundItem item = widget.items[index];
            return _LibraryRow(
              key: ValueKey<String>(
                'lib-${item.kind}-${item.sameAs(widget.items.first) ? 'a' : ''}'
                '$index',
              ),
              index: index,
              item: item,
              palette: widget.palette,
              selected: _selected.contains(index),
              managing: _managing,
              onTap: () => _tap(index),
              onToggle: () => setState(() {
                if (!_selected.remove(index)) _selected.add(index);
              }),
              onRemove: widget.onRemove == null
                  ? null
                  : () => widget.onRemove!(index),
              dragHandle: _items > 1
                  ? ReorderableDragStartListener(
                      index: index,
                      child: Icon(
                        Icons.drag_indicator,
                        size: 16,
                        color: colors.contentFaint,
                      ),
                    )
                  : null,
            );
          },
        ),
        if (_selected.isNotEmpty) ...<Widget>[
          const SizedBox(height: Space.s2),
          Row(
            children: <Widget>[
              Text(
                '已选 ${_selected.length} 项',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: colors.contentMuted,
                ),
              ),
              const Spacer(),
              TextButton(
                onPressed: () => setState(_selected.clear),
                child: const Text('取消选择'),
              ),
              TextButton(
                onPressed: () {
                  widget.onRemoveMany?.call(_selected.toList()..sort());
                  setState(_selected.clear);
                },
                child: const Text('删除所选'),
              ),
            ],
          ),
        ],
      ],
    );
  }

  void _tap(int index) {
    if (_managing) {
      setState(() {
        if (!_selected.remove(index)) _selected.add(index);
      });
      return;
    }
    // 非管理模式：点一下就是「先看这张」。
    widget.onPreview?.call(index);
  }

  /// 拖动时的浮起外观（不缩放：缩放会让旁边的行跟着抖）。
  static Widget _dragProxy(
    Widget child,
    int index,
    Animation<double> animation,
  ) => Material(
    elevation: 0,
    color: Colors.transparent,
    child: Opacity(opacity: 0.9, child: child),
  );
}

/// 占用条：**明说**「占了多少」。
///
/// # 为什么它不再是一根进度条（2026-09-27）
///
/// 它原来对着一个 1.5 M 字符的「总预算」画进度——那个预算来自
/// localStorage 的 5 MB 配额。字节搬去 IndexedDB 之后**没有本地总预算了**
/// （配额由浏览器按磁盘剩余空间判），画一根进度条就得编一个分母，
/// 而那个分母是假的：明明只用了 2%，进度条也只到 2%，
/// 用户只会以为「我快用满了」而随手删图。
///
/// 所以现在只报**实际占用**（`N 项 · 12.4 MB`），
/// 真满了由 [BackgroundStore.put] 返回 `false`、界面如实说。
class _UsageBar extends StatelessWidget {
  const _UsageBar({required this.items});

  final List<BackgroundItem> items;

  static String _size(int bytes) {
    if (bytes < 1024 * 1024) return '${(bytes / 1024).round()} KB';
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final int bytes = DisplayPrefs.backgroundsBytes(items);
    final int images = items.whereType<BackgroundImage>().length;
    // 离 8 项上限只差一两项时提一句：那是**真的会拦住用户**的那一关。
    final bool nearCount = items.length >= kBackgroundMaxCount - 1;
    return Row(
      children: <Widget>[
        Text(
          images == 0
              ? '${items.length} 个内置图案（不占空间）'
              : '${items.length} 项 · 图片占用 ${_size(bytes)}',
          style: theme.textTheme.labelSmall?.copyWith(
            color: nearCount
                ? appPaletteOf(context).warning
                : colors.contentMuted,
          ),
        ),
        const Spacer(),
        Text(
          '上限 $kBackgroundMaxCount 项 / 单张 ${_size(kBackgroundImageMaxBytes)}',
          style: theme.textTheme.labelSmall?.copyWith(
            color: colors.contentFaint,
          ),
        ),
      ],
    );
  }
}

/// 库里的一行（缩略图 + 名字 + 勾选 + 拖动手柄 + 移除）。
class _LibraryRow extends StatelessWidget {
  const _LibraryRow({
    required super.key,
    required this.index,
    required this.item,
    required this.palette,
    required this.selected,
    required this.managing,
    required this.onTap,
    required this.onToggle,
    required this.onRemove,
    required this.dragHandle,
  });

  final int index;
  final BackgroundItem item;
  final AppPalette palette;
  final bool selected;
  final bool managing;
  final VoidCallback onTap;
  final VoidCallback onToggle;
  final VoidCallback? onRemove;
  final Widget? dragHandle;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s1),
      child: Material(
        color: selected
            ? palette.accent.withValues(alpha: 0.12)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(AppRadius.sm),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(AppRadius.sm),
          child: Padding(
            padding: const EdgeInsets.all(Space.s1),
            child: Row(
              children: <Widget>[
                if (managing)
                  Checkbox(
                    value: selected,
                    onChanged: (_) => onToggle(),
                    visualDensity: VisualDensity.compact,
                  )
                else
                  const SizedBox(width: Space.s2),
                ClipRRect(
                  borderRadius: BorderRadius.circular(AppRadius.xs),
                  child: SizedBox(
                    width: Space.s6,
                    height: Space.s6,
                    child: switch (item) {
                      BackgroundPattern(:final int id) => PatternPreview(
                        id: id,
                        colors: patternColorsFor(id, palette),
                        size: Space.s6,
                      ),
                      // 字节还没读回来时画占位块，而不是给 `Image.memory` 传 null
                      // （那会走 errorBuilder，静悄悄的）。
                      BackgroundImage(:final String? dataUrl) => dataUrl == null
                          ? ColoredBox(color: palette.surfaceAlt)
                          : _ImageTile(dataUrl: dataUrl, palette: palette),
                    },
                  ),
                ),
                const SizedBox(width: Space.s2),
                Expanded(
                  child: Text(
                    _label(item, index),
                    style: theme.textTheme.labelLarge,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                if (onRemove != null)
                  IconButton(
                    onPressed: onRemove,
                    tooltip: '移除这一项',
                    iconSize: 14,
                    visualDensity: VisualDensity.compact,
                    style: IconButton.styleFrom(
                      backgroundColor: palette.stage.withValues(alpha: 0.55),
                      foregroundColor: palette.ink,
                    ),
                    icon: const Icon(Icons.close),
                  ),
                if (dragHandle != null) ...<Widget>[
                  const SizedBox(width: Space.s1),
                  dragHandle!,
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }

  /// 一行的名字：图案用固定名，图片给「第 N 张 + 大小」。
  ///
  /// 为什么不显示文件名：它在导入时就被 `FileReader` 丢掉了，
  /// 界面上编一个「图 3」不算撒谎，但也不会更有用。
  static String _label(BackgroundItem item, int index) => switch (item) {
    BackgroundPattern(:final int id) => backgroundPatternLabel(id),
    BackgroundImage(:final String? dataUrl) => '图片 ${index + 1} · ${_size(dataUrl)}',
  };

  static String _size(String? dataUrl) {
    if (dataUrl == null) return '读取中';
    if (dataUrl.length < 1024 * 1024) {
      return '${(dataUrl.length / 1024).round()} KB';
    }
    return '${(dataUrl.length / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
}

/// 缩略图：走 [decodeDataUrlBytes]，坏图退回纯色面而不是红屏。
class _ImageTile extends StatelessWidget {
  const _ImageTile({required this.dataUrl, required this.palette});

  final String dataUrl;
  final AppPalette palette;

  @override
  Widget build(BuildContext context) {
    final Uint8List? bytes = decodeDataUrlBytes(dataUrl);
    if (bytes == null) return ColoredBox(color: palette.surfaceAlt);
    return Image.memory(
      bytes,
      fit: BoxFit.cover,
      // 缩略图只按 96 px 解码：不缩放的话一张 400 KB 的图会以原尺寸进缓存，
      // 8 张就是几十 MB。
      cacheWidth: kPatternPreviewSize.toInt(),
      gaplessPlayback: true,
      errorBuilder: (_, _, _) => ColoredBox(color: palette.surfaceAlt),
    );
  }
}

/// 3×3 位置选择垫：中心那格是当前值。
///
/// 为什么不是下拉：九个位置**看一眼就知道**，下拉要开一次再关一次。
class _AlignPad extends StatelessWidget {
  const _AlignPad({required this.value, required this.onChanged});

  final int value;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    final AppPalette palette = appPaletteOf(context);
    Widget cell(int index) {
      final bool selected = index == value;
      return Semantics(
        button: true,
        selected: selected,
        label: '位置 ${index + 1}',
        excludeSemantics: true,
        child: InkWell(
          onTap: () => onChanged(index),
          child: Container(
            width: Space.s5,
            height: Space.s5,
            decoration: BoxDecoration(
              color: selected
                  ? palette.accent.withValues(alpha: 0.55)
                  : colors.hoverWash,
              borderRadius: BorderRadius.circular(AppRadius.xs),
              border: Border.all(
                color: selected ? palette.accent : colors.hairline,
                width: selected ? 2 : 1,
              ),
            ),
          ),
        ),
      );
    }

    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s2),
      child: Row(
        children: <Widget>[
          Icon(Icons.crop_free, size: 16, color: colors.contentMuted),
          const SizedBox(width: Space.s2),
          for (int row = 0; row < 3; row++) ...<Widget>[
            for (int col = 0; col < 3; col++) ...<Widget>[
              cell(row * 3 + col),
              if (col < 2) const SizedBox(width: Space.s1),
            ],
            if (row < 2) const SizedBox(height: Space.s1),
          ],
        ],
      ),
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
