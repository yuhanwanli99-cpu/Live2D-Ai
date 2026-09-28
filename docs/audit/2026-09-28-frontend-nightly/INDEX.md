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

| lib/ui/chat_panel.dart | 0005 | 列表 builder 化 + 视图层裁剪 + 听按钮计时器对称 ✓；无发现 |
| lib/ui/message_bubble.dart | 0005 | **整条 excludeSemantics 吃掉重试/复制 → 读屏不可达（F-0005-4）**；宽度/光标/系统行三通道 ✓ |
| lib/ui/session_sheet.dart | 0005 | 垫层在位（M ✓）；两段式删除 ✓ |
| lib/ui/theme.dart | 0005 | 令牌接 ThemeData 唯一入口；main.dart:215 接 AppMaterial ✓ |
| lib/ui/settings_scaffold.dart | 0005 | 三宿主共用；panelAlpha 单一真源 ✓ |
| lib/ui/state_pill.dart · streaming_indicator.dart | 0005 | 动画启停/dispose 对称 ✓；但 StatePill 的 reduced-motion 分支零覆盖（F-0005-7） |
| lib/ui/audio_bar.dart · connection_badge.dart · inline_notice.dart · soft_motion.dart | 0005 | 三轴命名/五态三通道/reduced-motion 出口/唯一 notice 纪律全在位 ✓ |
| lib/ui/section_header · group_card · restart_notice · emphasized_text · theme_picker · confirm_discard_dialog · error_banner · stage_corner_controls | 0005 | 逐个读完无缺陷；confirm_discard 自垫垫层（M ✓） |
| （每个 delta 重建设置分区） | 0005 | chat_controller:341 → main:1072 → app_shell:643 → _settingsSurface:508 → **F-0005-2 P1** |
| lib/chat/chat_message.dart · turn_liveness.dart · shell_chat.dart | 0005 | 落盘取舍/收口判据/失败保草稿三处核对通过 ✓ |
| lib/app/collapsible_panel.dart（补读） | 0005 | 四通路齐全 + child 提到 builder 外 ✓（保活纪律的正确同构） |

| lib/design/tokens.dart | 0006 | AppPalette/AppColors 四套取值 + 12 字段五件套齐全 ✓；`panelAlphaFor` 有可读性下界 ✓ |
| lib/design/typography.dart | 0006 | 8 槽位单一真源 + 声明↔生效对账 ✓；copyWith 派生保 fontFamily ✓ |
| lib/design/breakpoints.dart | 0006 | 900/1280 唯一真源 ✓；全仓无旁路比较 |
| lib/ui/glass_rim.dart | 0006 | 三层描边固定数字 + shouldRepaint 五字段 ✓；onHover→setState（F-0006-4 P3） |
| lib/ui/background_patterns.dart · background_logic.dart | 0006 | 固定种子/图案更安静/轮播纯函数 ✓；shouldRepaint 齐全 ✓ |
| lib/ui/shell_backdrop.dart | 0006 | **每 delta 重解码整图（F-0006-1，升级 F-0003-1）** |
| lib/ui/field_row.dart（补审） | 0006 | 滑杆刻度用 contentFaint 当文字色（F-0006-2 P2） |

| lib/state/ui_state_tracker.dart | 0007 | 相位真源；定时器/dispose 对称 ✓；**本地失败无人复位（F-0007-1）** |
| lib/state/ui_phase.dart | 0007 | 纯函数 + 判定顺序即契约 ✓ |
| lib/state/live_region.dart | 0007 | 两档节流 + 只在真更新时通知（无反馈环）✓ |
| lib/chat/chat_controller.dart | 0007 | 引用语义/epoch 门禁/五路收口 ✓；`_failLocalTurn` 不碰 UI 相位（F-0007-1） |
| lib/chat/chat_session.dart | 0007 | 版本门禁/坏条目丢弃/activeId 悬空回落 ✓ |
| lib/chat/chat_markdown.dart | 0007 | 逐字符扫描、空记号不吞 ✓ |
| lib/ui/error_actions.dart | 0007 | **「打断并重发」只调 onStop（F-0007-2）** |

| lib/api/api_client.dart | 0008 | `_guard`/错误码/PA-not-PUT/baseline 转发即丢 ✓ |
| lib/api/ws_frame.dart | 0008 | 9 分支逐字对齐 Rust 投影（events.rs 实读）✓；宽容 seq 解析有理由 ✓ |
| lib/api/ws_client.dart | 0008 | readyState 判活 + 僵尸摘除 ✓；**无心跳看门狗（F-0008-1 P1）** |
| lib/api/ws_liveness.dart · ws_status.dart | 0008 | 纯函数判据齐；**无时间维度**（F-0008-1） |
| lib/api/env_api.dart | 0008 | `EnvKey` 无 value 字段 + 只 PUT 不回读（G 红线结构上成立）✓ |
| lib/api/diagnostics_api.dart | 0008 | 不发无效查询串（501 事故已收口）；capabilities 不持假广告 ✓ |
| lib/api/mods_api.dart | 0008 | secret 不回值 + 密码框渲染 ✓ |
| lib/api/settings_models.dart | 0008 | 有生效值而非 Option；`defaultMaxTokens=4096` 与 Rust 实测一致 ✓ |

