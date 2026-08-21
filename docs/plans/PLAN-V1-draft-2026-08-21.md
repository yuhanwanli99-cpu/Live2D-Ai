# Live2D-Ai — v1 完成计划（接手版）

> 接手日期: 2026-08-21 | 上一里程碑: v0.1.0（本地 tag）+ PC 第四轮已提交（af14074）
> 本计划取代此前所有未完成计划（PLAN-PC-V1~V4），作为 v1 达成前的唯一执行口径。
> 裁决者: 项目作者。v1 确立后才 push 云端。

---

## 0. 接手基线（2026-08-21 实测）

### 0.1 版本与提交状态

| 项 | 状态 |
| --- | --- |
| Android 主端 | v0.1.0 闭环 ✅（JVM 663/663，真机验收 9/10） |
| PC 端已提交 | 第四轮 af14074（外部接入/背景图/去免费模型/UI 令牌化/历史持久化/首启引导/热切换/流式上屏/语音 ASR） |
| PC 端工作树 | **第五轮在途、未提交**（见 0.2 清单，全部绿门） |
| git | main 领先 origin 5 commits，本地未 push（符合 v0 口径） |

### 0.2 第五轮在途工作清单（接手时未提交）

| 模块 | 内容 | 接线状态 |
| --- | --- | --- |
| 插件系统 | `plugins/`（manifest/manager/host/native/native_runtime）+ `plugin_routes.py`（/api/plugins 增删查启停）+ server.py 装配 | ✅ 已接 server |
| 插件门控 | input-port / live-ingress / voice-assets / model-assets / config-pack 五门控（is_enabled 才放行） | ✅ external_input.py / live_event.py 已接 |
| 插件管理 UI | `renderer/src/plugin-manager.ts` + 设置中心「插件管理」分区（启用/停用/状态/钩子/原生-外部标识） | ✅ settings-ui.ts 已接 |
| TurnConductor | `turn/conductor.py` + `turn/types.py`（TurnBundle/ExpertStatus，并行专家 + 超时 fallback） | ⚠️ **未接对话链**（仅模块+测试） |
| 运行时插件 | MemoryPlugin / ContextCompactorPlugin / BodyDirectorPlugin / MoodTtsDirectorPlugin（`plugins/native_runtime.py`） | ⚠️ **注册但未 hook 进对话链** |
| ExpressionDirector | `director.py`，director LLM provider 配置化 | ✅ service_context.py 已接 |
| 上下文压缩 | `context_compactor.py`（分层摘要） | ⚠️ 未接入链 |
| 资源路由 | `model_upload.py` / `config_transfer.py` / `voice_library.py` / `live_event.py` / `frontend_version.py` | ✅ server.py 已接 |
| 前端配套 | body-motion / camera-state / config-transfer / live-event / model-upload / voice-library / plugin-manager | ✅ 已入构建产物（07-05 重建） |
| 配置/许可 | persona.yaml 移除旧表情控制与工具区块、model_registry、LICENSE/NOTICE/CREDITS/README 更新 | ✅ 一致性 C30 已验证 |

### 0.3 接手时发现并处理的问题

1. **测试污染仓库（已修复）**：`tests/test_consistency_parse.py` 的 C-A2 负例直接改写**真实** `Live2D-Ai-pc/open-llm-vtuber/model_dict.json`（neutral→12345），进程中断时 `finally` 不执行即污染仓库。接手时该文件已被污染（导致 pytest 2 失败 + 一致性 C26 失败），已 `git checkout` 还原并复测全绿。**根除方案列入阶段 0.2**。
2. **环境约束（持续生效）**：本机无 Android SDK（仅 adb）→ Android 改动只能静态实现+静态审查，需 Windows/CI 编译回归；无 GPU/浏览器 → 渲染热路径与真机 E2E 需真机轮次；无麦克风 → 语音链路上轮只验证到「音频送达触发 ASR」，识别质量未验。

### 0.4 接手时绿门数字

| 检查 | 结果 |
| --- | --- |
| PC 后端 pytest（`--ignore=tests/test_tts_bargein.py` 基线既有环境问题） | **309 passed, 6 skipped** |
| PC 前端 vitest | **23 files / 372 passed** |
| PC 前端 tsc --noEmit | ✅ |
| 跨端一致性 `shared/check_cross_platform_consistency.py` | **33/33 exit 0** |
| 模型注册表 `shared/validate_registry.py` | ✅ |
| 前端构建产物 | 与最新 renderer 源码同步（07-05） |

