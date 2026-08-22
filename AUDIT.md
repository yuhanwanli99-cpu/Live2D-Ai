# AUDIT.md — 深度工程优雅度审计报告

> 审计日期：2026-08-22 ｜ 分支 refactor/elegance ｜ 基线：PC pytest 421 passed / renderer vitest 408 passed / tsc strict clean
> 范围：全仓源码（PC Python 234 文件、renderer TS 65、Android Kotlin 184；排除 vendor/前端产物/live2d 模型资产）
> 口径：只关注工程优雅（命名/结构/重复/职责/一致性/简洁性/耦合/可测试性），忽略安全加固、极致性能、跨平台兼容。
> 方法：AST 死代码扫描（305 个顶层定义 × 全语料引用计数）+ 重复模式 grep 普查 + 核心链路通读。每条均有文件:行号证据。

---

## P. PC Python（src/open_llm_vtuber/ + run_server.py）

### P-1 [重复与抽象] 高 — 仓库根/shared 路径解析存在 5 处独立实现
| 实现 | 位置 | 方式 |
| --- | --- | --- |
| `_repo_root()` | model_upload.py:30-32 | `Path(__file__).resolve().parents[4]` |
| `_repo_root()` | voice_library.py:35-36 | 同上，逐字重复 |
| `repo_root()` | utils/slop_rules.py:37-38 | parents[5]（utils 子目录深一层） |
| `_find_repo_shared_dir()` | service_context.py:769-781 | os.path 逐级向上找 shared/ |
| `_resolve_shared_path()` | run_server.py:38-44 | **硬编码「上两层=仓库根」**，文件移动即碎 |

service_context.py:774 的 docstring 自证了 run_server.py 存在语义重复的副本。
**建议**：新建 `utils/repo_paths.py` 单源（`find_repo_root()` / `resolve_shared_path()`），五处改为薄委托；run_server.py 的脆弱假设一并消除。
**风险**：低（纯等价替换，pytest 全量守护）。

### P-2 [简洁性] 中 — 确认死代码 7 处（AST 扫描 + 语料零引用验证）
| 定义 | 位置 |
| --- | --- |
| `AudioPayload` | conversations/types.py:12（仅本文件出现） |
| `ConversationConfig` | conversations/types.py:33 |
| `LivePlatformInterface` | live/live_interface.py:6（**整个 live/ 运行时包无任何导入方**；config_manager 的 `.live` 是配置类另一回事） |
| `NoOpTranslator` | translate/__init__.py:18 |
| `get_github_asset_url` | asr/utils.py:9 |
| `has_punctuation` | utils/sentence_divider.py:167 |
| `modify_latest_message` / `rename_history_file` | chat_history_manager.py:311 / :354 |

**建议**：逐项删除（live/ 整包删除前再人工确认一次），每删一批跑全量 pytest。
**风险**：低。

### P-3 [结构组织] 中 — routes.py 巨型 register_routes ≈2000 行
23 个端点 + ≥20 个嵌套 helper 全部内联在一个工厂函数里（如 /api/config POST 处理体 ~400 行、/api/test-tts ~195 行）。
**建议**：按域抽 helper 到模块级或拆分路由模块（config/persona/models/tts-test）。**风险**：中——端点行为必须靠设置中心 API 测试群回归；若拆模块则属结构性变更，本轮从克制原则可先只做"嵌套 helper 提升为模块级函数"这一步。

### P-4 [一致性] 低 — print() 残留于上游遗留文件
asr/fun_asr.py:17,99；vad/silero.py:205；tts/cosyvoice_cloud_tts.py:360；utils/install_utils.py:53。
logsetup.py 的 stderr 打印是 logger 引导期特例，豁免。
**建议**：改为 loguru logger。**风险**：低。

### P-5 [简洁性] 低 — deep_merge 放错位置
service_context.py:799 的通用字典合并工具，唯一调用点在同文件 ：712。
**建议**：移入 utils/（与 P-1 同文件批次）。**风险**：低。

### 该维度未发现明显问题的维度
命名与表达力（核心链路命名清晰、领域语言一致）；依赖与耦合（未发现循环导入）；可测试性（421 例测试覆盖良好，全局状态有 reset 钩子约定）。

## R. Renderer TypeScript（renderer/src/）

### R-1 [结构组织] 中 — settings-ui.ts 2127 行单入口
全部设置 tab 的渲染/绑定集中在 `initSettingsPanel()`（settings-ui.ts:236 起）。
**建议**：按 tab 拆子模块。**风险**：中（DOM 行为回归成本高）→ 本轮标记【缓行】，除非有行为等价的机械拆分把握。

### R-2 [结构组织] 低 — main.ts 863 行 initApp 编排巨函数
已部分模块化（pipeline/loader/camera 各自成文件）；剩余编排职责集中属可接受范围。记录不动。

## A. Android Kotlin（v0.1.0 冻结）

### A-1 [结构组织] 低 — Live2DRenderer.kt 2687 行渲染胶水巨文件
冻结态 + JVM 663/663 绿；动它违背克制原则 → 记录不动。
core/(16 文件) 与 data/(24 文件) 小而专一：命名、职责、一致性各维度未发现明显问题。

---

## 结论
高收益低风险重构项 = **P-1（路径单源）、P-2（死代码清除）、P-4（print→logger）、P-5（deep_merge 归位）**；
中风险缓行项 = P-3（routes 拆分第一步：helper 提升）、R-1（settings-ui 拆分）；
明确不动项 = R-2、A-1 及 Android 冻结面。
详见 REFACTOR_PLAN.md 执行排期。
