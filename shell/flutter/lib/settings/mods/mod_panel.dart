/// Mod 自有「产品面板」的扩展点（产品级加强波次）。
///
/// # 为什么要有这一层
///
/// 五个已注册 Mod 各有自己的产品级诉求（external-input 的计数、memory 的清空、
/// persona 的卡导入、director 的决策面板……）。如果每个都直接改
/// `sections/dev_tools_section.dart`，五条并行轨就必然在同一段 widget 树上冲突。
/// 于是把「每个 Mod 的产品面板」拆成**一个 Mod 一个文件**，由这里的注册表按 id 分派：
/// 各轨道只改自己那个文件，共享文件零改动。
///
/// # 契约
///
/// - `modId` 必须与 `ModDescriptor.id` 逐字一致；
/// - `stateLabels` 把该 Mod 的运行态字段翻成中文（未知 key 仍然不隐藏）；
/// - `build` 返回 null = 这个 Mod 不需要额外面板（只靠配置表单）；
/// - **注册了面板 = 运行态由该面板承担**：宿主不再渲染通用「运行态（只读）」
///   兜底块——那张块只服务**没有**专用面板的 Mod（2026-10-06 裁决，
///   见 `sections/dev_tools_section.dart` 的 `_stateBlock` 头注）。
library;

import 'package:flutter/foundation.dart' show ValueListenable;
import 'package:flutter/widgets.dart';

import '../../api/mods_api.dart';
import '../../api/settings_models.dart';
import '../../live2d/live2d_stage.dart' show PresetStatus;
import '../preset_labels.dart';
import 'director_debug_panels.dart' show DebugClock, PresetApply, PresetScaleApply;

/// director 卡片里「动作幅度」旋钮的接线（2026-10-09 从「外观与互动」搬来）。
///
/// 键名形如 action.models.id（doc 里不写尖括号，免得被当成 HTML）。
///
/// # 为什么数据不从 mods.json 走
///
/// 这三条仍是 [action] / [action.models.<id>]（live2d-ai.toml），
/// 仍走现有 PATCH 与 ActionScalesSyncer，**没有**写进 director 的 Mod 配置，
/// 也**没有**接回 action_tx。理由是归属：手势 / 表情 / 头部动作不是单 LLM
/// 主链的资产，强度旋钮跟产出这些动作的导演 Mod 放在一起。
///
/// # 为什么用「一个对象」而不是十二个字段
///
/// 面板上下文是共享只读面（见本文件头注的文件边界）：加一个可空对象，
/// 面板自己取字段，宿主只接线一次。
class ActionScalesWiring {
  const ActionScalesWiring({
    required this.action,
    this.modelOverrideEnabled = false,
    this.onHeadScaleChanged,
    this.onBodyScaleChanged,
    this.onExpressionScaleChanged,
    this.onModelOverrideEnabledChanged,
    this.onModelHeadScaleChanged,
    this.onModelBodyScaleChanged,
    this.onModelExpressionScaleChanged,
    this.onResetModelOverride,
    this.modelOverrideMessage,
    this.modelOverrideFailed = false,
  });

  /// 服务端权威的动作幅度视图（含 activeModelId 与逐模型覆盖表）。
  final ActionSettingsView action;

  /// 「本模型覆盖」开关的**值**（宿主给：= 本模型是否已有覆盖）。
  final bool modelOverrideEnabled;

  /// 三条**全局**滑条（走设置草稿 + 保存）。
  final ValueChanged<double>? onHeadScaleChanged;
  final ValueChanged<double>? onBodyScaleChanged;
  final ValueChanged<double>? onExpressionScaleChanged;

  /// 本模型覆盖（走直接 PATCH，不需要保存）。
  final ValueChanged<bool>? onModelOverrideEnabledChanged;
  final ValueChanged<double>? onModelHeadScaleChanged;
  final ValueChanged<double>? onModelBodyScaleChanged;
  final ValueChanged<double>? onModelExpressionScaleChanged;

  /// 「恢复跟随全局」：删掉本模型覆盖（models.id = null）。
  final VoidCallback? onResetModelOverride;

  /// 覆盖 PATCH 的结果（失败时带服务端给的 code：message）。
  final String? modelOverrideMessage;
  final bool modelOverrideFailed;
}

