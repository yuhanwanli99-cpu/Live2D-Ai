/// voice-input 的产品面板（2026-10-08 二次口径）。
///
/// # 这一版有什么
///
/// 1. 卡片标题行的启用开关（由 `ModsSection` 画）；
/// 2. 一句话：点「听」→ 说完再点一次，字进输入框；
/// 3. **9 个配置键全部进 [devKeys]**：产品面一个都不画，`devMode == true`
///    时按普通表单画出来（**不塞进「高级」**）。
///
/// # 为什么是 devKeys 而不是 hiddenKeys
///
/// 上一版把这 9 个键整体藏起来。现在产品路径（点「听」→ 定稿进输入框）
/// **不打** `/api/v1/voice/transcript`，所以唤醒词 / 手动闸 / 后端 / locale /
/// 令牌 / sidecar 那几项都只剩排障与集成用途——它们该在开发模式里可见、可改，
/// 但不该出现在产品面。
///
/// 键仍被解析、`mods.json` 里已有的值保存时原样带走（`devMode == false` 时
/// 与 [ModPanel.hiddenKeys] 同一条路径，见 `dev_tools_mod_config.dart` 的
/// `_buildConfig`）。
///
/// 本文件由 voice-input 轨道独占。
library;

import 'package:flutter/material.dart';

import '../../design/tokens.dart';
import '../../ui/inline_notice.dart';
import 'mod_panel.dart';

/// 配置键（与 `voice_input_settings_spec()` 逐字一致）。
const String kVoiceWakePhraseKey = 'wake_phrase';
const String kVoiceManualKey = 'manual_enabled';

class VoiceInputPanel extends ModPanel {
  const VoiceInputPanel();

  @override
  String get modId => 'voice-input';

  /// 9 个键：产品界面**一个都不画**，开发模式里按普通表单画（见头注）。
  ///
  /// 与 `voice_input_settings_spec()` 的字段逐键对应，**一个不漏**：
  /// 唤醒词 / 手动闸 / 识别后端 / 归一化语言 / 令牌 / sidecar 的四项。
  @override
  Set<String> get devKeys => const <String>{
    kVoiceWakePhraseKey,
    kVoiceManualKey,
    'backend',
    'locale',
    'token',
    'sidecar_script',
    'sidecar_url',
    'sidecar_transcriber',
    'sidecar_python',
  };

  // 2026-10-09：fieldHelp 已退役（三级功能介绍全删）。

  /// 运行态字段的中文标签（本面板不渲染运行态；留给通用兜底块的键名翻译）。
  @override
  Map<String, String> get stateLabels => const <String, String>{
    'wake_gate_open': '唤醒词已设置',
    'wake_phrase_set': '唤醒词是自定义的',
    'manual_enabled': '手动提交可用',
    'sidecar_status': '语音后端状态',
  };

  @override
  Widget? build(BuildContext context, ModPanelContext ctx) => const Padding(
    padding: EdgeInsets.only(top: Space.s3),
    child: InlineNotice(
      message: '点聊天栏的「听」。说完再点一次，字会出现在输入框里。',
      severity: NoticeSeverity.info,
      dense: true,
    ),
  );
}
