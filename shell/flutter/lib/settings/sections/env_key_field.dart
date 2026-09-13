/// 密钥写入行（rc.2 2026-09-12）：把 API key 写进 `.env`。
///
/// # 它为什么存在
///
/// rc.2 之前密钥**只能在**后端进程环境里：界面上能改模型名，却改不了 key。
/// 用户改完配置还是 401，且没有任何地方能改——这是「改不动」的那一类缺陷。
///
/// # 与其它字段的区别（安全边界）
///
/// - 值**只往上走**：`PUT /api/v1/env`，响应只回 `{key, set}`；
/// - **没有回显**：本控件不持有、不请求、不显示当前值，只显示「已设置/未设置」；
/// - 输入框是 `obscureText`，并且**每次保存后立即清空**（不留明文在内存里等 GC）。
///
/// 键名不是用户随手填的：它来自 `GET /api/v1/env`（即配置里 `api_key_env`
/// 指向的名字），所以不存在「界面上写了一个后端不读的变量名」这种静默失效。
library;

import 'package:flutter/material.dart';

import '../../api/env_api.dart';
import '../../design/tokens.dart';
import '../../ui/field_row.dart';
import '../../ui/theme.dart';

class EnvKeyField extends StatefulWidget {
  const EnvKeyField({
    required this.sectionLabel,
    required this.status,
    required this.onSave,
    this.debugHint,
    super.key,
  });

  /// 人话段名（「对话模型」/「语音合成」），用于文案。
  final String sectionLabel;

  /// 该段的键状态（`null` = 配置里没声明 `api_key_env`）。
  final EnvKey? status;

  /// 保存回调：`(key, value)`。空值 = 删除。
  final Future<void> Function(String key, String value)? onSave;

  /// 排障提示（`.env` 路径），只在需要时显示。
  final String? debugHint;

  @override
  State<EnvKeyField> createState() => _EnvKeyFieldState();
}

class _EnvKeyFieldState extends State<EnvKeyField> {
  final TextEditingController _value = TextEditingController();
  bool _saving = false;
  String? _message;

  @override
  void dispose() {
    _value.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final EnvKey? status = widget.status;
    final Future<void> Function(String, String)? save = widget.onSave;
    if (status == null || save == null) return;
    final String value = _value.text.trim();
    setState(() {
      _saving = true;
      _message = null;
    });
    try {
      await save(status.key, value);
      if (!mounted) return;
      setState(() {
        _saving = false;
        // 立刻清掉明文：它已经写进 .env，没有理由继续留在输入框里。
        _value.clear();
        _message = value.isEmpty
            ? '已清除 ${status.key}'
            : '已写入 ${status.key}，**立即生效**（不用重启）';
      });
    } catch (error) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _message = '保存失败：$error';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final EnvKey? status = widget.status;

    if (status == null) {
      return ReadonlyField(
        label: '${widget.sectionLabel}密钥',
        icon: Icons.key_off_outlined,
        text: '未绑定环境变量',
        description: '配置里没声明 api_key_env；本地端点通常不需要密钥。'
            '要绑定就在「开发者选项」里填变量名。',
      );
    }

    final bool canSave = !_saving && widget.onSave != null;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: Space.s2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(Icons.key_outlined, size: 18, color: colors.contentMuted),
              const SizedBox(width: Space.s2),
              Expanded(
                child: Text(
                  '${widget.sectionLabel}密钥',
                  style: theme.textTheme.bodyMedium,
                ),
              ),
              // 状态徽标：只表意，不靠颜色（无障碍约定）。
              Text(
                status.set ? '已设置' : '未设置',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: status.set ? colors.contentMuted : theme.colorScheme.error,
                ),
              ),
            ],
          ),
          const SizedBox(height: Space.s1),
          Row(
            children: <Widget>[
              Expanded(
                child: TextField(
                  controller: _value,
                  enabled: canSave,
                  obscureText: true,
                  autocorrect: false,
                  enableSuggestions: false,
                  decoration: InputDecoration(
                    isDense: true,
                    hintText: status.set ? '输入新值以替换（留空保存 = 清除）' : '粘贴密钥',
                    helperText: status.key,
                  ),
                  onSubmitted: (_) {
                    if (canSave) _save();
                  },
                ),
              ),
              const SizedBox(width: Space.s2),
              FilledButton.tonal(
                onPressed: canSave ? _save : null,
                child: Text(_saving ? '保存中…' : '保存密钥'),
              ),
            ],
          ),
          const SizedBox(height: Space.s1),
          Text(
            '写进 .env（密钥永不回显/永不下发）；这就是 [${status.key}] 的值。'
            '${widget.debugHint == null ? '' : '文件：${widget.debugHint}'}',
            style: theme.textTheme.bodySmall?.copyWith(color: colors.contentMuted),
          ),
          if (_message != null)
            Padding(
              padding: const EdgeInsets.only(top: Space.s1),
              child: Text(
                _message!,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: _message!.startsWith('保存失败')
                      ? theme.colorScheme.error
                      : colors.contentMuted,
                ),
              ),
            ),
        ],
      ),
    );
  }
}
