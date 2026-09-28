# BATCH-0004 — Phase 1 · Mod 面板域 + 设置控制器（lib/settings/mods/ 全量 + controller + sections）

## 本批读过（全读）
| 文件 | 行 | 一句话 |
|---|---|---|
| lib/settings/settings_controller.dart | 409 | 改动集+Tri+prune+失败保草稿——**健全，无发现**（三要点逐一核对） |
| lib/settings/settings_sections.dart | 68 | 恒定分区清单（dev_mode 死循环教训在位）✓ |
| lib/settings/mods/mod_panel.dart | 109 | 注册表契约清晰；ModPanelContext 不给裸 API ✓ |
| lib/settings/mods/mod_panels.dart | 36 | 注册表 ✓；注释有 `@@final@@` sed 残留（F-0004-3） |
| lib/settings/mods/persona_card_picker*.dart | 41 | 条件导入理由充分；stub 如实说不可用 ✓ |
| lib/settings/mods/external_input_panel.dart | 483 | 带码文案/确认对话框/「不禁用取现场证据」的设计都说得出理由 ✓ |
| lib/settings/mods/director_panel.dart | 372 | 瘦身版；labels best-effort ✓ |
| lib/settings/mods/persona_panel.dart | 608 | 会话守卫双保险（禁用按钮+契约守卫）✓；作用域文案不混 ✓ |
| lib/settings/mods/voice_input_panel.dart | 610 | 4 控制器 dispose 齐；自检不回 token 明文 ✓ |
| lib/settings/mods/memory_panel.dart | 694 | 列表行用 id 做 Key（比外观库长进）；但**桶切换不刷新列表**（F-0004-1）+ 输入反馈两瑕疵（F-0004-2） |
| 补读 | — | dev_tools secret 字段 obscure 渲染(:1005-1008) · shell_admin._notifyModChanged→_loadAdmin 链(:214/241) · activeSessionId 接线(shell_settings:355) · 五个面板测试文件用例计数+假绿灯模式扫描（零命中） |

## 发现
- **F-0004-1（P2）** memory 面板记忆列表不随会话桶切换刷新：面板 State 无 didUpdateWidget，`_records` 只在 initState/手动刷新时取；`ctx.activeSessionId` 是活值（切会话后面板文案立刻说「当前会话桶 B」，列表却还是 A 的），行内「编辑/删除」随即变成**跨桶 id 操作**。
- **F-0004-2（P3）** memory 面板输入反馈两瑕疵：导入失败仍清空输入框（违「失败不清草稿」家法）；_send busy 时后续动作静默吞。
- **F-0004-3（P3）** mod_panels.dart 注释 sed 残留 `@@final@@`。

## 本批核对过、不成发现的（正面记录）
- settings_controller：`dirty` 判据=编译补丁为空（非「碰没碰过」）；保存用服务端回填 view；失败保草稿；pruneAgainstRemote 全 15 字段+浮点容差；ApplyStatus.unknown 保守按需重启。
- 五面板统一纪律在位：错误全带码且码进文案（用户拿码搜日志）；每个 await 后 mounted 守卫；TextEditingController 全 dispose；busy 同步置位防连点；`unawaited` 使用规范。
- 密钥面：secret 字段 obscure 渲染 + 留空不提交 + 自检结果只报 token_set 布尔（external_input tokenStatusText 同样不回显）——红线 G 在本域无泄漏。
- persona：没有活动会话时按钮禁用 + `_runForSession` 契约守卫双保险（注释明言「接线错也不静默退化全局」）——好设计。
- voice：`voiceTranscriptUrl` scheme/host 校验；sidecar 逐参数不过 shell（宿主侧）。
- F-0003-6 触发链**实证**：任一 Mod 命令成功 → _notifyModChanged + _loadAdmin（shell_admin.dart:214-215, 239-241）→ _ModConfigTile 按 identical 重灌表单——用户在 A 面板填表、在 B 面板点个操作，草稿就没了。
- 假绿灯扫描：memory/persona/controller 测试全部真交互（确认对话框走真实 tap、args 录制断言）；`isNotNull, reason:` 两处为按钮态断言非空转。
- settings_sections 的 appearance.description 仍写「展台图」（措辞陈旧半档，不到成发现门槛，记此备忘）。

## 本批未核实
- memory record `id` 是否桶内序列（若全局唯一则 F-0004-1 的跨桶误删概率降为「必然显示错、偶尔删错」）——Rust 侧，范围外。
- Mod 面板在设置面板 Offstage 关闭再打开时 State 是否总是保留（B1 证过 Offstage 保活，未对 ExpansionTile 内层逐个实测）。
