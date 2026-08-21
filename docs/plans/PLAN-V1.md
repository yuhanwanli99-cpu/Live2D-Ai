# Live2D-Ai — PC 端 v1 完成计划（仅 PC，2026-08-21 定稿）

> 接手日期：2026-08-21 | 上一里程碑：v0.1.0 + PC 第四轮已提交（`af14074`）
> 范围：**仅 PC 端**（`Live2D-Ai-pc/` + 其读取的 `shared/` 数据）。Android 端冻结在 v0.1.0，不在本计划内。
> 本文件取代 `PLAN-PC-V1~V4` 与 `PLAN-V1-draft-2026-08-21.md`，是 v1 达成前的唯一执行口径。
> 裁决者：项目作者。v1 确立后才 push 云端；此前一切提交只进本地 git。

---

## 0. 接手实测基线（2026-08-21，本机复核）

### 0.1 仓库状态

| 项 | 实测结果 |
| --- | --- |
| HEAD | `af14074`（PC 第四轮：外部接入/背景图/UI/历史持久化/流式上屏/语音 ASR） |
| 分支 | `main`，领先 origin 5 个 commit，本地未 push（符合 v0 口径） |
| 工作树 | **PC 第五轮在途**：37 个已跟踪文件改动 + 44 个未跟踪文件（插件系统/资源路由/前端模块/测试/计划文档） |
| 本机环境 | 无 Android SDK、无 GPU/浏览器/麦克风；PC 后端/前端单测与构建可跑 |

### 0.2 接手实测绿门（PC 相关）

| 检查 | 命令 | 实测结果 |
| --- | --- | --- |
| 根目录 pytest | `.venv/bin/python -m pytest tests/ -q` | ✅ **309 passed, 6 skipped** |
| PC 后端 pytest | `cd Live2D-Ai-pc/open-llm-vtuber && ../../.venv/bin/python -m pytest tests/ -q --ignore=tests/test_tts_bargein.py` | ❌ **296 passed, 1 failed**（见 0.3） |
| PC 前端 vitest | `cd renderer && npx vitest run` | ✅ **23 files / 372 passed** |
| PC 前端 tsc | `cd renderer && npx tsc --noEmit` | ✅ exit 0 |
| PC 前端生产构建 | `cd renderer && npm run build` | ✅ exit 0（产物写入 `frontend/`） |
| 跨端一致性 | `python shared/check_cross_platform_consistency.py` | ✅ **33/33**（含 Android 静态比对，不改 Android 代码，继续作为数据契约绿门） |
| 模型注册表 | `python shared/validate_registry.py` | ✅ All checks passed |

### 0.3 接手时发现的问题（必须在本计划内解决）

1. **PC 后端 1 个 RED（契约过期）**
   `open-llm-vtuber/tests/test_shared.py::test_s103_system_prompt_contains_majority_emotion_keys`
   仍要求 `shared/persona.yaml` 的 system_prompt 含 ≥6 个 emotion key；第五轮已按新契约把「表情控制/表情强度/静态工具区块」从 persona 移除，根测试 `tests/test_persona_shared.py` 已改为「不得存在旧区块」。两个口径矛盾，需把旧 PC 测试更新到新契约。

2. **测试会写真实仓库文件（污染风险）**
   `tests/test_consistency_parse.py::test_c26_negative_pc_emotionmap_subprocess_mutation_and_restore`
   直接改 `Live2D-Ai-pc/open-llm-vtuber/model_dict.json` 再靠 `finally` 还原；进程被强杀时真实文件即被污染（已实际发生过）。必须改成临时副本 + 环境变量重定向 + 真实文件 sha256 守护断言。

3. **一键绿门不完整**
   `scripts/verify_all.py` 只跑 health / registry / 一致性 / 根 pytest；PC 后端 pytest 路径不可靠，且缺前端 vitest / tsc / build。需扩成 PC 一键全量绿门。

4. **工作树噪声**
   `renderer/nohup.out` 是运行残留，不应入库；提交前清理并确认 `.gitignore` 覆盖。

5. **ruff 历史债**
   PC 端 `ruff check src tests` 现存 **1102** 条历史告警，不在 v1 判据内全量清零；新写模块尽量干净，v1 不因历史 lint 债卡发布。

---

## 1. 目标与裁决口径（PC 版）

**v1 五项标准中，PC 端负责满足：**

