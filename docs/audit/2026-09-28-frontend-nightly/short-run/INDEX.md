# INDEX（文件 → 批次 → 一句结论）

> 覆盖率看这里。前端源文件总数 = lib 91 个 .dart + test ~95 + web/ + tool/ + pubspec.yaml。

## lib/
| 文件 | 批次 | 结论 |
|---|---|---|
| lib/main.dart | 0001 | 组合根；F1/F2/F3/F4 落点在此；导演 cue 调用点完好（P 红线 ✓） |
| lib/app/app_shell.dart | 0001 | 保活/垫层/背景判据一处齐；死参数 F5；clamp 兜底不崩 |
| lib/app/shell_prefs.dart | 0001 | _applyPrefs 唯一下发漏斗 ✓；预览不驱动 slideshow（F2 派生） |
| lib/app/shell_settings.dart | 0001 | pane 接线合理；presetStatus 走 ValueNotifier 无 stale；_loadAdmin 非 ApiException 候选→B7 |
| lib/app/shell_admin.dart | 0001 | 密钥不回显纪律 ✓；catch 面待 B7 复核 |
| lib/app/shell_chat.dart | 0001 | 正常 |
| lib/app/nav_host.dart | 0001 | 垫层在 dock 内 ✓；裸 AppRadius.xl 归 tokens 批 |
| lib/app/page_cross_fade.dart | 0001 | Offstage+iframe 已证伪（slots 机制）；实现与注释一致 |
| lib/app/collapsible_panel.dart | 0001 | 四路关断齐全 |
| lib/app/browser_io.dart | 0001 | 存储三态/永不抛 ✓；无泄密无外链 |
| lib/app/app_shortcuts.dart | 0001 | 正常（ASCII Cmd） |
| lib/app/shortcut_help_dialog.dart | 0001 | 正常 |
| lib/ui/stage_host.dart | 0001 | 覆盖层/语义/垫层齐；progress 参数受 F4 牵连 |
| lib/ui/error_actions.dart | 0001 | 按码分流 ✓；但 goto 缺「开面板」（F1） |
| lib/ui/shell_slideshow.dart | 0001 | 自身幂等/jumpTo 完好——问题是生产没人调 jumpTo（F2） |
| lib/api/api_client.dart | 0001(部分) | _guard 全包 ApiException ✓；解析层待 B7 全审 |
| lib/live2d/live2d_host_web.dart | 0001(部分) | src=/render 同源 ✓；postMessage origin 校验 ✓；全审归 B9 |
| lib/live2d/live2d_bridge.dart | 0001(局部) | progress 有 notify 无听众（F4）；全审归 B9 |
| lib/live2d/live2d_stage.dart | 0001(局部) | _onBridgeChanged 不转发 progress（F4 链）；sendStageBg 无去重（F-0002-1 链）；全审归 B9 |
| lib/settings/display_prefs.dart | 0002 | 22 字段五件套齐全；clamp 纪律一致；DEC-1/5/6 现场核对（已登记） |
| lib/design/background_item.dart | 0002 | id-only 落盘/内容哈希跨端稳定 ✓；isRenderable 判据单一 |
| lib/data/background_store.dart | 0002 | 三态读契约正确；probe 默认保守 ✓ |
| lib/data/background_store_web.dart | 0002 | 永不抛/超时/oncomplete 纪律 ✓；open 失败永久降级（安全侧） |
| lib/data/background_store_stub.dart | 0002 | 语义定义 ✓ |
| lib/data/background_hydration.dart | 0002 | P0-1/P0-2 防线在位且有故障注入测试；但见 F-0002-2 窗口 |
| lib/data/background_reorder.dart | 0002 | P0-3 已修，语义与 SDK 一致 |
| lib/ui/audio_bar.dart | 0002(局部) | 滑杆 onChanged 直连落盘（F-0002-1 链）；语义标注正规 |
| lib/ui/field_row.dart | 0003(关键段) | FieldActionRow 成功结果不渲染（F-0003-2）；_FieldShell 仅错误槽；description 走 EmphasizedText ✓ |
| lib/ui/shell_backdrop.dart | 0003 | _imageLayer 每 build 重解码（F-0003-1 P1）；decodeDataUrlBytes 永不抛但无缓存 |
| lib/settings/sections/appearance_section.dart | 0003 | 多选存下标删错图（F-0003-3）；D1/DEC-2/6/7 现场=已登记；滑杆族=F-0002-1 实例 |
| lib/settings/sections/dev_tools_section.dart | 0003 | DebugPanels 计时器/监听对称 ✓；_ModConfigTile re-seed 吞草稿（F-0003-6）；归零分叉（F-0003-5）；诊断无泄密 ✓ |
| lib/settings/sections/llm_section.dart · tts_section.dart | 0003 | 自检成功无显示 + 文案猜错误（F-0003-2）；红线（只读静音/无 response_format/不叫试听）在位 ✓ |
| lib/settings/sections/env_key_field.dart | 0003 | 安全面 ✓（obscure/即清/永不回显）；星号上屏（F-0003-4） |
| lib/settings/sections/director_observer_section.dart | 0003 | feed 不落盘；stage-clock 灌环已被 D44 防住（证伪一条候选） |
| lib/settings/sections/persona_section.dart · pane_helpers.dart | 0003 | 正常 ✓ |
| lib/settings/settings_controller.dart | 0004 | 改动集/Tri/prune/失败保草稿 15 字段齐全——**健全，无发现** |
| lib/settings/settings_sections.dart | 0004 | 恒定分区清单；dev_mode 死循环教训在位 ✓ |
| lib/settings/mods/mod_panel.dart · mod_panels.dart | 0004 | 扩展点契约清晰（不给裸 API）；注释 sed 残留（F-0004-3） |
| lib/settings/mods/memory_panel.dart | 0004 | **切会话桶列表不刷新 → 跨桶编辑/删除（F-0004-1 P2）**；失败清草稿（F-0004-2） |
| lib/settings/mods/persona_panel.dart | 0004 | 会话守卫双保险（禁用+契约守卫）✓；作用域文案不混 |
| lib/settings/mods/voice_input_panel.dart | 0004 | 4 控制器 dispose 齐；自检不回 token 明文 ✓；正式审计在 B9/B10 |
| lib/settings/mods/external_input_panel.dart | 0004 | 带码文案/确认对话框/「不禁用取现场证据」都说得出理由 ✓ |
| lib/settings/mods/director_panel.dart | 0004 | 瘦身版；labels best-effort ✓ |
| lib/settings/mods/persona_card_picker*.dart | 0004 | 条件导入理由充分；web 侧复用 pickFileDataUrl（注入面在 B11 复核） |

