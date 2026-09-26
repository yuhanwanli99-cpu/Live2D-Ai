# 阶段4 worker 提示词真源（2026-09-26）

> 维护者落盘。**给编排者一句话**：4a（契约冻结）已随本任务书落盘，**零代码**；
> 读本文件 → 先按【Worker W4b】【Worker W4c】【Worker W4d】**原样分发**三个后台子 worker
> （并行、文件零重叠，见 [STAGE4-plan-2026-09-26.md](STAGE4-plan-2026-09-26.md) §3）→
> 都 settle 后派【Worker W4e】串行收口 → 最后按【合并收口 C1–C8】由你执行。
>
> 范围与判据真源：[STAGE4-plan-2026-09-26.md](STAGE4-plan-2026-09-26.md)（波次 4a–4e）
> 契约真源：[performance-protocol-v1.md](../architecture/performance-protocol-v1.md)（v1 唯一真源）
>
> **开工前置（硬）**：协议 **§12.1（O1–O16 裁决）+ §12.2（D22–D26 冻结）** 已于 2026-09-26 落定；
> 用户确认 V1（表演层 = 只断句 + 出 cues 的导演）。**分发时必须把 §12.1 + §12.2 原文贴进每个 worker
> 的【任务】顶部**；worker 不得再自行假设。若§12.1/§12.2 缺失 → 停下问维护者，不要派单。

---

## Worker W4b —— runtime 表演层 v1（`segments` 切分 + 三字段 cue）

```text
【Worker W4b · runtime 表演层 v1：segments 切分 + 三字段 cue（阶段4b）】

【工作树】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，HEAD ab47f0f0，工作树干净）。
不要碰 /home/skystar/Live2D-Ai 与其它历史 worktree。

【先读】
  1. docs/architecture/performance-protocol-v1.md 全文（v1 唯一真源；尤其 §2 schema、§3 segments 不变量、
     §10 失败/降级矩阵、§11 兼容矩阵、§12 待用户点头）。
  2. docs/plans/STAGE4-plan-2026-09-26.md §1/§2(4b)/§4.2/§4.6/§4.7（判据与负对照纪律）。
  3. docs/architecture/performance-layer-v0.md（现行实现面；speak 语义已作废）。
  4. crates/live2d-ai-runtime/src/performance/{plan.rs,mod.rs,prompt.rs,client.rs,tests.rs}（现行代码）。
  5. RESEARCH-actions-director-audit-2026-09-21.md §10.1 Q1（V1 裁决原文）。

【授权文件（仅这些）】
  crates/live2d-ai-runtime/src/performance/plan.rs
  crates/live2d-ai-runtime/src/performance/mod.rs
  crates/live2d-ai-runtime/src/performance/prompt.rs
  crates/live2d-ai-runtime/src/performance/client.rs
  crates/live2d-ai-runtime/src/performance/tests.rs
  crates/live2d-ai-runtime/src/conversation/engine.rs
  crates/live2d-ai-runtime/tests/conversation_engine_performance.rs
  crates/live2d-ai-desktop/src/web_api/ws/events.rs

【任务】（前置：用户已答复 §12 的 O1–O5；按答复实现，答复原文贴进回报）
1. plan.rs：新增 v1 schema（segments + cues[field,x,y,z?,id?,intensity,at,hold,ttl_ms?]），
   常量与 json_schema_strict 由同一组常量拼出（沿用 schema_and_validator_share_the_same_bounds 纪律）。
   - 核心不变量：segments.concat() == 主模型原文（逐码点）→ 不等 = 整份失败
     （错误码 performance_plan_segments_not_partition）；空原文 → segments=[]。
   - speak 字段保留可解析（V11：字段一律不删、缺省即旧语义）；只给 speak 时走 v0 旧路径。
   - segments 与 speak 同现 = **segments 优先、speak 忽略并 warn**（O1 裁决；**不整份失败**）。
   - body 给 z → 丢该键 + warn；轴越界钳位到 [-1,1]；intensity 钳位 1..=3；
     at 词表外 / 越界锚点（seg:0 或 seg:>段数）→ 整份失败；未知顶层键丢弃。
2. prompt.rs：提示词改成 v1 字段说明；【绝对不得】出现任何 Param… 参数名（协议 §5.1）。
3. engine.rs：按 **D22/D23/D24** 实现——
   - 表演层给的 segments **一对一**成为 TTS 单元/音频元素（`seg:N` ≡ `sentence_seq==N`），**不再过分句器二次切分**；
   - 每段**上屏文本 == 送 TTS 文本 == `clean_for_tts(段)`**（不得把原文的 Markdown 直接上屏）；
   - 纯空白段**仍产静音元素与 start/end 边界帧、seg 编号不跳**；
   - 回退路径（关/失败）仍用既有 SentenceAssembler 对 `clean_for_tts(原文)` 切多段（v0 行为不变）；
   - cue 锚段边界；只动（segments=[]）时给无声锚句。
4. mod.rs：回退矩阵扩 v1 原因码（见协议 §2.4 / §10），整份失败仍回退到
   segments=[clean_for_tts(原文)] + 规则 cue；未知字段宽容丢弃。
5. events.rs：v1 cue 投影到【既有】action_cue 帧；【不改帧结构、不新增帧类型、不删 preset_id】（V11）。

【必须落地的回归（名字可微调，语义不得变）】
  segments_must_be_a_verbatim_partition_of_the_source / segments_are_split_but_never_rewritten /
  legacy_speak_still_parses_when_segments_absent / both_segments_and_speak_prefers_segments_and_warns /
  anchor_beyond_segment_count_fails_the_whole_plan / body_z_is_dropped_and_axis_out_of_range_clamps /
  schema_and_validator_share_the_same_bounds / performance_prompt_never_contains_param_names /
  performance_cues_project_to_the_existing_action_cue_frame /
  segments_are_one_to_one_with_sentences_and_never_resplit (D22) /
  displayed_text_equals_tts_text_per_segment (D23) / blank_segment_keeps_its_seg_index (D24)

【负对照（D20，缺一不可）】
  B1「去掉 V1 拼接校验 → 改写会通过」与 B7「schema 常量与校验器不同源 → 边界漂移」各附
  ①命令原文 ②修改点 ③红输出 ④还原后绿输出。没有负对照的声明按自述处理、不进验收。

【门禁】
  cargo test --workspace --all-targets --no-fail-fast
  cargo test --doc --workspace
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo run -p xtask -- rust-ratio（≥95%）
  全部 tee 原始输出。

【回报】改动文件与关键 diff / 新增或修改测试名 / 五条门禁数字 / 负对照四段原文 /
  O1–O5 用户答复原文 / 未决。禁止「应该没问题」。自述无原始输出者退回。

【红线】禁止 git 写操作；不改 RESEARCH / STAGE3 / STAGE4 计划文件；不碰 /home/skystar/Live2D-Ai；
  不改 action_cue 帧结构；不删 speak / preset_id；不碰 topics.rs / mod_registry.rs / supervisor.rs /
  desktop/src/main.rs（确需改 → 停下问维护者）；不碰 4c/4d 的授权文件（渲染面 / Flutter）。
```

