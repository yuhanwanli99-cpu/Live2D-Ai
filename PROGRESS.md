# PROGRESS — 无人值守执行跟踪

> 目标：①完成 PLAN-V3 P0-P2（soullink 表演引擎复用）②防回归严格测试 ③深度审计+大范围重构（不改外部行为）。
> 运行模式：无人值守；所有进度落盘本文件；重构提交信息 refactor:*；测试失败立即回滚并记录。

## 环境/工具备注
- 分支：refactor/elegance（基线快照自 main@af14074）
- get_goal 工具绑定异常（binding error），目标以本文件为准；每轮尝试重读
- 冒烟清单中"肉眼可辨"项无法无人值守验证 → P2 默认值切换被门控为【需人工冒烟后执行】，其余照常
- 基线（2026-08-22）：PC pytest **407 passed**；renderer vitest 27files/**395 passed**；tsc strict clean

## 提交索引
| hash | 类型 | 说明 |
| --- | --- | --- |
| 45a45df | feat | 表演编排层+多LLM管家+插件做实+记忆UI+日志升级（基线代码快照） |
| 714f84a | docs | PLAN-V3/调研报告/plugin-sdk 契约等文档 |

## 阶段看板
| 阶段 | 状态 | 备注 |
| --- | --- | --- |
| P0 Spike（profile 生成+headless 冒烟） | 进行中 | 见 §P0 |
| P1 双跑开关（performanceEngine=legacy|soullink + 适配器 + 单测） | 未开始 | 三态表演签名随适配器落地 |
| P2 默认切换+退役 legacy | 受限 | 默认切换需人工冒烟确认；退役/治理部分可先做 |
| 审计(AUDIT.md)/重构(REFACTOR_PLAN.md) | 未开始 | P1-P2 落库后启动 |

## P0 执行记录
- [ ] npm 安装 @soullink-emotion/engine@0.1.0-beta.1(runtime依赖) + @soullink-emotion/profile-generator@0.1.0-beta.1(dev)
- [ ] profile-generator 生成 shared/model-profiles/bai.profile.json
- [ ] headless 冒烟：输出参数 ⊆ bai 128 参数集；眨眼/呼吸曲线方差检查；非标参数(肩/胯/腿/翅/耳/光环)入档核对
- [ ] 报告落盘 verification/soullink-p0-report.md + commit feat(p0)

## 已知风险/跳过记录
- （暂无）
