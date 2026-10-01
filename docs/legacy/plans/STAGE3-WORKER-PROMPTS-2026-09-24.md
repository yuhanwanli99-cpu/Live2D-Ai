> 历史（2026-10-01 归档，勿当现网）。

# 阶段3 worker 提示词真源（2026-09-24）

> 维护者落盘。**给编排者一句话**：读本文件 → 先按【Worker W9-Dart】【Worker W9-Rust】**原样分发**两个后台
> 子 worker（并行、文件零重叠）→ 都 settle 后按【合并收口 C1–C6】串行执行。
> 范围与裁决真源：`docs/legacy/plans/STAGE3-single-driver-plan-2026-09-24.md`（D10–D13）。

---

## Worker W9-Dart —— 退役 `latest.preset_id` 驱动通道

```text
【Worker W9-Dart · 退役 latest.preset_id 驱动通道，统一到 action_cue（阶段3）】

【工作树】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，HEAD dde21788，工作树干净）。
不要碰 /home/skystar/Live2D-Ai。

【先读】docs/legacy/plans/STAGE3-single-driver-plan-2026-09-24.md §1/§2/§3（现状与裁决 D10–D13）。

【授权文件（仅这些）】
  shell/flutter/lib/main.dart
  shell/flutter/lib/live2d/live2d_stage.dart
  shell/flutter/test/action_cue_test.dart（或新建 driver_policy_test.dart，二选一并在回报里说明）

【任务】
1. 删通道 B（main.dart）：
   - _applyDirectorPreset()（约 :484-514）、_directorTurnHandled、_directorFetching、_directorPresetGate 字段；
   - TextDeltaEvent / TextFallbackEvent 里那次 unawaited(_applyDirectorPreset())（约 :421-423）；
   - TurnStateEvent 里 _directorTurnHandled = false（约 :432-434）。
2. 删 live2d_stage.dart 的 DirectorPresetGate + DirectorPresetAction + DirectorPresetDecision（约 :55-101）。
3. 保留通道 A（_directorCues / ActionCueEvent / _applyDirectorCueForSeq）：确认 preset_id=="none" 会走到
   applyPreset('none')（撤销）；preset_id 为空串仍是「不动」。
4. 测试（缺一不可）：
   - action_cue 帧含 preset_id:"none" → 该句音频开始时**恰好一次** applyPreset('none')；同 seq 重复帧只应用一次。
   - 保留并跑通 action_cue_test.dart 其余用例；把针对 DirectorPresetGate 的三条删掉（它们测的是被删的类）。
   - 源码级断言：shell/flutter/lib/ 中不存在「state('director') + applyPreset」路径
     （仿 test/action_scales_wiring_test.dart 的源码扫描先例；回报里写清用的是哪种断言）。
   - 贴原始输出：grep -rn "mods/director/state" shell/flutter/lib/（应只剩 director_panel.dart）；
     grep -rn "_applyDirectorPreset\|DirectorPresetGate" shell/flutter/lib/（应为 0）。
【门禁】export PATH="$PATH:/home/skystar/flutter/bin"；cd shell/flutter && flutter analyze && flutter test；
  有 .dart 改动 ⇒ flutter build web --release --base-href /app/ --no-web-resources-cdn，并出三证据
  （① main.dart.js mtime 晚于最后 .dart；② grep -c 'gstatic.com/flutter-canvaskit' build/web/main.dart.js == 0；
   ③ 服务由编排者起后 ./scripts/ignite.sh --check 四项 ok）。
【回报】改动文件与关键 diff / 测试名 / analyze+test 原始数字 / 两条 grep 原始输出 / 三证据 / 未决。禁止「应该没问题」。
【红线】禁止 git 写操作；不改 RESEARCH 与 STAGE3 计划文件；不碰 /home/skystar/Live2D-Ai；不改 action_cue 帧结构。
```

---

## Worker W9-Rust —— 撤销经 `action_cue` 的 `none` cue + 文档对齐