---

## Worker W4c —— 渲染面字段化通道 + 音频时钟 + 事件级 ack

```text
【Worker W4c · 渲染面字段化通道 + 音频时钟 + 事件级 ack（阶段4c）】

【工作树】/home/skystar/Live2D-Ai-l1（分支 mod/l1-product，HEAD ab47f0f0）。不要碰其它 worktree。

【先读】
  1. docs/architecture/performance-protocol-v1.md §4（三字段）、§5（映射原则 + 红线）、§6（锚点与音频时钟）、
     §7（ack 事件表）、§12（O6/O9/O10/O11/O12/O13 待用户点头点）。
  2. docs/plans/STAGE4-plan-2026-09-26.md §2(4c)/§3/§4.3/§4.6/§4.7。
  3. docs/architecture/action-packs-v0.md（intensity 语义 + §11 幅度标定，数值真源）。
  4. crates/l2d-wasm-demo/src/preset/{mod.rs,table.rs,scales.rs} 与 tests/；src/web/net.rs；
     src/web/surface/{input.rs,render.rs}；src/main.rs。
  5. RESEARCH-actions-director-audit-2026-09-21.md §10.4（stage-clock 推荐）、§10.7（可实现清单）。

【授权文件（仅这些）】
  crates/l2d-wasm-demo/src/preset/mod.rs
  crates/l2d-wasm-demo/src/preset/table.rs
  crates/l2d-wasm-demo/src/preset/scales.rs
  crates/l2d-wasm-demo/src/preset/tests/**（含新增文件）
  crates/l2d-wasm-demo/src/web/net.rs
  crates/l2d-wasm-demo/src/web/surface/input.rs
  crates/l2d-wasm-demo/src/web/surface/render.rs
  crates/l2d-wasm-demo/src/main.rs
  assets/actions/presets.json
  assets/actions/preset_labels.json

【任务】（前置：用户已答复 §12 的 O6 / O9 / O10 / O11 / O13；按答复实现，答复原文贴进回报）
1. 字段化通道：body / head / expression → 每皮套映射表；多轴按比例合成；同类 cue 按 add 合成
   （先加后钳）；expression 只写五官（ParamMouthForm / ParamEyeL*Open / ParamEyeR*Open /
   ParamEyeL*Smile / ParamEyeR*Smile / ParamBrowLY / ParamBrowRY），
   【绝不写 ParamMouthOpenY】（口型归 TTS）、【绝不写 ParamAngle* / ParamBodyAngle*】（V3）。
2. hold=true 保持到下次指令、不自动回基准、不产生 preset_expired；hold=false 按 ttl_ms 到点回。
3. stage-clock 消息 {seg,pos_ms,playing} 替代 performance.now() 当 now_ms；无 clock（段 A）回落墙钟；
   交接点 = 首个音频 start（既有 sentenceStarts + first_chunk）。
4. 事件级 ack：preset_applied / preset_replaced / preset_expired / preset_dropped + segment_ended；
   payload 见协议 §7.2（含 clamped / degraded / reason）；【不做每帧状态流】。
   **wire 名冻结（O13）**：ack 用独立 type `preset-applied`/`preset-replaced`/`preset-expired`/`preset-dropped`
   与 `segment-ended`；stage-clock 用 `{version:1,type:"stage-clock",payload:{seg,pos_ms,playing}}`。**不得改名**。
   皮套缺参数必须发 preset_dropped，【不得静默无反应】。
5. 白名单【保持现状】：ALLOWED_PARAMS 的 13 个标准参数集合不动；不开非标准参数（翅膀/耳朵…，O11）。
   每皮套基础强度 = 现有 [action] 三倍率（协议 §5.3）；幅值标定与通道上限沿用 scales.rs，不另立一套。

【必须落地的回归（名字可微调，语义不得变）】
  body_head_expression_map_to_the_documented_channels / expressions_only_write_facial_channels /
  same_field_cues_add_before_clamping / new_batch_replaces_the_field_animation_value_adds (D26) /
  hold_cue_never_auto_returns_and_never_expires /
  non_hold_cue_expires_at_ttl_and_emits_preset_expired /
  missing_param_emits_preset_dropped_instead_of_silent_noop /
  clock_message_replaces_wall_clock_in_frame_advance / segment_ended_is_emitted_once_per_segment

【负对照（D20，缺一不可）】
  C2「去掉 expression 只写五官的约束 → 头身被表情抢占」与 C6「去掉缺参数 ack → 静默无反应」各附
  ①命令原文 ②修改点 ③红输出 ④还原后绿输出。

【门禁】
  cargo test -p l2d-wasm-demo（原生；含新增回归）
  cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo
  改了 preset/ 或 assets ⇒ 视需要跑 trunk build 并说明产物位置（dist/ 被 gitignore）。
  原始读数：HUD preset: 行原文 + ack 事件 JSON 序列原文（贴帧，不转述）。

【回报】改动文件与关键 diff / 回归名与数字 / 负对照四段原文 / HUD 与 ack 原始序列 /
  O6/O9/O10/O11/O13 答复原文 / 未决。禁止「应该没问题」。

【红线】禁止 git 写操作；不改 RESEARCH / STAGE 计划文件；不碰 /home/skystar/Live2D-Ai；
  不改 action_cue 帧结构；不删 preset_id 旧路径；不改口型通道；不碰 4b/4d 的授权文件。
```