1. AI + Live2D 完整闭环（文本/语音 → LLM → 表情/口型/动作）—— PC 已达成 ✅
2. 扩展接入（插件 SDK / 契约接口 → 社区可做 mod）—— 第五轮在途，阶段 1/3 完成 🔄
3. 内置设置（应用内可配置）—— PC 完整 ✅
4. 应用内配置改动（不靠改代码/硬编码）—— PC 完整 ✅
5. Live2D 文件导入 + JSON 配置导入 —— **由 PC 端提供**，阶段 3 完成 🔄

**已定级（不再悬置）：**

- **D1 已决**：knowledge / web-search **进 v1 实做**（阶段 2）。
- **D2 已决**：v1 标准 5 **仅要求 PC 端**；Android 不纳入本计划，发布说明中注明。

---

## 2. 阶段划分总览

| 阶段 | 主题 | 本机可完成 | 前置 |
| --- | --- | --- | --- |
| 阶段 0 | 基线固化 + 第五轮收尾 | ✅ | 无 |
| 阶段 1 | 插件真链（TurnConductor / 记忆 / 压缩 / 导演） | ✅ 代码+单测；真实语音/视觉体验待真机轮 | 阶段 0 |
| 阶段 2 | 知识库 + 联网搜索实做（D1） | ✅ mock 可测；真实联网需外网冒烟 | 阶段 1 |
| 阶段 3 | 资产与配置能力装配验证 + 插件契约（v1 标准 5） | ⚠️ API 级 E2E 本机；浏览器视觉项待真机轮 | 阶段 2 |
| 阶段 4 | v1 发布收口 | ✅ 文档/绿门本机；作者裁决 | 全部 |

---

## 3. 阶段 0：基线固化 + 第五轮收尾（立即执行，本机完成）

| # | 任务 | 完成判据（DoD） |
| --- | --- | --- |
| 0.1 | 更新 `open-llm-vtuber/tests/test_shared.py` 过期 persona 断言 | `S1-03` 改为「system_prompt 不含旧表情控制/表情强度/静态工具区块」，与根 `tests/test_persona_shared.py` 口径一致；PC 后端 pytest 全绿 |
| 0.2 | 根除一致性负例仓库污染 | `shared/check_cross_platform_consistency.py` 增加测试专用环境变量覆盖（`LIVE2D_AI_PC_MODEL_DICT`，只影响 C26 读取路径）；`test_consistency_parse.py` 负例改为在 `tempfile` 副本上变更 + 子进程传环境变量；新增「测试前后真实 `model_dict.json` sha256 不变」守护断言 |
| 0.3 | 扩展 `scripts/verify_all.py` 为 PC 一键绿门 | 默认跑：health + registry + 一致性 + 根 pytest + PC 后端 pytest（正确 cwd/.venv，保留 `test_tts_bargein` 环境豁免）+ 前端 vitest + tsc + build；退出码 0 才算全绿 |
| 0.4 | 前端产物同步与工作树清理 | `npm run build` 后 `frontend/` 产物与最新 renderer 源码一致；删除/忽略 `renderer/nohup.out`；`cache/`、`chat_history/`、运行时状态不入库 |
| 0.5 | 补第五轮 CHANGELOG | 顶部新增「第五轮：插件系统 + 资源/配置路由 + 运行时插件」，写入最终绿门数字 |
| 0.6 | 敏感信息复查 | `git diff` + 未跟踪文件逐项确认不含真实 Key/token/对话数据；`.env`、`conf.yaml` 保持忽略 |
| 0.7 | 提交第五轮（分 commit） | ① 后端：插件宿主/管理/门控/运行时/资源路由/TurnConductor 骨架 + 根 tests + 一致性脚本修复；② 前端：renderer 新模块与测试 + 构建产物；③ persona/registry/许可/文档/计划更新。提交前 0.3 一键绿门通过；提交后 `git status` 除本机忽略文件外干净 |

**阶段 0 完成判据**：PC 后端 pytest 297/297、根 pytest 309+、前端 372、tsc/build 过、一致性 33/33；工作树干净、第五轮已提交、一键绿门可复现。

---

## 4. 阶段 1：插件真链（v1 标准 2 的核心）

> 原则：LLM-A 是热路径、流式、绝不被专家阻塞；专家并行 + 各自超时；一切 fallback 必须可见。

