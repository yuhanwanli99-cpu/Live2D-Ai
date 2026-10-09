/// 「模型服务」页（2026-10-09 新一级）。
///
/// 原一级「对话」与一级「语音合成」**合成这一项**：页内两组
/// （**语言模型** / **语音合成**），每组内部再按普通 / 开发者的层级展开
/// （语言模型没有 dev-only 字段；语音合成的开发者层只有采样率、声道、
/// 服务端静音）。字段语义**一字不改**，只是换了容器与归属。
///
/// # 为什么用 GroupCard 而不是新设计系统
///
/// 口径：用现成的 SectionHeader / GroupCard。这里只负责「把两段摆进两张卡」，
/// 字段逻辑仍住在 llm_section.dart / tts_section.dart（**没有第二套**）。
///
/// # 定位到「语音合成」这一组
///
/// 错误横幅的「去语音合成设置」不能靠滚动碰运气：它带一个 focusGroup 值，
/// 本页拿到 kServiceVoiceGroup 时用 GlobalKey + Scrollable.ensureVisible
/// 把语音那张卡滚进视口。
library;

import 'package:flutter/material.dart';

import '../../api/env_api.dart';
import '../../api/settings_models.dart';
import '../../design/tokens.dart';
import '../../ui/field_row.dart';
import '../../ui/group_card.dart';
import '../../ui/section_header.dart';
import '../../ui/soft_motion.dart';
import '../settings_controller.dart';
import '../settings_sections.dart';
import 'llm_section.dart';
import 'tts_section.dart';

class ServiceSection extends StatefulWidget {
  const ServiceSection({
    required this.controller,
    required this.view,
    required this.devMode,
    this.serverMuted = false,
    this.llmEnvKey,
    this.ttsEnvKey,
    this.envFile,
    this.onSaveKey,
    this.onTestLlm,
    this.onTestTts,
    this.llmTestResult,
    this.ttsTestResult,
    this.llmTesting = false,
    this.ttsTesting = false,
    this.focusGroup,
    super.key,
  });

  final SettingsController controller;
  final SettingsView view;
  final bool devMode;

  /// 服务端静音观测值（只读，透传给语音合成组）。
  final bool serverMuted;

  /// 两段各自的密钥键状态（GET /api/v1/env）。
  final EnvKey? llmEnvKey;
  final EnvKey? ttsEnvKey;
  final String? envFile;

  /// 保存密钥（PUT /api/v1/env）。
  final Future<void> Function(String key, String value)? onSaveKey;

  /// 两个连通性自检。
  final Future<void> Function()? onTestLlm;
  final Future<void> Function()? onTestTts;
  final FieldTestResult? llmTestResult;
  final FieldTestResult? ttsTestResult;
  final bool llmTesting;
  final bool ttsTesting;

  /// 进来时要定位到的组（kServiceVoiceGroup = 语音合成）；null = 不定位。
  final String? focusGroup;

  @override
  State<ServiceSection> createState() => _ServiceSectionState();
}

class _ServiceSectionState extends State<ServiceSection> {
  final GlobalKey _voiceGroupKey = GlobalKey();

  @override
  void initState() {
    super.initState();
    _focusIfNeeded();
  }

  @override
  void didUpdateWidget(ServiceSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.focusGroup != oldWidget.focusGroup) _focusIfNeeded();
  }

  /// 把 focusGroup 指的那一组滚进视口（**用 Key，不靠滚动碰运气**）。
  void _focusIfNeeded() {
    if (widget.focusGroup != kServiceVoiceGroup) return;
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (!mounted) return;
      final BuildContext? ctx = _voiceGroupKey.currentContext;
      if (ctx == null) return;
      Scrollable.ensureVisible(
        ctx,
        // 时长过 appMotion 闸门（「减少动画」时归零）；直接给令牌会被
        // motion_wiring_test 判红——那是刻意的（漏过闸门不报错，只在开了
        // 减少动画的用户那里表现为「说好的不动，结果它还在动」）。
        duration: appMotion(context, AppDurations.reveal),
        alignment: 0.0,
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(title: '模型服务'),
        GroupCard(
          title: '语言模型',
          child: LlmSection(
            controller: widget.controller,
            view: widget.view,
            devMode: widget.devMode,
            envKey: widget.llmEnvKey,
            envFile: widget.envFile,
            onSaveKey: widget.onSaveKey,
            onTest: widget.onTestLlm,
            testResult: widget.llmTestResult,
            testing: widget.llmTesting,
          ),
        ),
        GroupCard(
          key: _voiceGroupKey,
          title: '语音合成',
          child: TtsSection(
            controller: widget.controller,
            view: widget.view,
            devMode: widget.devMode,
            serverMuted: widget.serverMuted,
            envKey: widget.ttsEnvKey,
            envFile: widget.envFile,
            onSaveKey: widget.onSaveKey,
            onTest: widget.onTestTts,
            testResult: widget.ttsTestResult,
            testing: widget.ttsTesting,
          ),
        ),
      ],
    );
  }
}