```text
【Worker W9-Rust · 撤销经 action_cue 的 none cue + 文档对齐（阶段3）】

【工作树】/home/skystar/Live2D-Ai-l1。不要碰 /home/skystar/Live2D-Ai。
【先读】docs/legacy/plans/STAGE3-single-driver-plan-2026-09-24.md §0-§2（裁决 D10/D11/D12 必须体现在文档里）。

【授权文件（仅这些）】
  crates/live2d-ai-mod-director/src/plan.rs
  crates/live2d-ai-mod-director/src/lib.rs
  crates/live2d-ai-mod-director/src/tests.rs
  crates/live2d-ai-mod-director/src/tests_staging.rs
  crates/live2d-ai-desktop/src/app_event.rs（**仅注释**）
  docs/architecture/performance-layer-v0.md
  docs/architecture/director-mod-v0.md
  docs/plans/HANDOFF-2026-09-22-performance-layer.md
  AGENTS.md

【任务】
1. plan.rs：DirectorPlan::rule(epoch, preset_id, intensity, ttl_ms) 在 preset 为空 / "none" 时**不再返回空 plan**，
   改为产出一条 {sentence_seq:1, preset_id:"none", intensity(clamp 1..=3), ttl_ms(clamp 1..=5000),
   priority:PRIORITY_RULE} 的 cue；有非 none 预设时行为**逐字段不变**。
   头注写清「none cue = 撤销哨兵（D10）」。
   注意：arbiter.apply（arbiter.rs:41-50）不校验 preset id，director 规则路径不经 parse_plan 的能力集校验
   ⇒ "none" 可直接承载；在回归里钉住这一点。
2. lib.rs：start() 里「动作预设经只读状态面 latest.preset_id 交给前端」的日志与注释改成实情
   （latest 仅供面板展示；驱动唯一走 ModServices.cues → WS action_cue）；模块头注同步。
3. 回归（缺一不可）：
   - 中性轮（preset None）规则 plan 含**一条** preset_id=="none" 的 cue（不是空 cues）；
   - 有预设时规则 plan 与改动前逐字段相同；
   - 核对既有回归（tests.rs / tests_staging.rs）不因 cues_emitted / 账本计数语义变化而变假绿——逐条说明你如何确认。
4. 文档（D10/D11/D12 必须明文）：
   - performance-layer-v0.md：补「action_cue.cues[].preset_id=="none" = 该句音频开始时撤销两槽；
     cues:[] = 本轮不动」；§7 写明「前端拉取 latest.preset_id 驱动的通道**已退役**，latest 仅面板」+
     D11 的不对称（performance 开的中性轮 noop，靠 ttl 到点）。
   - director-mod-v0.md：把 §0/§18/§34/§39/§47-48/§224/§263-264/§341 里「只读状态面 + 前端拉取驱动舞台」
     的说法改成「面板只读；驱动走 action_cue」；channel 字段说明同步。
   - AGENTS.md：§27/§275/§294/§450 的 director 台账改成同一实情。
   - app_event.rs:233 注释补一句「前端拉取通道已退役」。
   - HANDOFF-2026-09-22-performance-layer.md §6 同步。
【门禁】cargo test --workspace --all-targets --no-fail-fast；cargo test --doc --workspace；
  cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；
  cargo run -p xtask -- rust-ratio（≥95%）。全部 tee 原始输出。
【回报】改动文件与关键 diff / 新增或修改测试名 / 门禁数字 / 文档改了哪些段落（逐处）/ 未决。禁止「应该没问题」。
【红线】禁止 git 写操作；不改 RESEARCH-actions-director-audit-2026-09-21.md 与 STAGE3 计划文件；
  不改 action_cue 帧结构；不碰 topics.rs / mod_registry.rs / supervisor.rs / desktop/src/main.rs。
```

---

## 合并收口 C1–C6（编排者串行执行）

```text
【合并收口 · W9-Dart / W9-Rust 都 settle 后，由你（编排者）串行执行】
C1 越权核对：git status --porcelain，只应出现两轨授权文件（外加维护者的 docs/plans/STAGE3-*.md，忽略）。
C2 Rust 全量门禁（--no-fail-fast + tee）：workspace all-targets / doc / fmt --check /
   clippy --all-targets -D warnings / rust-ratio ≥95%。
C3 Flutter analyze + test；flutter build web --release --base-href /app/ --no-web-resources-cdn 三证据
   （mtime / gstatic 0 / ignite --check 四项 ok）。
C4 主链回归：python3 scripts/verify_core_chain.py --timeout 300 --text 下午好 → 18 跳全过。
C5 驱动实测（两种配置，贴 HUD preset: 行 + WS 帧序列）：
   ①[performance] enabled=false + director 启用：动作由 action_cue 驱动；中性轮出现 preset_id=="none" cue → 两槽归零；
   ②[performance] enabled=true：动作由 action_cue 驱动（表演层）；中性轮 noop（不动）。
   注：live2d-ai.toml 是 gitignored 运行时配置，改前备份 /tmp，收工还原；确保 mods.json 里 director.enabled=true。
C6 grep 断言：grep -rn "mods/director/state" shell/flutter/lib/ 只剩面板；
   grep -rn "_applyDirectorPreset|DirectorPresetGate" shell/flutter/lib/ = 0。
【红线】git 写操作由维护者执行；不改 RESEARCH/STAGE3；不碰 /home/skystar/Live2D-Ai；密钥/.env/请求体不进报告。
【报告】按 W9-Dart / W9-Rust / C1–C6 分节；命令原文 + 原始输出 + 未决；自述无输出者退回；禁止「应该没问题」。
```