## test/（B4 抽查）
| test/memory_panel_test.dart | 0004 | 真交互（确认框走真实 tap）；无「切桶刷新」用例 → F-0004-1 |
| test/persona_panel_test.dart / voice_input_panel_test / external_input_panel_test / mods_section_test / settings_controller_test | 0004 | 全部真交互断言，**假绿灯模式扫描零命中**（expect(常量,常量)/isNotNull 空转/源码字符串扫描） |

## test/（B3 抽查）
| test/field_row_test.dart | 0003 | 动作行只测 busy+错误槽——成功态零用例（覆盖缺口并入 F-0003-2） |
| test/visual_language_test.dart | 0003 | 星号扫描=窗口式（Text 字面实参），变量通道是盲区 → F-0003-4 |
| test/asset_guard_tts_config_test.dart | 0003 | 行为断言真可失败（点一次回调一次/无试听/草稿隔离） |
| test/director_observer_test.dart | 0003(局部) | 持久化红线=标识符交集守卫，真断言 |

## test/
| 文件 | 批次 | 结论 |
|---|---|---|
| test/stage_keepalive_test.dart | 0001(局部) | initState 计数真断言；DOM 级由 flutter≥2.5 slots 机制兜底 |
| test/asset_guard_director_cue_test.dart | 0001 | 非空转：剥注释+判别力自证；扫 lib/main.dart 现状吻合 |
| test/transient_results_test.dart | 0001(局部) | 源码扫描式守卫（钉 _gotoSection 单入口+清结果），与 action_cue_test 旧弱断言互补 |
| test/app_shell_layout_test.dart / compact_settings_page_test.dart | 0001(局部) | Offstage 保活断言真实可失败 |
| test/background_logic_test.dart | 0001(局部) | jumpTo 单测存在但生产未接（F2 空档） |
| （错误出路路径） | 0001 | **零测试覆盖**（errorActionsFor/ErrorAction 全仓无测试命中）→ F1 |
| test/display_prefs_test.dart | 0002(抽查) | 192 条具体断言、toJson 无 dataUrl 有钉；非空转 |
| test/background_store_test.dart | 0002 | 故障注入矩阵真实可失败（P0-1/P0-2 回归在位）|
| test/background_reorder_test.dart | 0002(存在性) | P0-3 回归位点 |
| （store 置换窗口时序） | 0002 | **零覆盖**（main 的 _store 换接无测试）→ F-0002-2 |

## web/ + pubspec + tool/
（尚无）