| # | 任务 | DoD |
| --- | --- | --- |
| 1.1 | TurnConductor 接入 `single_conversation.py` | 核心对话脑 A 流式主链**零阻塞**；body/mood/memory/compactor 以 `asyncio.gather` 并行、独立超时；超时/异常走确定性 fallback 并写入 `ExpertStatus`；`TurnBundle` 汇总后分发 TTS 与 Live2D 参数；新增端到端 pytest 锁定「专家 sleep/抛错时 A 仍逐 token 产出」 |
| 1.2 | MemoryPlugin 真接入 | `llm.before` 注入 top-k facts（`cache/memory_store.json`）；支持“记住/忘掉”显式指令与每 N 轮自动写入；插件管理页可列表/删除/清空/关闭注入；注入失败只降级不炸链 |
| 1.3 | ContextCompactor 真接入 | 超 token 阈值触发 `turn_summary → topic_summary → long_term_fact` 分层压缩；**原始轮次永远落盘**；动作/情绪信号结构化保存不随摘要丢失；设置页/状态页显示上下文使用率 |
| 1.4 | body/mood director 输出进链 | `sentence.after` 产出 BodyFrame/MoodFrame/TtsFrame，接入现有 mood_engine 与 TTS instruction 分发（保留 P7-05 异常降级）；消除 `ExpressionDirector` 与 body-director 的双导演重叠 |
| 1.5 | 专家状态可见 | 每个专家 `ok/running/timeout/fallback/disabled/error` 落日志，并在插件管理页状态栏可见；禁用插件立即从钩子链移除 |
| 1.6 | 插件配置表单生效 | 插件管理页按 manifest `config_schema` 动态生成配置表单，修改即时生效或明确提示需重启；配置持久化到本机存储，不靠改代码 |
| 1.7 | PC interrupt 端到端 pytest | 补 `control: interrupt` 从前端发送到停声/清队列的端到端用例（真机 VAD 触发另登记真机轮次） |

**阶段 1 完成判据**：TurnConductor 真链 E2E 测试绿；memory/compactor 在对话链实际生效且可观测；专家 fallback 全部可见；插件配置表单可用；interrupt 用例绿。

---

## 5. 阶段 2：知识库 + 联网搜索实做（D1 已决：进 v1）

| # | 任务 | DoD |
| --- | --- | --- |
| 2.1 | KnowledgePlugin 运行时落地 | `plugins/native.py` 中 knowledge 由 `stub` 改为 `runtime`；新增 `KnowledgePlugin`：本地知识库目录 `cache/knowledge/`（或设置中心可配路径），支持 txt/md 文档导入、按空行/标题切块；`llm.before` 检索 top-k 并以「[知识库命中] 来源：文件名」注入；管理 UI 可列表/删除/清空/关闭注入；检索失败只降级不炸链 |
| 2.2 | 知识库检索质量底线 | 检索先用确定性关键词/重叠评分（零外部依赖），命中带来源；token 预算硬上限（不超 `context_compactor` 预算）；每轮 `knowledge_hits` 写入 `TurnBundle` 并在状态页可见 |
| 2.3 | WebSearchPlugin 运行时落地 | `plugins/native.py` 中 web-search 由 `stub` 改为 `runtime`；接入既有 MCP `ddg-search`（`shared/mcp_tools.json`）与 `ToolExecutor`；hook `tool` 收到查询时执行搜索，超时（建议 8s）与异常走确定性 fallback 并写 `expert_status`；结果裁剪（条数/字符上限）后注入 LLM 上下文，附来源 URL |
| 2.4 | 门控与可见性 | knowledge / web-search 未启用时对应能力从工具列表与注入链移除；插件管理页显示状态、最近一次耗时/错误/结果条数 |
| 2.5 | 测试 | 全部用 mock 工具执行器，单测覆盖：查询触发、结果裁剪、超时 fallback、禁用移除、异常不炸链；真实联网只做冒烟并单独登记（本机可能无外网） |

**阶段 2 完成判据**：knowledge 与 web-search 均从 stub 转为 runtime、在对话链真实生效；mock E2E 绿；结果带来源引用；禁用/超时/异常路径可见。

---

## 6. 阶段 3：资产与配置能力装配验证 + 插件契约（v1 标准 5，仅 PC）

| # | 任务 | DoD |
| --- | --- | --- |
| 3.1 | model-assets 闭环验证 | 模型 zip 上传 → 校验（zip 结构/许可文件/model3.json）→ 进 `live2d-models` → 可设为默认 → 删除，装配级 E2E 全通（本机 API 级 + 前端单测；浏览器视觉项登记真机轮次） |
| 3.2 | voice-assets 闭环 | 参考音频上传与克隆音色管理；与 CosyVoice 声音克隆打通（无真实 Key 时用契约 mock，真 Key 冒烟单独登记） |
| 3.3 | config-pack 闭环 | 配置导入/导出/重置 E2E：导入前校验、失败回滚、成功原子落盘，语义与设置中心一致 |
| 3.4 | 插件契约文档可执行 | 扩写 `docs/architecture/plugin-sdk.md`：manifest 字段、hook_points 枚举、entrypoint 协议、config_schema 动态表单规则、origin 判定、错误可见通道；附一个最小可跑外部示例插件 + 加载测试 |
| 3.5 | 外部插件加载验证 | 外部插件从 `plugins-external/`（或设置中心配置的目录）加载、校验、启用/禁用、卸载全链路测试；非法 manifest 拒绝加载且不拖垮宿主 |