/// 导演卡片里「表情调试 / 动作调试 / 临时幅度」的接线（2026-10-09）。
///
/// 三块从核心「开发模式」页整块搬来（`settings/mods/director_debug_panels.dart`），
/// **数据与口径一字未改**：预设帧仍由前端直发渲染面（不经后端 / LLM），临时幅度
/// 仍只发渲染面、不落盘；`action_tx` 仍未接（`core-chain-baseline.md` §3.3）。
///
/// 为什么用一个对象挂在 [ModPanelContext] 上：面板是共享只读面，宿主只接线一次，
/// 面板自己取字段——与 [ActionScalesWiring] 同一条边界。
class DirectorDebugWiring {
  const DirectorDebugWiring({
    this.onApplyPreset,
    this.status,
    this.productScales,
    this.pinnedScales,
    this.onApplyScales,
    this.onClearScales,
    this.labels = PresetLabelTable.empty,
    this.clock,
  });

  /// 直发一条预设（none = 归零）。为空时按钮禁用（不假装能点）。
  final PresetApply? onApplyPreset;

  /// 舞台的本地预设状态（`Live2DStageState.presetStatus`）。
  final ValueListenable<PresetStatus?>? status;

  /// 服务端**产品设置**里的动作幅度（显示 + 作为「恢复」目标）。
  final ActionSettingsView? productScales;

  /// 当前**临时幅度覆盖**（ActionScalesSyncer.pinned；null = 没有）。只读、不落盘。
  final Map<String, double>? pinnedScales;

  /// 临时幅度覆盖（**不落盘**，只发渲染面）；产品设置才是真源。
  final PresetScaleApply? onApplyScales;

  /// 清掉临时覆盖并**强制**写回产品值。
  final VoidCallback? onClearScales;

  /// 预设 id → 中文展示名（读 `assets/actions/preset_labels.json`）。
  final PresetLabelTable labels;

  /// 可注入时钟（表情到点计时用）；null = `DateTime.now`。
  final DebugClock? clock;
}

/// 选一个**角色卡文件**的读取器（由组合根注入；见 `persona_panel.dart`）。
///
/// 为什么类型声明在这一层：`ModPanelContext` 要把它交给 persona 面板，而
/// `persona_panel.dart` 反过来 import 本文件——把类型放这里就不必造一个
/// 「面板元数据依赖具体面板」的环。
typedef PersonaCardFilePicker =
    Future<({String? dataUrl, String? error})> Function();

/// 面板可用的上下文：当前 Mod、运行态快照、以及两个**危险动作**回调。
///
/// 为什么用回调而不是直接给 `ModsApi`：与同层 `ModStateLoader` 同款——
/// 宿主接线一次，widget 测试注入 fake 即可覆盖「点清空 → 计数归零」这类交互，
/// 且面板不需要知道 base URL。
class ModPanelContext {
  const ModPanelContext({
    required this.mod,
    required this.state,
    required this.stateLoading,
    required this.stateError,
    required this.onRefreshState,
    required this.onCommand,
    this.activeSessionId,
    this.onModChanged,
    this.devMode = false,
    this.pickCardFile,
    this.actionScales,
    this.directorDebug,
  });

  /// 当前 Mod 的列表项（含 `enabled` / `config` / `settings_spec`）。
  final ModInfo mod;

  /// 最近一次运行态快照；`null` = 还没取到（或取失败）。
  final Map<String, Object?>? state;

  /// 运行态是否正在读取。
  final bool stateLoading;

  /// 运行态读取失败时的**可处置**文案（已带错误码）。
  final String? stateError;

  /// 重新拉一次运行态（清空/保存后刷新计数用）。
  final Future<void> Function() onRefreshState;

  /// `POST /api/v1/mods/{id}/command`：一次性动作（清空 / 导出 / 自检……）。
  final Future<ModCommandResult> Function(
    String command, [
    Map<String, Object?> args,
  ])
  onCommand;

  /// 该 Mod 当前是否启用（面板据此决定按钮是否可点）。
  bool get enabled => mod.enabled;

  /// **当前活动会话 id**（L1 会话绑定，2026-09-15）。
  ///
  /// 语义：这是用户**正看着**的那个会话（Flutter 的 ChatSessionStore.activeId）。
  /// persona / memory 的会话绑定动作默认应落到它上面。
  ///
  /// null = 还没有任何会话（首次打开、一条消息都没发）。此时面板**必须**
  /// 说清降级语义（「先发一条消息再绑定」/「将写入全局」），不能假装绑好了。
  final String? activeSessionId;

  /// **Mod 变更通知**：面板做了一次会改变运行行为的动作后调用
  /// [notifyChanged]，宿主据此弹统一的「需重新点火 / 重启后生效」提示。
  ///
  /// null = 宿主没接（纯 widget 测试）。面板**不该**依赖它存在。
  final ValueChanged<String>? onModChanged;

  /// 见 [onModChanged]；[what] 是过去式短语（例如「已导入角色卡并绑定到当前会话」）。
  void notifyChanged(String what) => onModChanged?.call(what);

