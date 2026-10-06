# 前端资产守护网（A1–A8 接触点清单 + 守护断言对照）

> **任务**：0.2.0 封口路线 Stage A 的 A1（资产守护网）。
> **范围真源**：docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md §3（资产清单 A1–A8 +
> （该计划已于 2026-10-06 的 E8 文档减量中移出工作树，取回见
> [已移出工作树的文档索引](../REMOVED-docs-index-2026-10-06.md)）
> 守护红线 + §3.3 冲突热点）、docs/architecture/performance-protocol-v1.md。
> **工作树**：/home/skystar/Live2D-Ai-fe @ feat/frontend-redesign @ e4f139a8（= main = v0.2.0-rc.4，**未重放**）。
> **本文件只新增文档与测试；lib/** 与既有测试文件一行未改。**
>
> 本文回答四个问题：① 每个资产的前端接触点在哪（file:line）；② 现在谁在守
> （现有测试名）；③ 断了会怎样；④ 本次新增了什么（以及为什么只能这么写）。

---

## 0. 一句话结论

- main 上**已有**一张有效的资产守护网（十余个相关测试文件、数百条断言），
  本任务是**盘点 + 补缺 + 加固**，不是从零建网。
- 实测到 **4 个真实缺口**，本次新增 **4 个文件 / 19 条断言**补齐：
  1. **main.dart 的导演 cue 调用点**：既有断言只查名字出现过（定义就能满足），
     把 :481-483 整段删掉仍全绿 → 新增窄调用点判据（结构，见 §4 的可证伪性）。
  2. **main.dart → Live2DStage → Live2DBridge 的 preset 转发缝**：
     action_scales_wiring_test 守的是 syncer 与 main.dart 构造点，**不守这条缝**
     → 新增函数体级 / 帧级判据（结构为主 + 1 条行为，见 §3.2）。
  3. **live2d_stage 的 iframe 重建自愈补发**（stage-bg / stageColor /
     actionScales）：桥侧行为断言测不到「舞台有没有补发」
     → 新增 _attach / didUpdateWidget 函数体级判据（结构）。
  4. **TTS 配置面本身**：既有守卫全在 PATCH/草稿层，且那两个测试文件**都在重放
     重叠清单里**（会被覆盖）→ 新增 5 条 widget 行为断言。
- 另有 5 个既有测试文件落在 29 个重放重叠文件里，**重放若直接覆盖会静默删掉
  main 的守卫**——见 §5，A2 必须以「并集」方式解冲突。

---

## 1. 复现环境与门禁（本次实测原始数字）

工作目录 /home/skystar/Live2D-Ai-fe/shell/flutter，flutter 在 HOME/flutter/bin。

    flutter analyze
    -> No issues found! (ran in 2.4s)          # 0 issue

    flutter test
    -> 00:24 +1110: All tests passed!          # 1110 passed / 0 failed
    (TEST_EXIT=0)

    只跑本次新增的 4 个文件：
    -> 00:01 +19: All tests passed!            # 19 passed / 0 failed

新增前后：main 基线 1091 条 → 现在 1110 条（**净增 19**）。

---

## 2. A1–A8 前端接触点清单

「现有测试」列给出 file:line + 测试名（用 main 上真实行号）。
「守护」列：**既有** = 现有断言已足够，本次不重复加；
**新增** = 本次补的（括号里是断言类型：行为 / 结构）。

### A1 9 条动作包 + none 哨兵（preset 帧）

| 接触点 | file:line | 现有测试 | 断了会怎样 | 守护 |
|---|---|---|---|---|
| 预设下发（旧路径 / v1 字段） | lib/live2d/live2d_stage.dart:418 applyPreset；:435 _bridge?.sendPreset( | test/action_cue_test.dart:311 sendPreset(none…) 发 {type:preset,id:none}（行为）；:341 body cue 的 field/x/y/hold/at 原样进 preset payload（行为） | 动作包到不了渲染面 ⇒ 表演层静默消失，聊天/语音照常（最难发现的一类） | **新增**（结构）：applyPreset 函数体必须转发 _bridge?.sendPreset( |
| preset 帧发出 | lib/live2d/live2d_bridge.dart:260 sendPreset；:293 _enqueueOrSend(preset, payload) | 同上 :311 / :341（行为） | 帧不出去，导演 cue 白下 | **新增**（行为）：缺 field 时**只发旧四键**、不泄漏 v1 键（V11 只增不改的「没带就不多发」这半边没人守） |
| 撤销哨兵 none | lib/live2d/live2d_stage.dart:434 空 id 早退 | test/action_cue_test.dart:149 none → 恰好一次撤销（行为）；:191 同 seq 只应用一次（行为） | 该句音频开始时撤销不到点 ⇒ 上一句表情挂着 | 既有 |
| 按句计划（整表替换 / 取用即移除） | lib/live2d/live2d_stage.dart:56-86 DirectorCuePlan | test/action_cue_test.dart:191/:217（行为） | 同 seq 重复帧重复演 / 迟到帧补演 | 既有 |
| 动作包定义表 | assets/actions/presets.json + crates/l2d-wasm-demo/src/preset/* | test/preset_labels_test.dart:12/:25/:33/:52（行为） | 面板显示错名 / 表情与动作串槽 | 既有（**Rust/资产侧，Flutter 只能守标签表**） |

### A2 [action] 幅度倍率 + 每皮套覆盖

| 接触点 | file:line | 现有测试 | 断了会怎样 | 守护 |
|---|---|---|---|---|
| 幅度下发出口 | lib/live2d/live2d_stage.dart:498 applyActionScales；:501 _bridge?.sendSync(actionScales:) | test/action_scales_wiring_test.dart:82（三层优先级行为）、:217（去重行为）、:511/:560/:582/:627/:671（每皮套行为） | 每皮套幅度下发丢失 / 被草稿静默冲掉 | **新增**（结构）：applyActionScales 函数体必须转发 sendSync(actionScales:)，且三键 + isFinite 判据在 |
| 三层优先级 syncer | lib/live2d/action_scales_sync.dart:55 ActionScalesSyncer（:97 setModelContext / :129 active / :150 syncNow） | test/action_scales_wiring_test.dart:82/:217（行为） | 临时覆盖被产品值冲掉 | 既有 |
| 构造点接线（main.dart 传给 stage） | lib/main.dart 构建 Live2DStage 处 | test/action_scales_wiring_test.dart:482 接线（源码扫描） | actionScales 从未传过 ⇒ 重建自愈恒 null | 既有 |
| 重建自愈补发 | lib/live2d/live2d_stage.dart:352 _attach sendSync(actionScales:)；:263-264 didUpdateWidget | **无** | 换皮 / iframe 重挂后幅度回出厂默认 | **新增**（结构）：asset_guard_stage_attach_resend_test.dart |

### A3 导演 Mod 接线（唯一驱动 = WS action_cue）

| 接触点 | file:line | 现有测试 | 断了会怎样 | 守护 |
|---|---|---|---|---|
| **按句订阅导演 cue** | lib/main.dart:481-483 _audio.sentenceStarts.listen(_applyDirectorCueForSeq) | test/action_cue_test.dart:240「源码级断言」（:290 只查 contains(_applyDirectorCueForSeq) ⇒ **定义就能满足，零检验力**） | cue 到了前端但没人喂给舞台 ⇒ 导演哑火、别的都正常 | **新增**（结构）：asset_guard_director_cue_test.dart 要求**调用形状**；注释掉 :481-483 必红 |
| cue 定义 + 参数透传 | lib/main.dart:591 定义；:612 stage.applyPreset( | test/action_cue_test.dart:341（行为，桥侧） | v1 三族字段被吞 | **新增**（结构）：main.dart 必须**调用** stage.applyPreset( |
| action_cue 帧消费（整表替换 / baseline 二路） | lib/main.dart:524-535（:534 _directorCues.replace(event.cues)、:529-531 baseline 立即应用） | test/action_cue_test.dart:240（:284 断言 replace 调用形状）；test/session_baseline_test.dart:33/:91/:234/:286（行为） | 帧到了不换计划 / baseline 被当按句计划缓存 | 既有 |
| 帧解析 | lib/api/ws_frame.dart（ActionCueEvent） | test/action_cue_test.dart:40/:79/:114（行为） | 新帧被当未知丢弃 | 既有 |
| 通道 B 已退役（负向） | — | test/action_cue_test.dart:240（语义扫描 + 退役 token 名单）；test/no_client_side_preset_ttl_prediction_test.dart（全文件） | 旧「拉状态面 + 本地推演」复活 | 既有 |
| 撤销事务（停止/新消息） | lib/main.dart:450-462 StageCancellation；:635-652 | test/stage_cancel_test.dart:19/:77（行为 + 源码） | 追着播旧 cue / 锁死输入框 | 既有 |

### A4 表演协议 v1（segments + cues）

| 接触点 | file:line | 现有测试 | 断了会怎样 | 守护 |
|---|---|---|---|---|
| v1 字段 → 既有 preset 帧（只增不改） | lib/live2d/live2d_bridge.dart:279-292 | test/action_cue_test.dart:341（行为，field/x/y/z/hold/at/seq/epoch/sentence_seq 全断言） | 字段名被改 ⇒ 渲染面认不出 | 既有 + **新增**（行为）「缺省不发 v1 键」 |
| 协议真源 | docs/architecture/performance-protocol-v1.md | 文档，无代码测试 | — | n/a（规范） |

### A5 TTS 过滤 clean_for_tts（上屏 == 送 TTS == clean_for_tts(段)）

| 接触点 | file:line | 现有测试 | 断了会怎样 | 守护 |
|---|---|---|---|---|
| TTS 配置面（base_url/voice/model/音频规格/密钥/静音/自检） | lib/settings/sections/tts_section.dart:72-80 / :81-90 / :91-99 / :100-105 / :106-114 / :123-130 / :131-158 / :159-180 | test/settings_controller_test.dart:142/:153（草稿行为）、:156（PATCH 只含 tts.voice）；test/settings_api_test.dart:65/:78/:312（PATCH/GET 形状） | 配置项被旧版覆盖（response_format 控件被「顺手加回」/ 静音做成第二个开关） | **新增**（行为）：asset_guard_tts_config_test.dart 5 条 widget 断言（**因为那两个既有测试文件本身在重放重叠清单里**） |
| clean_for_tts 本体 | crates/live2d-ai-runtime/src/tts.rs + conversation_engine_tts_flow.rs | **Rust 侧** | 思考/标记被念出来 / 上屏与送 TTS 不同源 | 前端**无接触点**（红线 #2 由 Rust 回归守，不在本 Flutter 网内） |

### A6 音频时钟基准 stage-clock + 事件级 ack

| 接触点 | file:line | 现有测试 | 断了会怎样 | 守护 |
|---|---|---|---|---|
| stage-clock 帧 | lib/live2d/live2d_bridge.dart:302 sendStageClock | test/stage_clock_test.dart:48 30ms 节拍 + wire 形状（行为）；:107 接线（源码） | 编排锚回 performance.now() ⇒ 与声音错拍 | 既有 |
| main.dart 时钟转发 | lib/main.dart:488-499 | test/stage_clock_test.dart:107（源码） | 时钟断在宿主 | 既有 |
| stage-bg 帧 | lib/live2d/live2d_bridge.dart:245-249 sendStageBg | test/live2d_bridge_test.dart:92 stage-bg 用 dataUrl、null→空串（行为） | 背景图不上屏 | 既有 |
| **舞台补发 stage-bg** | lib/live2d/live2d_stage.dart:355-356 _attach；:257-260 didUpdateWidget | **无**（桥单测测不到舞台发不发） | iframe 未就绪 / 重建时背景图**永久丢失**（rc.4 的 Win 现场） | **新增**（结构）：asset_guard_stage_attach_resend_test.dart |
| ack 解析（preset-applied/expired/dropped、segment-ended） | lib/live2d/render_events.dart:28-34 kRenderAckTypes；:85-106 parseRenderEvent；:195-199 kPresetAckKinds | test/render_events_test.dart:44/:143/:186/:202/:222/:260/:271（行为） | 面板谎报「已生效」/ 未知字段破坏兼容 | 既有 |
| ack → 快照 | lib/live2d/preset_status.dart:75-105 presetStatusUpdateFor | test/action_scales_wiring_test.dart:275（行为）；test/render_events_test.dart | 前端自行推演剩余时长（V8） | 既有 |
| ack 汇入导演日志 | lib/live2d/render_events.dart:132 DirectorEventLog；:253 ObserverBuffer | test/render_events_test.dart:44/:124/:186/:202/:260/:271（行为 + 源码） | 可观测性回退 | 既有 |

### A7 会话 baseline（绑会话）

| 接触点 | file:line | 现有测试 | 断了会怎样 | 守护 |
|---|---|---|---|---|
| baseline 原文 → cue 列表 → 下发清单 | lib/live2d/session_baseline.dart:102 parseBaselineCues；:118 baselineApplicationFromCues；:126 baselineApplication | test/session_baseline_test.dart:33/:91/:117/:197/:234/:286（行为 + 源码） | 空 baseline 退化成「什么都不做」⇒ 旧动作挂着 | 既有 |
| 两路交付（HTTP 响应体 + action_cue{baseline:true}） | lib/main.dart:529-531 / :666-722 | test/session_baseline_test.dart:234/:286（行为 + 源码） | 取消事务后半段丢 | 既有 |
| 回合状态判据（收口/断连/迟到帧/致命错误） | lib/chat/turn_liveness.dart:73 settleTurn、:98 mustReleaseTurnOnWsLoss、:167 mustSettleTurnOnError、:176 frameBelongsToCurrentTurn、:197 needsFailureFallbackCode | test/turn_liveness_test.dart:17/:24/:35/:39/:43/:48/:56/:63/:117/:126/:131/:141/:156/:195/:212/:216/:226/:230/:235/:243/:247（**全部行为**） | 「无字无错」永久转圈 / 用户的停止被说成模型没返回 | 既有（**但该测试文件本身在重放重叠清单里，见 §5**） |

### A8 导演可观测（dev_mode 才渲染、零后端、不落盘、不新增 WS 帧）

| 接触点 | file:line | 现有测试 | 断了会怎样 | 守护 |
|---|---|---|---|---|
| 观测面板 / feed | lib/settings/sections/director_observer_section.dart:46 kDirectorObserverModId、:69 DirectorObserverFeed、:144 DirectorObserverSection | test/director_observer_test.dart:390 标识符交集（禁止持久化标识符）；其余纯逻辑用例 | 观测面偷偷落盘 / 引用不存在端点 | 既有 |
| director Mod 面板 | lib/settings/mods/director_panel.dart:138 DirectorPanel；:107 directorPresetText | test/director_panel_test.dart:114/:175/:223/:287（行为） | 面板显示错名 / 空态不可读 | 既有 |
| 四栏事件流缓冲 | lib/live2d/render_events.dart:132 / :253 | test/render_events_test.dart:186/:202/:260/:271（行为） | 缓冲无界 / 心跳污染 | 既有 |

**A8 说明**：director_observer_section.dart 与 director_panel.dart 是 **main 独有新增文件**
（阶段5 提交，晚于重设计基线 rc.1），**不在 29 个重放重叠清单里** ⇒ 重放不会覆盖它们，
既有测试也不会丢。故本次不为 A8 新增断言。

---

## 3. 本次新增的 4 个文件 / 19 条断言

全部位于 shell/flutter/test/asset_guard_*_test.dart（新文件，非重叠路径 ⇒ 重放不会覆盖）。

### 3.1 test/asset_guard_director_cue_test.dart（A3）

| 测试名 | 类型 | 盯什么 |
|---|---|---|
| 结构：main.dart 把 _applyDirectorCueForSeq **调用**订到音频按句流上 | 结构 | 正则 _audio.sentenceStarts.listen( 之后紧跟 _applyDirectorCueForSeq |
| 判别力自证（对真实文件）：去掉调用点后，名字仍在但判据必红 | 结构 | 对**真实 main.dart** 内存变换：调用点整段替换 → hasMatch 必须 false，而 _applyDirectorCueForSeq 仍出现（定义在 :591）⇒ 复现「定义仍在、调用点没了」的现场；旧 contains(名字) 断言在该现场仍是绿 |
| 判别力自证：调用点未注释 → 命中；注释掉 → 不命中 | 结构 | 内建编排者的验收手法（注释掉必红） |
| 判别力自证：只剩定义（无调用）→ 不命中 | 结构 | 「不是只定义」的字面要求 |

**为什么是结构**：main.dart 经 app/browser_io.dart 依赖 package:web，VM 下加载不了；
Live2DStageState._bridge 只能由 web-only 的 buildLive2DHost(onTransport:) 挂上 ⇒
「cue 被订阅到音频流」在单元级没有可达行为路径（规划裁决 (b)）。
**不扫文案、不扫颜色字面量、不认裸名字。**

### 3.2 test/asset_guard_preset_dispatch_test.dart（A1 + A2 + A4）

| 测试名 | 类型 | 盯什么 |
|---|---|---|
| 结构：main.dart 仍然**调用** stage.applyPreset( | 结构 | 宿主 → 舞台那一段（main.dart:612） |
| 结构：applyPreset 的函数体把参数转发给 _bridge?.sendPreset( | 结构 | 舞台 → 桥那一段（函数体级，不是全文件 contains） |
| 结构：applyPreset 保留「没有 field 且 id 为空 = 本轮不动」的早退 | 结构 | 旧路径语义不许被重放改掉 |
| 结构：applyActionScales 的函数体转发给 _bridge?.sendSync(actionScales: | 结构 | A2 唯一出口 |
| 结构：applyActionScales 保留「三键 + 全部有限」的静默不发判据 | 结构 | 污染舞台快照的那条路 |
| 行为：缺 field 时只发旧四键，一个字都不多 | **行为** | V11：{id, source, ttl_ms, intensity} 逐字不变，且 field/x/y/z/hold/at/seq/epoch/sentence_seq 一个都不多发 |
| 取证：VM 测试里舞台走 host stub ⇒ _bridge 恒 null | 行为（取证） | 若变红 = VM 已能挂桥，请把上面的结构守卫升级为「喂 cue → 断言 preset 帧」的行为断言 |

**函数体级做法**：先用圆括号配平定位参数表，再对函数体做花括号配平，只取该方法的
函数体 ⇒「同类里别的方法碰了 sendPreset」不会冒充本方法的转发。

### 3.3 test/asset_guard_stage_attach_resend_test.dart（A2 + A6）

| 测试名 | 类型 | 盯什么 |
|---|---|---|
| 结构：_attach 每次挂桥都补发 stage-bg | 结构 | :356 unawaited(bridge.sendStageBg(widget.stageImage)) —— rc.4「Win 背景图丢失」的修复 |
| 结构：_attach 补发 sync 时带 stageColor 与 actionScales | 结构 | :347-354 重建自愈 |
| 结构：值变化时 didUpdateWidget 也补发（stage-bg + actionScales） | 结构 | :257-264 偏好变化那条路 |

**为什么是结构**：_attach 是私有方法、只由 web-only host 回调（VM 下不可达）。
既有 live2d_bridge_test.dart:92 是**桥侧**行为断言，测不到「舞台有没有补发」。

### 3.4 test/asset_guard_tts_config_test.dart（A5，全部**行为**）

| 测试名 | 类型 | 盯什么 |
|---|---|---|
| A5：TTS 配置面四件套都在（base_url / voice / model / 音频规格） | 行为 | 配置项不许被旧版覆盖 |
| A5：服务端静音是**只读观测值**，分区里没有第二个开关 | 行为 | 断言整段无任何 Switch/SwitchListTile（红线：静音不是开关） |
| A5：PCM 规格只在 devMode 出现（渐进披露） | 行为 | 采样率/声道数 |
| A5：连通性自检是按钮、会回调，文案不宣传成「试听」 | 行为 | 点击 → onTest 回调一次；description 含「不合成」 |
| A5：编辑 base_url / voice / model 只落进 tts 草稿，不碰别的段 | 行为 | 草稿按段提交，别的段仍为 null |

---

## 4. 已存在 vs 本次新增（对照表）

| 接触点 | 现有守卫（file:line + 测试名） | 本次新增 | 类型 |
|---|---|---|---|
| main.dart 导演 cue **调用点** | test/action_cue_test.dart:240「源码级断言…」（:290 只查名字） | asset_guard_director_cue_test.dart ×4 | 结构 |
| main.dart action_cue 整表替换 | test/action_cue_test.dart:240（:284 断言调用形状） | 无（已存在） | — |
| main.dart **调用** stage.applyPreset | 无（仅名字级） | asset_guard_preset_dispatch_test.dart「结构：main.dart 仍然调用…」 | 结构 |
| live2d_stage.applyPreset 转发 | 桥侧行为 test/action_cue_test.dart:311/:341；**舞台侧无** | asset_guard_preset_dispatch_test.dart ×2 | 结构 |
| live2d_bridge.sendPreset 帧形状 | test/action_cue_test.dart:311/:341（行为） | 补「缺省不发 v1 键」（行为） | 行为 |
| live2d_stage.applyActionScales 转发 | action_scales_wiring_test.dart 守 syncer/构造点，**缝无** | asset_guard_preset_dispatch_test.dart ×2 | 结构 |
| live2d_bridge stage-bg 帧 | test/live2d_bridge_test.dart:92（行为） | 无（已存在） | — |
| live2d_stage 重建自愈补发 | **无** | asset_guard_stage_attach_resend_test.dart ×3 | 结构 |
| stage-clock（帧 + 接线） | test/stage_clock_test.dart:48/:107 | 无（已存在） | — |
| ack 解析 / 汇入导演日志 | test/render_events_test.dart 多条（行为） | 无（已存在） | — |
| TTS 配置面（widget） | settings_controller_test.dart / settings_api_test.dart（草稿层；**且两文件在重放重叠清单里**） | asset_guard_tts_config_test.dart ×5 | 行为 |
| turn_liveness（A7） | test/turn_liveness_test.dart 全文件（行为） | 无（已存在；覆盖风险见 §5） | — |
| session baseline（A7） | test/session_baseline_test.dart:33/:91/:117/:197/:234/:286 | 无（已存在） | — |
| ui_state_tracker error/回合消费 | test/ui_state_tracker_test.dart:249/:262/:272/:285/:294/:304（行为）、:140/:149/:158（回合）、:44/:62（连接态） | 无（已存在；且该测试文件**不在**重叠清单里 ⇒ 重放后仍在） | — |
| director 观测 / 面板（A8） | test/director_observer_test.dart、test/director_panel_test.dart | 无（已存在；文件非重叠） | — |

### 「注释掉调用点 → 必红」的验证结论

编排者要求的硬验收（把 lib/main.dart:481-483 注释掉 ⇒ 断言必红）：

- **做到了。** asset_guard_director_cue_test.dart 的
  「判别力自证（对真实文件）」用例在**内存里**对真实 main.dart 做了等价变换
  （把判据命中的调用点整段替换掉），断言 hasMatch 变 false，而
  _applyDirectorCueForSeq 仍出现在文件里（定义在 :591）。该用例**实测通过**，
  即「定义仍在、调用点没了」时本判据确实是红的。
- 另有一条合成自证用例把「// 注释掉 → 不命中」直接编码（剥注释是判据路径的一部分）。
- **没有**真的去改 lib/main.dart 做人工试验：任务硬约束「lib/** 一行都不许改」，
  且中途失败会留下脏树。上面的内存变换是它的等价且无副作用的版本。

---

## 5. 重放会覆盖的守卫（最高风险，A2 必须按并集解冲突）

/tmp/overlap-real.txt 的 29 个重叠文件里，**5 个是测试文件**；此外 6 个是资产
lib 接触点本身。重放若直接「用重设计版覆盖」，下列守卫会被**静默删掉**：

| 重叠测试文件 | 现在守什么（关键断言） | 覆盖后会丢什么 | A2 解冲突要求 |
|---|---|---|---|
| test/turn_liveness_test.dart | **A7 的全部回合收口判据**：空气泡≠删掉（:24）、用户停止≠模型没返回（:48/:131）、断连中途收口（:72/:82）、connecting 不收口（:89）、迟到旧轮帧丢弃（:230）、致命错误兜底（:212）、禁止无字无错（:243） | **A7 的唯一行为守卫整份消失**（lib/chat/turn_liveness.dart 是重放冲突点，改错时没有任何红灯） | **并集**：main 版 21 条一条不少；重设计版若有新增用例，追加而不是替换；合并后必须仍覆盖上述 6 类 |
| test/settings_controller_test.dart | 草稿 / PATCH 按段语义：改 voice 只发 {"tts":{"voice":…}}、不得顺手清 base_url（:156/:159）、pruneAgainstRemote 撤回同值改动、:413 llm/tts 段循环 | TTS/LLM 配置面的**提交形状**守卫 | 并集；保留 main 的「只提交被改过的段」类断言 |
| test/settings_api_test.dart | Settings GET/PATCH wire 形状：tts.api_key_env / clear_api_key **不得**出现（:73-85）、model 为 null 的 Tri 语义（:312-317）、坏响应回落（:334-340）、testTts（:413） | A5 协议形状守卫 | 并集 |
| test/mods_section_test.dart | Mods 分区渲染 + settings_spec 表单 + secret 留空不提交（rc.4 M2/M3 产品链路） | Mod 产品链路的前端守卫 | 并集 |
| test/design_tokens_lint_test.dart | 设计令牌 lint（裸色 / 裸字号 / 圆角统一） | 设计令牌门禁（重设计若引入新令牌，必须并入白名单而不是放行） | 并集；重设计新增令牌走「加进令牌表」，不得靠删规则 |

**A2 的硬要求**：这 5 个文件的重放必须是「**main 版断言 ∪ 重设计版断言**」，
不允许整文件覆盖；解完冲突后 test/turn_liveness_test.dart 的 21 条必须逐条仍在。

---

## 6. 未决问题 / 已知张力（不自行裁决）

1. **表演层「说话权归属」（V12/Q1）是未裁决项**——本轮不碰、不发明语义。
2. **A7 的守卫保险**：test/turn_liveness_test.dart 自身是重叠文件，理论上会在重放里
   被覆盖。按编排者「已有有效断言 → 不要重复加」的口径，本次**没有**再写一份
   asset_guard_turn_state 冗余副本，而是把风险登记在 §5，要求 A2 用并集解冲突。
   若编排者更希望「即使 A2 失手也有第二道网」，可低成本补一个只含 4 条核心
   不变式的 asset_guard_turn_state_test.dart（候选用例清单见 §5 左列）。
3. **stage → bridge 的转发只能结构守卫**：Live2DStage 无传输注入参数（:90-105），
   桥只由 web-only host 回调挂上。asset_guard_preset_dispatch_test.dart 的「取证」
   用例会在「VM 已能挂桥」时变红，届时应把这些结构断言升级为行为断言。
4. **A5 的前端边界**：clean_for_tts（红线 #2）本体在
   crates/live2d-ai-runtime/src/tts.rs + conversation_engine_tts_flow.rs，
   是 **Rust 回归的职责**；本 Flutter 网只守 TTS 配置面，不假装守了过滤逻辑。
5. **命名差异（记录，不改）**：任务书写 live2d_stage.dart 的 set_scales；
   main 上的真实方法名是 **applyActionScales**（:498），协议字段是
   sync.payload.actionScales。本次按真实名守。
6. **A1 动作包表本体**（9 包 + none）在 assets/actions/presets.json +
   crates/l2d-wasm-demo/src/preset/*；Flutter 侧只守「帧发得出去 + 帧形状不变」，
   「包表内容对不对」由 Rust 回归守。
7. **A8 不新增断言**：相关文件是 main 独有（非重放重叠），既有测试重放后仍在（§2 A8）。

---

## 7. 本次交付物

新增（4 个测试文件 + 本文档）：

- docs/architecture/frontend-asset-inventory.md（本文档）
- shell/flutter/test/asset_guard_director_cue_test.dart（4 条，结构）
- shell/flutter/test/asset_guard_preset_dispatch_test.dart（7 条：5 结构 + 1 行为 + 1 取证）
- shell/flutter/test/asset_guard_stage_attach_resend_test.dart（3 条，结构）
- shell/flutter/test/asset_guard_tts_config_test.dart（5 条，行为）

未改动：lib/** 一行未改；既有测试文件一行未改；未重放任何重设计文件；
未做任何 git 破坏性操作；crates/** 未碰。
