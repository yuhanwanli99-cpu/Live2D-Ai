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

import 'package:flutter/material.dart';

import '../../api/settings_models.dart';
import '../../design/theme_id.dart';
import '../../design/tokens.dart';
import '../../settings/display_prefs.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
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
    this.onAddToPlaylist,
    this.onClearPlaylist,
    this.stageImageMessage,
    this.stageImageFailed = false,
    this.onPickShellImage,
    this.onClearShellImage,
    this.shellImageMessage,
    this.shellImageFailed = false,
    this.action,
    this.onHeadScaleChanged,
    this.onBodyScaleChanged,
    this.onExpressionScaleChanged,
    super.key,
  });

  /// 本地显示偏好（纯本地，`localStorage`，**不走设置草稿**）。
  final DisplayPrefs prefs;
  final ValueChanged<DisplayPrefs> onPrefsChanged;

  final bool devMode;

  /// 选/清舞台背景图。为 `null` 时按钮禁用（**不是**点了没反应）。
  final VoidCallback? onPickStageImage;
  final VoidCallback? onClearStageImage;

  /// 舞台背景轮播列表：把**当前**舞台图追加进列表 / 清空列表。
  ///
  /// 列表由用户手动维护（`DisplayPrefs.stagePlaylist`），图源全部来自本机
  /// 偏好；增删 / 排序在 [_StagePlaylistEditor] 里做，不依赖任何 Mod。
  final VoidCallback? onAddToPlaylist;
  final VoidCallback? onClearPlaylist;

  /// 上次选图的结果（超限时是**错误态**：图太大记不住）。
  final String? stageImageMessage;
  final bool stageImageFailed;

  /// 壳全局背景的选 / 清图（2026-09-14，rc.5）。同步开着时它们改的是
  /// **舞台那张图**（共用一份真相）；关掉才改壳自己的。
  final VoidCallback? onPickShellImage;
  final VoidCallback? onClearShellImage;
  final String? shellImageMessage;
  final bool shellImageFailed;

  // ── 动作幅度（2026-09-16，服务端产品设置） ──
  //
  // 与上面那些**纯本地 DisplayPrefs** 不同：这三项存在 `live2d-ai.toml` 的
  // `[action]` 段，走设置草稿 + 保存按钮（PATCH /api/v1/settings）。
  // 为 null 时整块不渲染（宿主还没拿到服务端设置）。
  final ActionSettingsView? action;
  final ValueChanged<double>? onHeadScaleChanged;
  final ValueChanged<double>? onBodyScaleChanged;
  final ValueChanged<double>? onExpressionScaleChanged;

  @override
  Widget build(BuildContext context) {
    // 这些参数实时下发给渲染面：**没有「保存」按钮**。
    // 理由：拖滑杆时就想看到舞台反应，多一步保存会毁掉这个手感。
    // 主题同理——点一下立刻整套换掉，正是它该有的手感。
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '外观',
          description: '四套配色任选。**只影响本机显示**，选完立刻生效，不需要保存。',
        ),
        ThemePickerField(
          value: prefs.theme,
          onChanged: (AppThemeId id) => onPrefsChanged(prefs.copyWith(theme: id)),
        ),
        _StageImageView(
          hasImage: prefs.stageImage != null,
          playlist: prefs.stagePlaylist,
          currentImage: prefs.stageImage,
          message: stageImageMessage,
          failed: stageImageFailed,
          onPick: onPickStageImage,
          onClear: onClearStageImage,
          onAddToPlaylist: onAddToPlaylist,
          onClearPlaylist: onClearPlaylist,
          // 删除 / 上移下移只改**本地列表**：宿主 `_updatePrefs` 是唯一落点，
          // 改完立即写入本机偏好并下发渲染面（见 shell_prefs.dart）。
          onPlaylistChanged: (List<String> next) =>
              onPrefsChanged(prefs.copyWith(stagePlaylist: next)),
        ),
        _ShellImageView(
          sync: prefs.syncShellStageBg,
          // 「有没有图」按**实际会画的那张**算（同步开时就是舞台那张）。
          hasImage: prefs.effectiveShellImage != null,
          message: shellImageMessage,
          failed: shellImageFailed,
          onSyncChanged: (bool v) =>
              onPrefsChanged(prefs.copyWith(syncShellStageBg: v)),
          onPick: onPickShellImage,
          onClear: onClearShellImage,
        ),
        const Divider(),
        const SectionHeader(
          title: '舞台与口型',
          description: '这些参数只影响**本机显示**，改动立刻下发到渲染面，不需要保存。',
        ),
        SliderField(
          label: '模型缩放',
          icon: Icons.zoom_out_map,
          value: prefs.scale,
          // **区间从这里读，不写字面量**（2026-09-11 修）：
          // 这两个数在 `DisplayPrefs` 里已经声明过一次（`clampScale` 也读它们），
          // 在 UI 里再写一遍就是第二个真源——改一处忘一处会让滑杆范围与
          // 实际 clamp 范围悄悄分叉。回归见 `test/display_prefs_test.dart`。
          min: DisplayPrefs.minScale,
          max: DisplayPrefs.maxScale,
          onChanged: (double v) => onPrefsChanged(prefs.copyWith(scale: v)),
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
        ),
        ToggleField(
          label: '口型同步',
          icon: Icons.record_voice_over_outlined,
          value: prefs.lipSync,
          onChanged: (bool v) => onPrefsChanged(prefs.copyWith(lipSync: v)),
          description: '关掉后声音照放，但嘴不动',
        ),
        ToggleField(
          label: '待机小动作',
          icon: Icons.self_improvement,
          value: prefs.idleEnabled,
          onChanged: (bool v) => onPrefsChanged(prefs.copyWith(idleEnabled: v)),
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
        if (action != null) ...[
          const Divider(),
          const SectionHeader(
            title: '动作幅度',
            description: '拖动**立刻在舞台上生效**（无需先保存）；'
                '「保存」才把值写进服务端（live2d-ai.toml 的 [action] 段），'
                '重开也还在。放弃改动 / 重新加载会立刻回到磁盘上的值。'
                '出厂 head 75% / body 80% / expression 100%。'
                '头摆太大就调小 head，身摆太小就调大 body。',
          ),
          SliderField(
            label: '头部摆幅',
            icon: Icons.face_retouching_natural,
            value: action!.headScale,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: onHeadScaleChanged != null,
            onChanged: onHeadScaleChanged ?? (double _) {},
            description: 'ParamAngle* 的倍率；出厂 75%',
          ),
          SliderField(
            label: '身体摆幅',
            icon: Icons.accessibility_new,
            value: action!.bodyScale,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: onBodyScaleChanged != null,
            onChanged: onBodyScaleChanged ?? (double _) {},
            description: 'ParamBodyAngle* 的倍率；出厂 80%，身/头比约 0.35',
          ),
          SliderField(
            label: '表情幅度',
            icon: Icons.mood,
            value: action!.expressionScale,
            min: ActionSettingsView.minScale,
            max: ActionSettingsView.maxScale,
            divisions: 46,
            enabled: onExpressionScaleChanged != null,
            onChanged: onExpressionScaleChanged ?? (double _) {},
            description: '口 / 眉 / 眼 的倍率；出厂 100%',
          ),
        ],
        const Divider(),
        const SectionHeader(
          title: '互动',
          description: '「允许拖动与缩放」管的是**拖拽 / 滚轮 / 双击复位**，'
              '与「点击角色有没有反应」无关。',
        ),
        ToggleField(
          label: '允许拖动与缩放',
          icon: Icons.pan_tool_outlined,
          value: prefs.allowDragZoom,
          onChanged: (bool v) =>
              onPrefsChanged(prefs.copyWith(allowDragZoom: v)),
          description: '关掉后模型不因拖动/滚轮移动；缩放仍可用舞台右下角的按钮',
        ),
        const SizedBox(height: Space.s3),
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

/// 壳全局背景那一块（2026-09-14，rc.5）。
///
/// 与 `_StageImageView` 一样的三条文案纪律：不是二选一、失败要说实话、
/// 按钮是文字。多一条**同步语义**必须写清楚——「与舞台同步」开着时，
/// 这里的选 / 清图和上面「舞台背景图」是同一份数据，不是各存一张。
class _ShellImageView extends StatelessWidget {
  const _ShellImageView({
    required this.sync,
    required this.hasImage,
    required this.message,
    required this.failed,
    required this.onSyncChanged,
    required this.onPick,
    required this.onClear,
  });

  final bool sync;
  final bool hasImage;
  final String? message;
  final bool failed;
  final ValueChanged<bool> onSyncChanged;
  final VoidCallback? onPick;
  final VoidCallback? onClear;

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
          Text('壳背景', style: theme.textTheme.labelLarge),
          const SizedBox(height: Space.s1),
          EmphasizedText(
            '聊天与侧栏背后的**整壳背景**，固定 15% 透明度铺一层，'
            '**不做**透明度滑条；底色仍跟主题走。开启「与舞台同步」时与舞台共用'
            '同一张图，关掉才用壳自己的图。',
            style: theme.textTheme.labelSmall?.copyWith(
              color: colors.contentMuted,
            ),
          ),
          const SizedBox(height: Space.s2),
          ToggleField(
            label: '与舞台同步',
            icon: Icons.sync,
            value: sync,
            onChanged: onSyncChanged,
            description: sync ? '壳画的就是舞台那张图（一份真相）' : '壳有自己的一张图',
          ),
          const SizedBox(height: Space.s2),
          Wrap(
            spacing: Space.s2,
            runSpacing: Space.s2,
            children: <Widget>[
              FilledButton.tonal(
                onPressed: onPick,
                child: Text(hasImage ? '换一张壳背景图' : '选择壳背景图'),
              ),
              if (hasImage)
                TextButton(onPressed: onClear, child: const Text('清除壳背景')),
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
        ],
      ),
    );
  }
}