---

## Worker W4d —— 前端接线（ack 消费 + 取消纪律 + 时钟下发）

```text
【Worker W4d · 前端接线：ack 消费 + 取消纪律 + 时钟下发（阶段4d）】

【工作树】/home/skystar/Live2D-Ai-l1（分支 mod/l1-product，HEAD ab47f0f0）。不要碰其它 worktree。

【先读】
  1. docs/architecture/performance-protocol-v1.md §6（锚点/音频时钟/断开不补帧）、§7（ack）、§8（唤醒条件）、
     §9（会话 baseline）、§11（兼容：遇未知字段忽略）。
  2. docs/plans/STAGE4-plan-2026-09-26.md §2(4d)/§3/§4.4/§4.6/§4.7（D14 口径）。
  3. docs/plans/STAGE3-single-driver-plan-2026-09-24.md §4（既有验收口径，可参考形式）。
  4. shell/flutter/lib/live2d/live2d_stage.dart、lib/main.dart、lib/audio/audio_player.dart、
     test/action_cue_test.dart、test/action_scales_wiring_test.dart（源码扫描先例）。

【授权文件（仅这些）】
  shell/flutter/lib/main.dart
  shell/flutter/lib/live2d/**
  shell/flutter/lib/audio/**
  shell/flutter/test/**（含新增测试文件）

【任务】
1. 消费渲染面 ack 四条 + segment_ended → 汇入日志文本（喂导演）；遇未知字段忽略（V11）。
2. 下发 stage-clock {seg,pos_ms,playing}（30ms 或每帧）给渲染面；段边界用现有 sentenceStarts，
   并接住新的「段结束」事件。
3. 停止 / 新消息：清动作 + 表情 + TTS 待播 + 回该会话 baseline；同步取消、【不补帧】。
4. 【删掉前端猜 preset 剩余时长的本地定时器】（v0 的 presetStatus 类实现）——真源在渲染面（V8）。
   不得建立前端镜像、不得在前端复制状态机。
5. 不改 WS action_cue 帧结构；不改 action_cue 的应用语义（preset_id=="none" 仍撤销、空串仍不动）。

【必须落地的回归 / 断言】
  ack_events_are_consumed_and_forwarded_to_the_director_log
  stage_clock_is_sent_at_thirty_ms_cadence_while_playing
  stop_and_new_message_clear_action_expression_and_pending_tts
  源码扫描 no_client_side_preset_ttl_prediction：
    【同一文件同时含「本地计时器推演 preset 剩余时长」与 applyPreset 即红】——
    用 Directory/File 扫描 + 语义判定（仿 action_scales_wiring_test.dart / motion_wiring_test.dart），
    【不是】字面 grep 清单；回报里写清用的是哪种断言（D14）。
  unknown_fields_in_render_events_are_ignored

【负对照（D20，缺一不可）】
  D4「去掉源码扫描断言 → 前端猜时长的实现可混回」附 ①命令原文 ②修改点 ③红输出 ④还原后绿输出。

【门禁】
  export PATH="$PATH:/home/skystar/flutter/bin"
  cd shell/flutter && flutter analyze && flutter test
  有 .dart 改动 ⇒ flutter build web --release --base-href /app/ --no-web-resources-cdn，并出三证据
  （① main.dart.js mtime 晚于最后 .dart；② main.dart.js 不含 gstatic.com/flutter-canvaskit；
   ③ 服务由编排者起后 ./scripts/ignite.sh --check 四项 ok）。

【回报】改动文件与关键 diff / 测试名与 analyze+test 原始数字 / 三证据 / 负对照四段原文 /
  源码扫描用的断言方式 / WS 帧序列原文 / 未决。禁止「应该没问题」。

【红线】禁止 git 写操作；不改 RESEARCH / STAGE 计划文件；不碰 /home/skystar/Live2D-Ai；
  不改 action_cue 帧结构；不在前端复制核心逻辑 / 不建前端镜像；不碰 4b/4c/4e 的授权文件。
```