---

## 1. 目标与裁决口径（不变）

- **v1 = 作者裁决的五项标准全部满足**：
  1. AI+Live2D 完整闭环 ✅（v0.1 已达成）
  2. 扩展接入（插件 SDK / 契约接口 → 社区可做 mod）🔄 第五轮主体在途
  3. 内置设置 ✅ PC 完整；Android 基本具备
  4. 应用内配置改动 ✅ PC 完整；Android 部分
  5. Live2D 文件导入 + JSON 配置导入 🔄 后端/前端/测试在途，缺真机装配验证
- **push 条件**：v1 确立后；此前全部本地提交。
- **Android v0.2 候选**（HANDOVER §四）并入本计划阶段 3，不另立计划。

---

## 2. 阶段划分与任务

### 阶段 0：基线固化 + 在途收尾（本次可完成，无需真机）

| # | 任务 | 说明 |
| --- | --- | --- |
| 0.1 | 第五轮 CHANGELOG | 在 CHANGELOG.md 顶部补「第五轮：插件系统 + 资源/配置路由」条目（含验证数字 309/372/33-33） |
| 0.2 | 根除测试污染 | `test_consistency_parse.py` 负例改为在**临时副本**（tempfile 目录 + 环境变量重定向 registry 路径）上做变更，永远不触碰真实文件；补一条「负例跑完后真实文件字节不变」的守护断言 |
| 0.3 | 提交第五轮 | 分两个 commit：① 后端插件系统+运行时+路由+门控+测试；② 前端配套+persona/许可/文档更新。提交前重跑 0.4 全绿门 |
| 0.4 | 绿门脚本化 | `scripts/verify_all.py` 扩展为「一键全绿门」（pytest + vitest + tsc + 一致性 + 注册表），作为每轮收尾固定动作 |

### 阶段 1：插件真链（PLAN-PC-V4 P2 接线，v1 标准 2 的核心）

| # | 任务 | DoD |
| --- | --- | --- |
| 1.1 | TurnConductor 接入 `single_conversation.py` | 对话脑 A 流式主链零改动、不被阻塞；body/mood/memory 专家 `asyncio.gather` 并行 + 各自超时；超时/异常走确定性 fallback 且写入 expert_status；TurnBundle 汇总后分发 TTS/Live2D；新增端到端 pytest 锁「A 流式不被专家阻塞」 |
| 1.2 | MemoryPlugin 接线 | llm.before 注入 top-k facts（`cache/memory_store.json`）；「记住/忘掉」显式指令与每 N 轮自动写入；管理 UI（列表/删除/清空/关闭注入）挂插件管理页；注入失败只降级不炸链 |
| 1.3 | ContextCompactor 接线 | 超 token 阈值触发 recent_turns 压缩（turn→topic→long-term 分层）；原始轮次**始终落盘**（压缩不丢原文）；动作/情绪信号结构化保存不随摘要丢失；设置页/状态页显示上下文使用率 |
| 1.4 | body/mood director 输出进链 | sentence.after hook 产出 BodyFrame/MoodFrame/TtsFrame，接入现有 mood_engine 与 TTS instruction 分发（保留 P7-05 异常降级）；ExpressionDirector 与 body-director 职责合并明确（避免双导演） |
| 1.5 | 专家状态可见 | 每个专家 ok/running/timeout/fallback/disabled/error 落日志 + 插件管理页状态栏（复用 0.2 的 status 机制） |
| 1.6 | knowledge / web-search 决策 | 二选一：① 实现本地知识库检索 + 联网查询（M 量级）；② 正式降级为 v1.1 并在 UI 标注「stub 不参与运行」。**执行前与作者确认**，不允许 stub 继续虚挂 |

### 阶段 2：资产与配置能力装配验证（v1 标准 5）