| lib/audio/sentence_assembler.dart | 0009 | 攒句六规则 + SerialQueue 纯逻辑 ✓；空句/WAV 直通分支自洽 ✓ |
| lib/audio/audio_player.dart | 0009 | blob URL 回收/元素摘除/ticker 停止全对称 ✓；静音走 element.muted（不折增益）✓ |
| lib/audio/epoch_gate.dart · stage_clock.dart · gain.dart | 0009 | 三个纯判据模块 + 对应测试 ✓ |
| lib/voice/voice_listen_controller.dart | 0009 | await 后状态守卫遍布；两个 Timer dispose 归零；**PTT 无松手兜底（F-0009-1 P3）** |
| （红线 O：30Hz 通路） | 0009 | Stream→GlobalKey→postMessage ✓ 实测确认，不经 Widget 树 |

| lib/live2d/live2d_host_web.dart | 0010 | 收发双向钉 origin + source 校验 + listener 随 dispose 摘除（G ✓）|
| lib/live2d/live2d_bridge.dart | 0010 | v1 信封 + 就绪队列（mouth 合并、队列有上限）+ ack 历史形状 ✓；**progress/fps 无出口（F-0010-2）** |
| lib/live2d/stage_pointer_interceptor.dart | 0010 | 根因注释 + `enabled` 开关 + **诚实记录管不到的两处** ✓ |
| lib/live2d/session_baseline.dart · stage_cancel.dart | 0010 | 两路交付共用解码器；空 baseline 恰好一次撤销；四步编排纯逻辑 ✓ |
| lib/live2d/live2d_stage.dart | 0010 | `_onBridgeChanged` 转发四类、漏两类（F-0010-2）；**`sync(paused:)` 死线字段（F-0010-1）** |
| （v1 契约核对） | 0010 | 6 type + 10 payload 键与 `main.rs` 逐字对齐；渲染面兼容历史扁平帧 ✓ |
| （红线 M 覆盖） | 0010 | 8 使用点 / 8 断言，分散在 3 个测试文件 ✓ |

## Phase 2 横扫
| 轴 | 批次 | 结论 |
|---|---|---|
| I 样式令牌/主题覆盖 | 0021 | 字体族单一入口 ✓（组件层唯一覆盖就是错的那处）；裸 TextStyle 4 处全合法 ✓；**行内码缺 `fontFamilyFallback` → 联网回退（F-0021-1 P2，红线 L 破口）** |
| H 路由/懒加载/舞台保活 | 0020 | **无新发现**：三宿主 × 保活机制逐条核 ✓；keepalive 测试有**反向对照** ✓；`_ensureSettingsLoaded` 失败后有「重试」✓ |
| G 安全 | 0019 | innerHTML/eval/重定向 **全仓零命中** ✓；密钥四面（类字段/GET/状态 DTO/localStorage/剪贴板）**全清** ✓；文件选择不查体积（F-0019-1 P3）|
| F 无障碍/键盘/字体门禁 | 0018 | 6 个 IconButton 标签全覆盖 ✓ · 焦点序两层都有 ✓ · **字体门禁的扫描根 `Directory('lib')` 经核**不构成缺口**（web/ 的文字不过 CanvasKit）**；发现 `http_<status>` 合成码（F-0018-1 P3）|
| E 契约漂移 | 0017 | SettingsView 六段逐字段对齐 ✓、**`performance` 段前端零解析（F-0017-1 P2）**；错误码全集与前端三条前缀分支**逐条对得上** ✓ |
| D build 重活/列表/painter | 0016 | **全部 2 个 CustomPainter 的 shouldRepaint 齐全 ✓**；4 个长列表全 builder 化 ✓；缩略图每 build 重解码（F-0016-1 P2，与 F-0006-1 同根因）|
| C 竞态/取消/三态 | 0015 | `.timeout` 5 处全在会挂起的平台操作上（刻意）；6 条加载路径里 4 条样板正确、**shell_admin 5 条缺兜底 catch+finally（F-0015-1 P3）**；`swapModel` 超时订阅泄漏（F-0015-2 P3）|
| B dispose/订阅/监听对称性 | 0014 | **全仓 7 类资源逐项人工对账，全对称 ✓**；确认三处「顺序错了不报错」的调用点顺序正确 |
| A 状态源唯一性 | 0013 | 9 个状态源盘点：`DisplayPrefs`/`UiPhase`/`_assistant`/`口型电平` **单一真源 ✓**；`_backgroundIndex` 两份（F-0001-2 已登记）；记忆桶第二份快照（F-0004-1）；**`_store` 在水合窗口指向错误的库（F-0013-1 P1）** |

