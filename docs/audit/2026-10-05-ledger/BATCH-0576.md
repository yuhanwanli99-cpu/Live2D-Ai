# BATCH-0576 落盘（无装饰）· docs/plans 139 份，AGENTS 逐名提到 12 份

## 跑了什么
git ls-files docs/plans/ | wc -l                      -> 139
把 139 个文件名逐个在 AGENTS.md 里 grep -q，统计命中数   -> 12
抽样：BACKLOG-0.2.0-closeout 0 / director-staging-wire 0 / actions-performance-round 1 / stageB-plan-and-status 0

## 四个可核点
1. 139 份计划里 AGENTS **逐名**提到 12 份（8.6%）。
2. AGENTS 的定位是**项目说明**（分层 / 门禁 / 变更历史 / 裁决台账），不是计划索引；

## 正面模式 P30（新）
P30 **「闭环」要分层**：裁决闭环 != 执行闭环。
  判据：看到「已闭环 / 已完成」，先问「哪一层的完成」。
  本批实例：brief.md:9 写「C1–C11 已正式裁决」（裁决层闭环），
  同一文件 :94 写「C10 无法自动化——列入手动验收」（执行层未闭环）。
  与 B0448「分支覆盖率是指标、两条路径是否等价才是判据」同族。

## 未核
那 12 份被 AGENTS 提到的是哪 12 份 · 127 份未提到的性质（历史件/未完成件）·
docs/README.md 是否承担了索引职责（它被本次审计改动过）· director 本体其余文件