---

## Worker W4e —— 会话 baseline + host 接线 + 收口（**串行**，4b/4c/4d 全 settle 后）

```text
【Worker W4e · 会话 baseline + host 接线 + 收口（阶段4e）】

【工作树】/home/skystar/Live2D-Ai-l1（分支 mod/l1-product）。不要碰其它 worktree。
【前置】4b / 4c / 4d 已 settle 且各自门禁通过；本轨【串行】执行。

【先读】
  1. docs/architecture/performance-protocol-v1.md §9（会话 baseline）、§10（失败/降级）、§12（O7/O8/O15）。
  2. docs/architecture/session-scope-l1.md（会话表 / 会话 id 闸 / prompt_for 语义）。
  3. docs/architecture/director-mod-v0.md（V12 事实 + 零调用红线）。
  4. docs/plans/STAGE4-plan-2026-09-26.md §2(4e)/§3/§4.5/§4.6/§4.7。
  5. crates/live2d-ai-desktop/src/session_scope.rs；crates/live2d-ai-mod-director/src/**。

【授权文件（仅这些）】
  crates/live2d-ai-desktop/src/session_scope.rs
  crates/live2d-ai-desktop/src/web_api/chat_routes.rs
  crates/live2d-ai-desktop/src/web_api/dispatch.rs
  crates/live2d-ai-desktop/src/web_api/settings.rs
  crates/live2d-ai-mod-director/src/**
  docs/plans/STAGE4-CLOSEOUT-2026-09-26.md（新建收口报告）

【任务】（前置：用户已答复 §12 的 O7 / O8 / O15；按答复实现，答复原文贴进回报）
1. session_scope.rs：新增 baseline 作为 prompt_for(...) 的【兄弟字段】——同一张会话表、同一会话键、
   同一 sanitize_session_id 闸；【不新造第二套会话表、不加新机制】（V10）。
2. host：停止 / 新消息 → 清动作 + 表情 + TTS 待播 + 回该会话 baseline（V10）；
   与 4d 的取消纪律对齐；【不补帧】。
3. director 二路产出面能表达三字段（V12 事实下【只产 cues】、不产文本）；action_tx / apply_settings
   零调用红线不放松。
4. 写收口报告 docs/plans/STAGE4-CLOSEOUT-2026-09-26.md：门禁双来源 + 负对照清单 + 残留。

【必须落地的回归（名字可微调，语义不得变）】
  baseline_is_bound_per_session_and_not_global / stop_returns_to_the_session_baseline /
  new_message_cancels_orchestration_without_backfill
  既有 action_tx / apply_settings 零调用回归必须仍绿。

【负对照（D20，缺一不可）】
  E2「去掉 baseline 绑定 → 退回全局而非会话 baseline」附 ①命令原文 ②修改点 ③红输出 ④还原后绿输出。

【门禁】
  cargo test --workspace --all-targets --no-fail-fast；cargo test --doc --workspace；
  cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；
  cargo run -p xtask -- rust-ratio（≥95%）；全部 tee。
  cd shell/flutter && flutter analyze && flutter test。
  python3 scripts/verify_core_chain.py --timeout 300（贴跳数）。
  开工前查内存与 TTS 实例数（D21，避免双实例 OOM）。

【回报】改动文件与关键 diff / 回归名与数字 / 五条门禁数字 / 负对照四段原文 /
  驱动实测（两种配置：HUD preset: 行 + WS 帧序列 + ack 序列）/ O7/O8/O15 答复原文 / 未决。
  禁止「应该没问题」。

【红线】禁止 git 写操作；不改 RESEARCH / STAGE 计划文件；不碰 /home/skystar/Live2D-Ai；
  不碰 topics.rs / mod_registry.rs / supervisor.rs / desktop/src/main.rs（确需改 → 停下问维护者）；
  不碰 4b/4c/4d 已改的文件；不把密钥 / .env / 请求体写进报告与文档。
```