| # | 任务 | DoD |
| --- | --- | --- |
| 2.1 | model-assets 验证 | 模型 zip 上传→校验→进 live2d-models→可选为默认→删除，真机装配级 E2E 全通（复用第二轮 20/20 装配框架） |
| 2.2 | voice-assets | 参考音频上传与克隆音色管理闭环；与 CosyVoice 声音克隆打通（Android 侧同源，列入 3.3） |
| 2.3 | config-pack | 配置导入/导出/重置 E2E（导入前校验、失败回滚，同设置中心语义） |
| 2.4 | 外部插件契约文档 | `docs/architecture/plugin-sdk.md` 扩写为可执行契约：manifest 字段、hook_points 枚举、entrypoint 协议、config_schema 表单生成规则、origin 判定；附一个最小可跑示例插件 |

### 阶段 3：Android v0.2（需 SDK/Windows 环境或 CI）

| # | 任务 | 优先级 | 说明 |
| --- | --- | --- | --- |
| 3.1 | 官方 C++ 渲染器模型加载接线（ENGINE 路径） | **高** | 呈现帧率 ~50→90-120fps 的最后一公里；保留 LEGACY 回退；改动必须过 JVM 回归 + 真机帧率复测 |
| 3.2 | sherpa 音色选择器 | 中 | 100 中文音色 + 默认 zf_001 真机验证 |
| 3.3 | CosyVoice 声音克隆 | 中 | 5 秒录音定制音色（与 2.2 voice-assets 共用契约） |
| 3.4 | FPS 角标呈现帧率 | 低 | 双数显示（循环/呈现）或标注，消除误读 |
| 3.5 | PC interrupt 端到端 pytest | 低 | 补 `control: interrupt` 发送端到前端的端到端用例 |

### 阶段 4：v1 发布收口（作者裁决）

| # | 任务 |
| --- | --- |
| 4.1 | 全量绿门：后端 pytest / 前端 vitest+tsc / Android JVM / 一致性 33-33 / smoke_check / 装配级 E2E 全绿且数字入 CHANGELOG |
| 4.2 | 文档定稿：README（v1 特性与路线图）、HANDOVER（v1 架构事实）、CHANGELOG、docs/README 索引；本计划标记「已执行」 |
| 4.3 | 许可核查：AGPL-3.0-only + 商业授权双许可齐备（LICENSE/NOTICE/CREDITS 已改，核查 SPDX 头与第三方清单） |
| 4.4 | 作者验收五项标准 → `git tag v1.0.0` → push 云端 |

---

## 3. 明确不做（沿用 PLAN-PC-V4 §5，继续有效）

主播相关、具体平台接入、个人陪伴/关系养成、领域分化专家、多角色群聊、插件市场在线安装、VRM/PNGTuber/MMD、AI 唱歌、跨设备云同步。

---

## 4. 风险登记

| 风险 | 缓解 |
| --- | --- |
| 本机无 Android SDK → 阶段 3 无法本地回归 | 静态实现 + 静态审查，CI/Windows 编译回归后再真机；非阻塞阶段 0-2 |
| 无 GPU/浏览器 → 渲染热路径、UI 视觉项无法本机验证 | 只做有单测覆盖的改动；视觉项集中到真机轮次一次验完 |
| 麦克风链路上轮未真机验证（授权/识别质量/VAD 中断） | 真机轮次重点补验，登记进 DoD |
| conf.yaml 内联 persona fallback 与 shared/persona.yaml 漂移 | 阶段 4 文档定稿时顺手消除（run_server 已自动注入，fallback 可删） |
| 测试负例直接写真实文件（本次已踩坑） | 阶段 0.2 根除 + 守护断言 |
| 阶段 1.6 knowledge/web-search 未定级 | 开工前与作者二选一，不虚挂 stub |

---

## 5. 阶段完成判据汇总

- **阶段 0**：第五轮已提交；`verify_all.py` 一键绿门；测试负例不再触碰真实仓库文件。
- **阶段 1**：TurnConductor 真链端到端测试绿；memory/compactor 注入与压缩在对话链生效且可观测；专家 fallback 全部可见；knowledge/web-search 已定级。
- **阶段 2**：模型/声音/配置三条资产链路装配级 E2E 全通；外部插件契约文档 + 示例。
- **阶段 3**：Android 项逐条过 JVM/CI 回归与真机验收（沿用 v0.1 SOP）。
- **阶段 4**：作者点头 v1 → tag + push。
