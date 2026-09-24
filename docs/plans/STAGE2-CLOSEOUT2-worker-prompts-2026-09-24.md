# 阶段2 收口 · 第二轮 worker 提示词真源（2026-09-24）

> 维护者（顶层管理与代码审查）落盘。**给编排者一句话**：读本文件 → 先按【Worker A】【Worker B】两段
> **原样分发**两个后台子 worker（并行、文件零重叠）→ 全部 settle 后按【合并收口 S1–S6】串行执行。
>
> 背景：2026-09-24 第一轮只跑了收口编排者，A/B 两个执行 worker 从未被派发（工作树 porcelain=0、
> D1/D2 零改动）。本文件把「派 worker」与「收口」合并为一次可执行流程，消除粘贴目标错位。
> 裁决与现状真源：`docs/plans/STAGE2-CLOSEOUT-plan-2026-09-24.md` §7。

---

## Worker A —— 测试端 flaky 确定性修复（D1）

```text
【Worker A · 测试端 flaky 确定性修复（D1）】

【工作树】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，HEAD a7952412，工作树干净）。
不要碰 /home/skystar/Live2D-Ai。

【维护者授权】你唯一被授权改的文件：
  crates/live2d-ai-desktop/src/mod_registry.rs 的 **#[cfg(test)] 测试段**
  以及（如需）该 crate 测试用的新 helper 文件。
mod_registry.rs 属基座独占文件，本轮特批；**产品代码（#[cfg(test)] 之外的任何一行）一律不许改**。
若你判断必须改产品代码才能确定性同步 → 停下，把理由与最小方案交回维护者，不要自行改。

【背景（维护者已复核，直接用）】
- 失败用例：mod_registry::tests::event_delivers_to_running_mod（mod_registry.rs:1087-1101）。
- 原始失败：/tmp/flaky-close-16.log（594 passed / 1 failed，panicked at :1100）。
- 机制：`for _ in 0..10 { try_lock + RECEIVED.is_empty; sleep(50ms) }` = 最多 500ms 的墙钟轮询；
  并发负载（同时 3× flutter test）下 worker 投递超过 500ms ⇒ 断言失败。断言本身没错，错的是等待方式。

【任务】
1. 改为**确定性同步**：TestMod::on_event 收到事件时向测试发信号（mpsc::channel / Condvar / Barrier 均可），
   测试端阻塞等信号，不再 sleep 轮询。若保留超时，只能是「防挂死」兜底（如 recv_timeout(10s)），
   **不得**作为通过与否的判据；断言文本「worker 应将 event 投递到 running Mod 的 on_event」保持不变。
2. 审计同一 crate 测试里的同类轮询等待（`for _ in 0..N` + `sleep(` / `try_lock` 轮询）：列出全部命中；
   只改「等待事件送达」同类且确属时钟假设的用例，逐条给理由；不确定的只报告不改。
3. 取证：
   - 定向 10×：cargo test -p live2d-ai-desktop --bin live2d-ai-desktop mod_registry::tests::event_delivers_to_running_mod
   - 压测 ≥30×：cargo test --no-fail-fast -p live2d-ai-desktop --bin live2d-ai-desktop -- --test-threads=1
     （注意：--no-fail-fast 是 **cargo** 参数，必须在 -- 之前），每轮 tee 到 /tmp/flaky-A-$i.log；
     另一终端跑 flutter test 制造负载。30/30 绿才算过；一旦复现 → 立刻停，原始输出交维护者。
   - 反向证明：本地临时改回 sleep 轮询应能复现（证明修复真的绑定信号而非碰巧变快）；做不到如实说，不要编。

【门禁】虽只改测试段，仍须：cargo test --workspace --all-targets；cargo test --doc --workspace；
        cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings。（全部 tee 原始输出）
【回报】改动文件与关键 diff / 测试名 / 30 次逐次结果与日志路径 / 反证结果 / 未决问题。禁止「应该没问题」。
【红线】禁止 git 任何写操作；不改 RESEARCH-actions-director-audit-2026-09-21.md；不碰 /home/skystar/Live2D-Ai；
        密钥 / .env / 请求体不进报告。
```

---

## Worker B —— W7b：Developer 面板与「临时幅度覆盖」状态同步（D2）

```text
【Worker B · W7b：Developer 面板与「临时幅度覆盖」状态同步（D2）】

【工作树】/home/skystar/Live2D-Ai-l1。不要碰 /home/skystar/Live2D-Ai。

【维护者授权文件（仅这些）】
  shell/flutter/lib/settings/sections/dev_tools_section.dart
  shell/flutter/lib/app/shell_settings.dart
  shell/flutter/test/action_scales_wiring_test.dart（扩展）
  （ActionScalesSyncer 的 hasPin / pinned / active API 已存在，**不要改 action_scales_sync.dart**）

【背景（T4 实测 + 维护者复核）】
- W7 已把临时覆盖做成 ActionScalesSyncer 的显式 pin（优先级：临时覆盖 > 草稿 > 磁盘值），
  「改主题 / 调音量不冲掉 pin」已通过（HUD 1.50 为证）。
- 但**离开后重新进入** Developer 分区：面板滑条回 0.75（产品值），HUD 仍是 1.50。
  根因：DebugPanels.initState / didUpdateWidget 的 _syncScalesFromProduct() 只从 widget.productScales 播种，
  不知道 syncer 里有 active pin（dev_tools_section.dart:1417-1441）。

【任务（最小改动）】
1. 把「是否有 pin / pin 的值」以 readonly 方式传进 DebugPanels；播种规则 = `pinned ?? productScales`；
   didUpdateWidget 在 pinned 变化时也重新播种。
2. UI 明示两态：有 pin → 「临时覆盖生效中（不落盘，点『恢复产品设置』清除）」；无 pin → 维持现状。
   滑条位置必须与渲染面有效值一致。
3. 「恢复产品设置」语义不变（清 pin + 强制写回产品值）。
4. 不改 syncer 的优先级/去重语义；不改 main.dart / live2d_stage.dart；不改渲染面。
5. 测试（扩展既有 action_scales_wiring_test.dart，不要另建第二个文件）：
   - 有 pin 时面板初值 = pin（不是产品值）；
   - 清 pin 后面板回产品值；
   - 既有 8 条继续绿。

【门禁】export PATH="$PATH:/home/skystar/flutter/bin"；cd shell/flutter && flutter analyze && flutter test；
        改了 Dart ⇒ flutter build web --release --base-href /app/ --no-web-resources-cdn，
        并给三证据：① main.dart.js mtime 晚于最后 .dart；② grep -c 'gstatic.com/flutter-canvaskit' build/web/main.dart.js == 0；
        ③ 服务由编排者起后 ./scripts/ignite.sh --check 四项 ok。
【回报】关键 diff / 测试名 / analyze+test 实际数字 / 三证据原始输出 / 未决。禁止「应该没问题」。
【红线】禁止 git 写操作；不改 RESEARCH；不碰 /home/skystar/Live2D-Ai。
```

---

## 合并收口 S1–S6（编排者串行执行）

```text
【合并收口 · A/B 都 settle 后，由你（编排者）串行执行】
S1 越权核对：git status --porcelain。只应出现 A/B 授权文件（外加维护者的 `docs/plans/STAGE2-*.md` 两条文档改动，忽略它们）。
   A 若动 mod_registry.rs 的 #[cfg(test)] 之外、或 B 动授权外文件 → 停，报维护者。
S2 复核 A：定向用例 + 你亲跑 20×
   cargo test --no-fail-fast -p live2d-ai-desktop --bin live2d-ai-desktop -- --test-threads=1
   逐次 tee 原始输出；另一终端跑 flutter test 造负载；复现即停、交原始输出。
S3 复核 B：cd shell/flutter && flutter analyze && flutter test；
   flutter build web --release --base-href /app/ --no-web-resources-cdn 三证据
   （① main.dart.js mtime 晚于最后 .dart；② gstatic 计数 0；③ ignite --check 四项 ok）。
S4 合并收口门禁（串行、--no-fail-fast + tee 原始输出）：
   cargo test --workspace --all-targets / cargo test --doc --workspace / cargo fmt --all -- --check /
   cargo clippy --workspace --all-targets -- -D warnings / cargo run -p xtask -- rust-ratio（≥95%）。
S5 T6 复核（TTS 已在线 8080=200）：
   python3 scripts/verify_core_chain.py --timeout 300 --text 下午好   → 期望 18 跳全过；
   POST /api/v1/settings/test/llm（带 Origin）读 body 的 ok（不看 HTTP 状态）。
S6 T4 复核（B 修完后）：重进 Developer 分区，面板滑条应与 HUD 一致（1.50）且显示「临时覆盖生效中」；
   点「恢复产品设置」回产品值。贴两条原始读数。
【红线】git 写操作全部由维护者执行，你与 worker 零 git 写；不改 RESEARCH；不碰 /home/skystar/Live2D-Ai；
   密钥 / .env / 请求体不进报告。
【报告】按 Worker A / Worker B / S1–S6 分节；每节含命令原文 + 原始输出 + 未决；自述无输出者退回。
```

---

## 维护者亲核基线（2026-09-24 第二轮）

| 项 | 实测 |
| --- | --- |
| HEAD / 工作树 | `a7952412`，`porcelain=0` |
| D1 未实施 | `mod_registry.rs:1094-1098` 仍是 `0..10` + `sleep(50ms)` |
| D2 未实施 | `dev_tools_section.dart` 中「临时覆盖生效中」= 0 命中 |
| Gate 0 后基线门禁 | cargo 26 组 ok / 1396 passed；doc 3；fmt clean；clippy 0 warning；rust-ratio 97.1886% PASS；analyze 无问题；flutter test +1049 |
| T6（D3） | ✅ 已取证：8080=200；`verify_core_chain` 18/18；WS 277 audio 帧 / 264960 B PCM / 24000 Hz / `turn_state=completed` / error 0；`test/llm` body `ok:true` |