---

## 合并收口 C1–C8（编排者串行执行）

```text
【合并收口 · 4b/4c/4d/4e 都 settle 后，由你（编排者）串行执行】
C1 越权核对：git status --porcelain，路径集合必须 ⊆（各波次授权文件 ∪ 维护者的
   docs/plans/STAGE4-*.md）；出现任何未授权路径 → 该轨退回，不进入收口。
C2 契约一致性：逐条核对实现与 performance-protocol-v1.md（schema / 不变量 / ack / 锚点 / 兼容矩阵）
   是否一致；不一致 → 回改文档或退回实现（不得两边各说各话）。
C3 Rust 全量门禁（--no-fail-fast + tee）：workspace all-targets / doc / fmt --check /
   clippy --all-targets -D warnings / rust-ratio ≥95%。
C4 Flutter analyze + test；build web 三证据（mtime / 无 gstatic canvaskit / ignite --check 四项 ok）。
C5 端到端 + 驱动实测（两种配置，贴 HUD preset: 行 + WS 帧序列 + ack 序列）：
   ① [performance] enabled=false + director 启用；
   ② [performance] enabled=true。
   live2d-ai.toml / mods.json 是 gitignored 运行时配置：改前备份 /tmp，收工还原。
C6 verify_core_chain.py --timeout 300：贴跳数（主链无回归）。
C7 负对照汇总：把 §4.6 强制清单的 ①–④ 原始输出逐条并进报告；缺一即按自述处理。
C8 判据核对（D14）：确认没有任何判据是「字面 grep 命中/不命中」；源码扫描判据都写成
   「同一文件同时含 A 与 B 即红」并说明了断言方式。
【红线】git 写操作由维护者执行；不改 RESEARCH / STAGE3；不碰 /home/skystar/Live2D-Ai；
  密钥 / .env / 请求体不进报告。
【报告】按 W4b / W4c / W4d / W4e / C1–C8 分节；命令原文 + 原始输出 + 未决；
  自述无输出者退回；禁止「应该没问题」。
```
