/// 密钥写入行（rc.2 2026-09-12）：把 API key 写进 `.env`。
///
/// # 它为什么存在
///
/// rc.2 之前密钥**只能在**后端进程环境里：界面上能改模型名，却改不了 key。
/// 用户改完配置还是 401，且没有任何地方能改——这是「改不动」的那一类缺陷。
///
/// # 与其它字段的区别（安全边界）
///
/// - 值**只往上走**：`PUT /api/v1/env`，响应回 `{key, set, apply_status}`；
/// - **没有回显**：本控件不持有、不请求、不显示当前值，只显示「已设置/未设置」；
/// - 输入框是 `obscureText`，并且**每次保存后立即清空**（不留明文在内存里等 GC）。
///
/// 键名不是用户随手填的：它来自 `GET /api/v1/env`（即配置里 `api_key_env`
/// 指向的名字），所以不存在「界面上写了一个后端不读的变量名」这种静默失效。
library;

import 'package:flutter/material.dart';

import '../../api/env_api.dart';
import '../../api/settings_models.dart';
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
  ///
  /// 返回**写盘结果**（[EnvWriteResult]）而不只是 `void`：界面要按 `applyStatus`
  /// 分两句说（S-002）——写进 `.env` 是一件事，引擎装上没有是另一件事。
  final Future<EnvWriteResult> Function(String key, String value)? onSave;

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
    final Future<EnvWriteResult> Function(String, String)? save = widget.onSave;
    if (status == null || save == null) return;
    final String value = _value.text.trim();
    setState(() {
      _saving = true;
      _message = null;
    });
    try {
      final EnvWriteResult result = await save(status.key, value);
      if (!mounted) return;
      setState(() {
        _saving = false;
        // 立刻清掉明文：它已经写进 .env，没有理由继续留在输入框里。
        _value.clear();
        _message = _writeMessage(status.key, value.isEmpty, result.applyStatus);
      });
    } catch (error) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _message = '保存失败：$error';
      });
    }
  }

  /// 写盘与生效**分开说**（S-002）：只有 `applied` 才配「立即生效」。
  ///
  /// 从前无论后端能不能装上，这里都写死「**立即生效**（不用重启）」——
  /// 密钥落进了 `.env`、链路却还拿着旧 client 时，这句话是假的。
  static String _writeMessage(String key, bool cleared, ApplyStatus apply) {
    final String wrote = cleared ? '已清除 $key' : '已写入 $key';
    if (apply == ApplyStatus.applied) {
      return cleared ? wrote : '$wrote，**立即生效**（不用重启）';
    }
    final String note = switch (apply) {
      ApplyStatus.applied => '',
      ApplyStatus.restartRequired => '引擎没装上这份改动，需要重启服务后才生效',
      // S-001（2026-10-16）：热重载失败——写盘成功、引擎**仍是旧配置**。
      ApplyStatus.reloadFailed => '引擎没能用上这份改动，仍是旧配置',
      ApplyStatus.queued => '正在生效中',
      ApplyStatus.noSupervisor => '当前没有对话引擎，下次启动后生效',
      ApplyStatus.unknown => '无法确认是否已生效，必要时重启服务',
    };
    return '$wrote；$note';
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
                  color: status.set
                      ? colors.contentMuted
                      : theme.colorScheme.error,
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
