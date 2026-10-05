# BATCH-0004 · HTTP 对话入口 / 会话作用域 / say 注入

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`

## 读过的文件（来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/chat_routes.rs` — 1086（>500 行规则：单文件单独占一批）

## 为验证调用链而读的清单外文件（只读片段，不计入本批审计结论）
- `ws/broadcaster.rs`（BATCH-0003 已审，此处复用其 `broadcast` 语义）
- `dispatch.rs:168-189`（ChatPost 路由分支，核对 epoch 占位参数的说法）

## 跑过的命令（全部只读）
```
git ls-files 'crates/live2d-ai-desktop/src/web_api' | xargs wc -l | sort -rn
grep -c "tracing::" crates/live2d-ai-desktop/src/web_api/chat_routes.rs   # → 0
grep -c "println!|eprintln!" crates/live2d-ai-desktop/src/web_api/chat_routes.rs  # → 0
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 A（状态真源）：✔ 会话表只有一份——`handle.session_scopes()`（chat_routes.rs:241/337），
  baseline 是同表兄弟字段（头注 :19-24 声明，测试 `session_route_writes_and_reads_the_baseline_per_session`
  用「存进去的就是写回来的」逐值对照坐实）。**无第二份会话表** ✔
- 维度 C（epoch 门禁）：✔ dispatch.rs:169-171 的注释**属实**——chat_routes.rs:178 真的调
  `handle.current_epoch()`，占位参数在 :140 命名为 `current_epoch_unused` 并在 :179 显式丢弃。
  测试 `chat_with_supervisor_returns_200` 断言 `"epoch":0`（启动瞬间）真实有效（非恒真）。
- 维度 C（拒绝路径的副作用）：✘ 429 busy 之前已经改了状态并发帧 → F-0004-01
- 维度 B：✔ `chat_routes` 不持有线程/资源，无需 Drop；测试里每个 supervisor 都 `handle.quit()` 收尾
- 维度 E（契约一致性）：✔ `text_delta`/会话字段与 Dart 侧已核；✘ CT 校验顺序与错误码两套 → F-0004-03
- 维度 J（测试质量）：本文件测试**质量高**（真 supervisor、真 broadcaster 订阅端读真实帧原文、
  断言「不得补帧」用 `rx.try_recv().is_err()` 真检查队列为空）——与 F-0001-02 / F-0002-02 形成鲜明对比。
  唯一的问题测试是 `new_message_cancels_orchestration_without_backfill` 的**注释与断言范围不匹配**（见 F-0004-01 反证）。
- 红线 R：✔ 全文件不触碰密钥；`baseline` 走 `sanitize_session_id` 同一道闸（:363-366），
  且有专门测试拒 `../etc/passwd`（:1067）与超长（:1068-1071）

## 治理（行数）
- `chat_routes.rs` = **1086 行**，AGENTS.md 的硬上限是「源码 ≤500，豁免 ≤1000 需头注写理由」
  ⇒ **超出豁免上限本身**；且头注 :52 仍写「本文件当前约 560 行」——豁免理由与事实脱节 → F-0004-02
- 任务书 §9 的已知结构债清单列了 `mod_registry.rs 721` / `mods_routes.rs 958` / `persona/lib.rs 998`
  **未列** chat_routes.rs ⇒ 属未登记的新增超限

## 未核实项
1. 前端收到 429 时的 UI 行为（是否会重绘/提示）未读 Dart → F-0004-01 的用户可见性标 未核实。
2. `session_scope` 的 `MAX_SESSION_BASELINE_CHARS` 上限值未读（在 `crate::session_scope`，属另一批）。

## 本批新增
P0 0 · P1 0 · P2 2 · P3 1