  /// **开发模式**：决定面板里那些 `devKeys` 与调试块画不画（缺省 false = 不画）。
  ///
  /// 与卡片副标题（`ModsSection.devMode`）是**同一个开关**，由宿主一次传下来；
  /// 产品面（false）看不到测试注入 / 计数 / 唤醒词这类排障控件。
  final bool devMode;

  /// 角色卡文件选择器；null = 这个构建没接文件入口（persona 面板只留粘贴框）。
  ///
  /// 组合根一次注入（`shell_settings.dart` 的 `pickPersonaCardFile`），理由见
  /// [PersonaCardFilePicker] 与 `persona_panel.dart` 的头注。
  final PersonaCardFilePicker? pickCardFile;

  /// 动作幅度旋钮的接线（只有 director 面板消费；null = 不渲染那块）。
  ///
  /// 为什么住这里而不是面板自己取：面板不碰网络、不碰组合根，只画；
  /// 数据与回调由宿主（shell_settings.dart → ModsSection → 卡片）透传。
  final ActionScalesWiring? actionScales;

  /// 导演卡片里「表情调试 / 动作调试 / 临时幅度」的接线（只有 director 面板消费；
  /// null = 不渲染那块）。2026-10-09 从核心「开发模式」页整块搬来，仍只在
  /// [devMode] 为真时渲染。
  final DirectorDebugWiring? directorDebug;
}

/// 一个 Mod 的产品面板。
abstract class ModPanel {
  const ModPanel();

  /// 负责的 Mod id（= `ModDescriptor.id`）。
  String get modId;

  /// 该 Mod 运行态字段的中文标签（合并进统一的运行态渲染）。
  Map<String, String> get stateLabels => const <String, String>{};

  /// 这些配置 key 在**通用表单**里收进「高级」折叠（缺省空 = 全部平铺）。
  ///
  /// 为什么由面板声明而不是 schema 加字段：折叠是**呈现**决策，不是协议；
  /// 各 Mod 面板本来就是这个 Mod 的呈现所有者（见本文件头注的文件边界）。
  /// 没有它，语音输入那一屏会把 sidecar 路径 / ASR 命令 / 令牌全摊在主区。
  Set<String> get advancedKeys => const <String>{};

  /// 这些配置 key **产品面完全不渲染**（也**不**收进「高级」）。
  ///
  /// # 与 [advancedKeys] 的区别（2026-10-08）
  ///
  /// - [advancedKeys] = 「用户偶尔要用，但不想摊在主区」→ 折叠起来，**仍可改**；
  /// - [hiddenKeys] = 「用户看不懂、也不该由用户填」→ **产品界面一个都不画**。
  ///
  /// 键**仍然被解析、仍然写进 `mods.json`**：隐藏只发生在呈现层。保存表单时
  /// 这些键从服务端已有 config 原样带走（`_ModConfigTileState._buildConfig` 从
  /// 现有 config 出发、只覆盖表单字段），所以既有值不会丢。
  ///
  /// 场景：导演卡片里的词表映射与 `staging_*`（二路端点 / 模型 / 密钥变量名 /
  /// 超时）——模型已定死，这些不再当用户旋钮。
  Set<String> get hiddenKeys => const <String>{};

  /// 这些配置 key **只在开发模式里**出现在通用表单（产品面一个都不画）。
  ///
  /// # 与 [hiddenKeys] 的区别（2026-10-08）
  ///
  /// - [hiddenKeys] = 谁都看不到，**保存时原样带走**；
  /// - [devKeys] = 开发模式（`devMode == true`）时照常画在**通用表单**里
  ///   （不进「高级」折叠），产品面与 [hiddenKeys] 一样不画、保存时原样带走。
  ///
  /// 与 [advancedKeys] 的关键区别：`advancedKeys` 折起来**仍可展开改到**——
  /// 那样产品用户还是能改到这些键。[devKeys] 是**开关级**的门。
  Set<String> get devKeys => const <String>{};

  /// **已退役（2026-10-09）**：三级功能介绍全删，通用表单不再画面板声明的
  /// 字段说明。这个面**保留为空**（不再有生产 override），因为三个面板回归
  /// （external-input / voice-input / memory）仍按它断言「旧说明不再上屏」。
  ///
  /// # 历史（2026-10-08）
  ///
  /// 说明曾是**呈现**文案，不是协议字段；折叠（[advancedKeys]）、
  /// 「不画」（[hiddenKeys]）与它走同一条「面板声明呈现」的路。
  /// 新代码**不要**再 override 它。
  Map<String, String> get fieldHelp => const <String, String>{};

  /// 产品面板本体；返回 null = 不需要额外面板。
  Widget? build(BuildContext context, ModPanelContext ctx);
}