## test/ 全量（B12 覆盖盘点）
| 指标 | 数值 |
|---|---|
| lib .dart / test .dart | 113 / 93 |
| 类名零命中的 lib 文件 | 6（3 个合理=package:web/组合根，3 个是缺口）|
| 源码扫描式守卫 | 28 个文件（12 个剥注释，判别力好）|
| 恒真/弱断言 | 205 处，**确证假绿灯 1 处**（F-0005-3）|
| `expect(常量,常量)` | 0（0005 的 480 那条是钉常量，有判别力）|
| 空泵 | 1 处（F-0005-7）|
| 断言总数 / 带 reason | 3143 / 684 |

## test/（B12 新增缺口）
| lib/settings/sections/llm_section.dart | 0012 | **整块零测试覆盖（F-0012-2）**；自检成败靠文案猜（F-0012-1 P1）|
| lib/settings/sections/tts_section.dart | 0012 | 同款文案猜分类（F-0012-1 第二处）|

## test/（B10 抽查）
| test/stage_pointer_interceptor_test.dart | 0010 | 7 处接线断言（横幅/角标/loading/error/三宿主）+ 透明性真断言 |
| test/confirm_discard_test.dart:141 · session_sheet_test.dart:354 · compact_settings_page_test.dart:333 | 0010 | 另 3 处垫层断言（M 红线全覆盖）|

## test/（B9 抽查）
| test/sentence_assembler_test.dart | 0009 | 418 行真断言 |
| test/stage_clock_test.dart | 0009 | 99 行行为 + 接线扫描（每串唯一出现，判别力可接受）|
| test/wav_test.dart · audio_epoch_gate_test.dart · audio_gain_test.dart · voice_listen_controller_test.dart | 0009 | 存在且规模合理，模式抽查无空转 |

## test/（B8 抽查）
| test/ws_frame_test.dart | 0008 | 40 例真断言，零源码扫描 |
| test/settings_api_test.dart | 0008 | 32 例；三态 PATCH 齐全 |
| test/env_api_test.dart | 0008 | MockClient 断言真实 method/path/header/body ✓ |
| test/admin_api_test.dart | 0008 | 19 例真断言 |
| test/ws_liveness_test.dart | 0008 | 53 行纯函数回归；**缺「时间维度」判据**（F-0008-1 的修法位点） |

## test/（B7 抽查）
| test/ui_state_tracker_test.dart | 0007 | 30+ 条真断言（连接/轮次/打断/error 五组），零源码扫描 |
| test/chat_session_test.dart | 0007 | 32 条真断言 |
| test/turn_liveness_test.dart | 0007 | 判别力良好；「本地失败回落相位」可照此模式新增 |

## test/（B6 抽查）
| test/design_tokens_lint_test.dart | 0006 | 6 条规则 + 「扫描确实覆盖源码」自检 + 豁免面收窄——质量高；断点规则模式有洞（F-0006-3 P3） |
| test/theme_palette_test.dart（清单核对） | 0006 | 覆盖 ink/muted/accent/onAccent/danger 五组；**缺 contentFaint 压 surfaceAlt**（F-0006-2） |
| test/breakpoints_test.dart（存在性） | 0006 | 只测纯函数，不扫调用点 |

## test/（B5 抽查）
| test/message_bubble_test.dart | 0005 | 真断言（星号/宽度/复制/思考区）；`expect(常量, 480.0)` 是**钉常量**不是空转（有判别力） |
| test/chat_bubble_labels_test.dart | 0005 | **F-0005-3：空泡不上屏的断言结构上恒真** |
| test/chat_notice_test.dart | 0005 | ②③ 行为断言真；**③ 组「气泡层分流」守卫空转（F-0005-6）** |
| test/semantics_test.dart | 0005 | liveRegion/舞台语义真断言；**RepaintBoundary 守卫空转（F-0005-1）+ reduced-motion 空泵（F-0005-7）** |
| test/no_backdrop_filter_test.dart:246-252 | 0005 | **同一条空转守卫的重复副本（F-0005-1）** |
| test/font_subset_test.dart | 0005 | 门禁自身质量高（词法器+哈希同源+校准位）；**覆盖面缺口 = F-0005-5** |

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
| 文件 | 批次 | 结论 |
|---|---|---|
| web/index.html | 0011 | 零外部源（红线 K ✓）；base href 由构建期替换 |
| web/manifest.json | 0011 | 零外部 URL ✓ |
| pubspec.yaml | 0011 | 依赖面仅 flutter+http+web（入口包体无债务）✓；头注版本号不同步（F-0011-1 P3）|
| tool/visual_review_test.dart | 0011 | 12 张视觉回归图；**自报盲区**（画不出平台视图）✓ |
| test/wiring_test.dart | 0011 | 逐个点名的接线守卫，每条写清后果 ✓（不剥注释，与 F-0005-6 同族弱点）|
| scripts/ignite.sh（只读） | 0011 | 红线 K 三重防线：写死 flag + 产物 grep + 服务态 `--check` ✓ |
| （产物形状取证） | 0011 | build/web/canvaskit 在位 + main.dart.js 无 gstatic ⇒ 带 flag 构建，与 ignite.sh 判定一致 |
