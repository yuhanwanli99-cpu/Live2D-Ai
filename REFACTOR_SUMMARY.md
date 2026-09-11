# REFACTOR_SUMMARY.md — 深度审计与重构总结

> 日期：2026-08-22 ｜ 分支 refactor/elegance ｜ 运行模式：无人值守（目标 goal-65c8e7e5）
> 范围：Live2D-Ai 全仓（PC Python / renderer TS / Android Kotlin），排除 vendor、构建产物与模型资产。

## 一、总览

| 阶段 | 结果 |
| --- | --- |
| 扫描 | AST 死代码扫描覆盖 305 个顶层定义 × 全语料引用计数；重复模式 grep 普查（路径解析/异常处理/日志风格/print 残留）；核心链路文件通读（websocket_handler/service_context/routes/tts_preprocessor/soullink-adapter/main.ts/Android core+data） |
| 审计 | AUDIT.md：PC 6 项 + Renderer 2 项 + Android 1 项，全部带 文件:行号 证据；命名/依赖/可测试性三维度未发现明显问题 |
| 计划 | REFACTOR_PLAN.md：R1-R4 高收益低风险执行，R5/R6 明确缓行（附理由），A-1/R-2 明确不动 |
| 执行 | 4 个重构提交，每项独立验证（pytest 全量 + ruff + 冒烟）后落库 |

## 二、提交清单（本轮接续会话）

| hash | 类型 | 内容 | 验证 |
| --- | --- | --- | --- |
| 10b688d | fix(p1) | soullink profile 直链解析修复（**404 回退根因**：引擎此前从未真正生效）+ 引擎动态 chunk 隔离 legacy 路径（109 kB 独立分块验证）+ 启动首错上屏；+4 回归单测 | vitest 408 绿、tsc clean、bundle 重建 |
| b85e384 | feat(p2) | SLOP 台词治理三件套：slop_miner.py 离线挖掘器 + shared/slop_rules.json（12 条策展种子）+ tts_filter 运行时正则治理（仅影响 TTS 音频）；14 新测试 | PC pytest 421 绿 |
| b9a38aa | refactor | **R1+R4**：utils/repo_paths.py 单源化五处仓库根/shared 解析（含 run_server.py 硬编码两层上的脆弱假设）；deep_merge 私有化 | pytest 421 绿 + 解析冒烟全对 |
| 9f13945 | refactor | **R2**：清除 7 处零引用死代码（AudioPayload/ConversationConfig/live 包/NoOpTranslator/get_github_asset_url/has_punctuation/chat_history_manager×2）+ 失配导入清理 | pytest 421 绿、ruff F 干净 |
| b4d154f | refactor | **R3**：fun_asr print→loguru ×2；删除整文件废弃零引用 utils/install_utils.py | pytest 421 绿 |

## 三、数字对账

- 审计扫描：234 py + 65 ts + 184 kt（tracked 源码文件口径）
- 死代码：305 个顶层定义扫描 → 9 候选 → 语料复核确认 8 处删除（7 + install_utils 整文件）；reset_runtime_rules 为测试活跃使用保留
- 测试基线变化：PC pytest 407 → **421**（SLOP +14）；renderer vitest 395 → **408**（soullink loader +4，28 files）
- 行为等价性：所有重构项零外部行为变化；唯一"行为修复"是 10b688d——它让 P1 已声明的 soullink 开关真正生效（属缺陷修复而非行为变更）

## 四、明确不做与理由（防过度工程）

| 项 | 不做理由 |
| --- | --- |
| R5 routes.py 嵌套 helper 提升 | helper 依赖 register_routes 闭包上下文，提升需大签名改造，收益 < 回归风险 |
| R6 settings-ui.ts(2127 行) 按 tab 拆分 | DOM 行为回归成本高，无人值守无法做主观视觉判定 |
| A-1 Live2DRenderer.kt(2687 行) | Android v0.1.0 冻结 + JVM 663/663 绿，动它违背克制原则 |
| config_manager/* 上游 pydantic 风格 | 上游同步成本 > 收益 |

## 五、遗留与移交

1. **P2 默认切换仍门控人工冒烟**（performanceEngine 默认 legacy）：切换 = 改一行默认值 + 删 legacy 文件，需人眼确认 soullink 表演效果后再执行。
2. **SLOP 规则表待真实数据策展**：用户机器上积累 chat_history 后运行 `python scripts/slop_miner.py chat_history/`，把高频候选人工筛入 shared/slop_rules.json。
3. **根目录 Live2D-Ai/tests/ 与 PC tests/ 是两套**：前者为旧数据一致性检查套件（本次未纳入门禁，存在与本轮无关的既有失败），建议后续轮次明确其定位或迁移。
4. 环境：PC 测试解释器固定用仓库根 .venv（Python 3.12.14，start.sh 同款）；uv run 会触发网络下载引导，离线环境勿用。

## 六、最终验证快照（2026-08-22）

- PC：`pytest tests/ -q` → **421 passed**（open-llm-vtuber/，.venv 解释器）
- renderer：`vitest run` → **408 passed (28 files)**；`tsc` strict clean；`vite build` 成功且 engine 独立分块
- lint：改动文件 ruff（项目默认规则族 F/E4/E7/E9）全绿
- git：工作树干净，全部成果以 refactor:/fix:/feat: 提交落库于 refactor/elegance 分支
