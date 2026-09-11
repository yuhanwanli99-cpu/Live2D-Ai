# REFACTOR_PLAN.md — 重构执行计划

> 依据 AUDIT.md（2026-08-22）。原则：不改变外部行为；每项独立验证通过即 `refactor: <说明>` 提交；
> 测试失败立即回滚并记录；高收益低风险优先；克制——不为拆而拆。

## 批次排期

| # | 项 | 来源 | 改法摘要 | 验证方式 | 预计范围 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | 路径解析单源 utils/repo_paths.py | P-1 高 | 新建 find_repo_root()/resolve_shared_path()；model_upload/voice_library/slop_rules/service_context/run_server 五处薄委托替换 | 全量 pytest + 冒烟 import 链 + ruff | ~6 文件，±60 行 | ✅ 完成 |
| R2 | 死代码清除 | P-2 中 | 删 7 处零引用定义：types.py×2、live/live_interface.py（整包）、translate.NoOpTranslator、asr/utils.get_github_asset_url、sentence_divider.has_punctuation、chat_history_manager×2 | 全量 pytest（删前逐项 grep 复核） | ~6 文件，-90 行 | ✅ 完成 |
| R3 | print→logger 归一 | P-4 低 | 上游遗留 4 文件 5 处 print 改 loguru（logsetup 引导期 stderr 豁免） | pytest + ruff | 4 文件 ±10 行 | ✅ 完成 |
| R4 | deep_merge 移入 repo_paths 同批 utils | P-5 低 | 移至 utils/dict_utils.py 并改调用点 | pytest | 2 文件 | ✅ 完成（并入 R1 批次提交） |
| R5 | routes.py 嵌套 helper 提升模块级（第一步） | P-3 中 | 仅把 _load_yaml/_save_yaml/_write_raw_atomic/_load_persona/_save_persona/_apply_persona_to_data 等 ~20 个嵌套 helper 原样提升为模块级私有函数（闭包变量显式传参），端点行为零改动 | 设置中心 API 测试群 + 全量 pytest | routes.py 内部重排 | ⏸ 缓行——评估后认定收益<回归风险（helper 依赖 register_routes 闭包上下文 ctx，提升需大签名改造，违背行为等价克制原则）|
| R6 | settings-ui.ts 按 tab 拆分 | R-1 中 | 机械拆分渲染函数到子模块 | vitest+tsc+人工冒烟 | 大 | ⏸ 缓行（DOM 行为回归成本高，无人值守不做主观判定）|

## 明确不动（记录）
- renderer main.ts initApp 编排职责（已模块化到位）
- Android Live2DRenderer.kt / 全冻结面
- config_manager/* 上游 pydantic 配置类风格

## 执行记录
- 2026-08-22 R1+R4（b9a38aa）：utils/repo_paths.py 单源落地，五处实现归一；deep_merge → _deep_merge 私有化；pytest 421 绿、路径解析冒烟全对。
- 2026-08-22 R2（9f13945）：7 处零引用死代码清除 + types.py 失配导入清理；pytest 421 绿。
- 2026-08-22 R3（b4d154f）：fun_asr 两处 print → loguru；追加发现 utils/install_utils.py 整文件废弃零引用（v1.0.0 起）→ 直接删除（P-2 补充）；cosyvoice `__main__` CLI 冒烟 print 与 silero 演示 handler print 属合理豁免；pytest 421 绿。
