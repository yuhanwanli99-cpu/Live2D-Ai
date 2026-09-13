# docs/legacy/ —— 归档，**别当现网**

> 这里放的是**已经作废**的历史文档：Python/Android 双端时代（截至 2026-08）与更早的
> 迁移期产物。它们的结论大多**已经不成立**——现行真源是 [`AGENTS.md`](../../AGENTS.md)
> 与 [`docs/README.md`](../README.md)。
>
> 2026-09-13（rc.3）从**仓库根**搬进来：根目录原本堆着 8 份互相矛盾的旧计划，
> 每一份都长得像「当前计划」。保留而不删除，是为了「当时为什么这么想」还查得到；
> **不要**按它们写代码。

| 文件 | 时代 / 内容 | 为什么作废 |
|---|---|---|
| `AGENT.md` | 2026-09 项目总览（早于 `AGENTS.md`） | 被 `AGENTS.md` 取代；两者并存会让人读错 |
| `AUDIT.md` | 2026-08-22 优雅度审计 | 基线是已归档的 Python/Android 双端（pytest/vitest） |
| `REFACTOR_PLAN.md` / `REFACTOR_SUMMARY.md` | 2026-08-22 审计后的重构计划与总结 | 同上；分支 `refactor/elegance` 不在主线 |
| `PLAN.md` | 架构规划（Python/Android 双端，截至 2026-08） | 已被 Rust 主线取代 |
| `plan.md` | Implementation Plan V2（更早） | 同上 |
| `PROGRESS.md` | 2026-06 工程质量进度 | 双端栈时代 |

**没有搬走**的两类，都是因为「活的东西还指向它」：

- `docs/design/web-action-trigger-archive.md`：动作手动触发的接口档案，
  `shell/flutter/lib/ui/stage_corner_controls.dart` 与 `AGENTS.md` 都在引用它；
- `tests/` + `shared/`：仓库根的历史资产（Python 时代跨端协议文件），
  仍有 pytest 门禁在跑（`pr-checks.yml` 的 `root-py-tests`）。
