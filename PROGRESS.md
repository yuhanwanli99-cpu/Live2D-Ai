# PROGRESS — 无人值守执行跟踪

> 目标：①完成 PLAN-V3 P0-P2（soullink 表演引擎复用）②防回归严格测试 ③深度审计+大范围重构（不改外部行为）。
> 运行模式：无人值守；所有进度落盘本文件；重构提交信息 refactor:*；测试失败立即回滚并记录。

## 环境/工具备注
- 分支：refactor/elegance（基线快照自 main@af14074）
- get_goal 工具绑定异常（binding error），目标以本文件为准；每轮尝试重读
- 冒烟清单中"肉眼可辨"项无法无人值守验证 → P2 默认值切换被门控为【需人工冒烟后执行】，其余照常
- 基线（2026-08-22）：PC pytest **407 passed**；renderer vitest 28files/**408 passed**（+4 soullink loader 回归）；tsc strict clean

## 提交索引
| hash | 类型 | 说明 |
| --- | --- | --- |
| 45a45df | feat | 表演编排层+多LLM管家+插件做实+记忆UI+日志升级（基线代码快照） |
| 714f84a | docs | PLAN-V3/调研报告/plugin-sdk 契约等文档 |
| 10b688d | fix(p1) | profile 直链解析修复（soullink 模式 404 回退根因）+ 引擎动态 chunk 隔离 legacy 路径 + 启动首错上屏 |

## 阶段看板
| 阶段 | 状态 | 备注 |
| --- | --- | --- |
| P0 Spike（profile 生成+headless 冒烟） | ✅ 完成 GO | verification/soullink-p0-report.md；发现：非标彩蛋参数未自动入档（需手工扩展/LLM refine）；能力检测与人盘互证 |
| P1 双跑开关（performanceEngine=legacy|soullink + 适配器 + 单测） | ✅ 完成（c9585fb，收尾修复 10b688d） | soullink-adapter.ts+ws-bridge 三回调+main 相位钩子；10b688d 修掉 profile 直链 404 根因（此前 soullink 模式必回退 legacy）并实现引擎动态分块隔离；vitest 28files/408 passed；tsc clean；bundle 重建 |
| P2 默认切换+退役 legacy | 🔶 受限部分完成 | 默认值切换**门控人工冒烟**（无人值守不做主观判定）；legacy 退役同样建议人工确认后执行——两项均已就绪，切换成本=改一行默认值+删文件 |
| P2 台词质量治理（SLOP 脚本） | 未开始 | 下一轮：离线扫 chat 历史 JSONL → 候选 → rules.json → tts_filter 后正则治理 |
| 审计(AUDIT.md)/重构(REFACTOR_PLAN.md) | 未开始 | P1-P2 落库后启动 |

## P0 执行记录
- [x] npm 安装 engine/profile-generator@0.1.0-beta.1（锁版本）
- [x] 生成 shared/model-profiles/bai.profile.json（heuristic；FACS 24→mapped 20；usedCdi 16/128）
- [x] headless 冒烟 PASS：未知参数 0；眨眼幅度 0.887；呼吸波动 0.900
- [x] 报告落盘 verification/soullink-p0-report.md

## 已知风险/跳过记录
- （暂无）