**阶段 3 完成判据**：三条资产链路装配级 E2E 全通；外部插件契约文档 + 示例可跑；非法插件被安全拒绝。

---

## 7. 阶段 4：v1 发布收口（作者裁决）

| # | 任务 | DoD |
| --- | --- | --- |
| 4.1 | 全量绿门 | `python scripts/verify_all.py` 一键通过：根 pytest / PC 后端 pytest / 前端 vitest + tsc + build / 一致性 33/33 / 注册表 / 冒烟 + 装配级 E2E；数字写入 CHANGELOG |
| 4.2 | 文档定稿 | README（v1 特性与路线图，注明 Android 维持 v0.1.0、PC 提供模型/配置导入）、HANDOVER（v1 PC 架构事实与 SOP）、CHANGELOG、docs/README 索引一致；本计划标记「已执行」 |
| 4.3 | 许可核查 | AGPL-3.0-only + 商业授权双许可齐备；SPDX 头与 LICENSE/NOTICE/CREDITS 第三方清单核对无误 |
| 4.4 | 作者验收 | 作者逐条确认五项 v1 标准（PC 侧）→ `git tag v1.0.0` → push 云端 |

---

## 8. 明确不做（PC 计划边界）

- 不碰 Android 代码与构建（冻结在 v0.1.0）。
- 不做主播相关、具体平台接入、个人陪伴/关系养成、主动搭话、领域分化专家、多角色群聊。
- 不做插件市场/在线安装、VRM/PNGTuber/MMD、AI 唱歌、跨设备云同步。
- 不做全树 ruff 历史债清零（不阻塞 v1）。
- 不做本地 GPU 模型/本地 LLM 优化（不属于 v1 闭环节点）。

---

## 9. 风险登记

| 风险 | 缓解 |
| --- | --- |
| 无浏览器/麦克风/GPU → 视觉 UI、真实拾音、渲染热路径无法本机验证 | 只做有单测覆盖的改动；视觉/音频项集中真机轮次一次验完 |
| 测试负例直接写真实文件 | 阶段 0.2 根除 + sha256 守护断言 |
| `test_shared.py` 与根 `test_persona_shared.py` 契约口径漂移 | 阶段 0.1 统一；后续 persona 改动必须同时跑两套 pytest |
| 80+ 在途文件未提交，存在误提交 Key/缓存风险 | 阶段 0.6 敏感信息复查；0.7 分三 commit；提交前跑一键绿门 |
| knowledge / web-search 真实网络不可用 | 单测全 mock；真实联网冒烟独立登记，不作为本机卡点 |
| 外部插件目录可能被塞入恶意 manifest | manifest 校验白名单 + 禁 eval/任意命令；加载失败隔离并记录，不拖垮宿主 |
| conf.yaml 内联 persona fallback 与 `shared/persona.yaml` 漂移 | 阶段 4 文档定稿时删除冗余 fallback（run_server 已自动注入） |
| 外部服务（DeepSeek/DashScope/CosyVoice）真实联调不稳定 | 单测用 mock；真实调用冒烟独立记录、允许跳过并登记 |

---

## 10. 每轮收尾固定动作（强制）

1. `python scripts/verify_all.py`（阶段 0.3 后可用；此前手动执行 0.2 表内命令）。
2. `git status --short` 逐行确认无 Key、缓存、运行时状态、一次性日志。
3. 把本轮绿门数字与未验证项写入 CHANGELOG。
4. 分语义 commit，不把多轮工作混成一个提交。

---

## 11. 阶段完成判据汇总

- **阶段 0**：第五轮已提交；一键绿门全过；测试不再触碰真实仓库文件。
- **阶段 1**：TurnConductor 真链测试绿；memory/compactor 在对话链生效且可观测；专家 fallback 可见；插件配置表单可用；interrupt 用例绿。
- **阶段 2**：knowledge / web-search 均为 runtime、真实进链、mock E2E 绿、来源可引用、禁用/超时/异常可见。
- **阶段 3**：模型/声音/配置三条资产链路 E2E 全通；插件契约文档 + 示例可跑；非法插件被安全拒绝。
- **阶段 4**：作者点头 v1（PC 侧）→ `git tag v1.0.0` → push。
