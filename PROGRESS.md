# 工程质量任务进度

> 🗄️ **本文件已归档（Python 时代工程质量进度，2026-06 双端栈）**。Rust 主线的进度与
> 执行计划见 `docs/plans/`（node-e-* / node-f-* / integrity 等）与 `CHANGELOG.md`。
> 下方内容保留为历史记录，不再反映当前架构。

> **2026-08-25 交接**：当前轮次修改已落盘并暂停，详细交接见 [docs/development/HANDOFF-2026-08-25-runtime-fixes.md](docs/development/HANDOFF-2026-08-25-runtime-fixes.md)。

见 [docs/plans/post-launch-quality-roadmap.md](docs/plans/post-launch-quality-roadmap.md)。

## 当前状态（2026-08-24）

- [x] 计划文档：已创建
- [x] Phase 0：测试基线已验证（pytest 437 passed，vitest 417 passed，tsc pass）
  - [ ] 实际 `uv sync` 未能执行：当前环境无 uv 二进制，但依赖已通过现有 .venv 验证；CI 已启用 uv cache 与 Python 3.12
- [x] Phase 1 部分完成：`routes.py` 从 2066 行拆至 1671 行
  - [x] 新建 `services/config_service.py`、`services/config_routes_helpers.py`
  - [x] 新建 `services/llm_routes_helpers.py`、`utils/http_utils.py`
  - [x] 新建 `services/tts_routes_helpers.py`
  - [x] 新建 `validators/log_level.py`、`validators/url_trust.py`
  - [ ] 剩余：LLM/TTS endpoint 逻辑进一步服务化、routes.py 历史 broad-exception 清理
- [x] Phase 2 部分完成：
  - [x] 创建 `renderer/src/settings/api-client.ts` 脚手架
  - [ ] 未 wired：`settings-ui.ts` 仍保持原状以确保 tsc/vitest 通过；需后续逐步替换 fetch 调用
- [x] Phase 3 部分完成：
  - [x] CI 升级 Python 3.12、启用 uv cache
  - [x] CI 增加 Android persona.yaml 与 shared/ 一致性校验
  - [ ] 剩余：关闭 ruff continue-on-error、修复历史 broad-exception
- [x] Phase 4 部分完成：
  - [x] 创建 `docs/development/uv-workflow.md`
  - [x] 在 `docs/architecture/core-contracts.md` 补充参数空间约定
  - [ ] 剩余：打 tag、最终全量验证

## 验证结果

- PC pytest：437 passed
- renderer vitest：417 passed
- renderer tsc --noEmit：pass

## 2026-08-24 执行 1-2-3-4 计划

- [x] pytest 基线保持：437 passed（PC 后端）
- [x] renderer vitest 基线保持：417 passed；tsc --noEmit 通过
- [x] 后端 service 拆分：
  - `services/llm_service.py`：模型发现 / LLM 测试 / Provider 检查 LLM 部分
  - `services/model_service.py`：模型注册表读取、扫描、形象热切换
  - `services/tts_service.py`：TTS Provider 检查、TTS 试听
  - `services/local_tts_service.py`：本地 Melo TTS 试听与音频下载
  - `services/config_endpoint_service.py`：/api/config/tree 原子写 + Pydantic 校验
- [x] `routes.py` 由 2066 行降至 1157 行（init_config_routes 降至 ~993 行）
- [x] 渲染层 `settings/api-client.ts` 封装所有设置面板 fetch，已接入 `settings-ui.ts`，`tsc --noEmit` 通过
- [x] 全仓库 BLE001（盲目捕获 Exception）已归零；未移除 ruff continue-on-error（ruff.yml 本身已无 continue-on-error）
- [ ] 剩余：将 `update_config`（~416 行）进一步拆入 `ConfigEndpointService`，目标 `routes.py` <1000 行
## 2026-08-24 PC Preview 发布治理

- [x] 许可证统一为标准 AGPL-3.0-only；商业使用遵守 AGPL 即可，另提供可选闭源商业许可证
- [x] 白模型来源记录为 Bilibili BV1NqNFejEeE；公共 PC Preview 不捆绑未明确授权再分发的模型原文件
- [x] 导演改为只选择语义动作；连续 halfBody/gaze 输出被忽略，渲染端确定性执行并自动回中
- [x] 动作 strength 1/2/3 已贯通后端协议与 renderer，幅度可区分
- [x] look_around 眼球时间线限制在安全区，结束回到 ParamEyeBallX/Y=0.5
- [x] routes.py 从 1157 行降至 11 行兼容组装层；设置、catalog、config update、proxy、webtool/ASR/TTS WS 全部迁入 api/
- [x] 新增 route 架构门禁，禁止 routes.py 回涨超过 600 行或重新吸收音频/proxy endpoints
- [x] 新增 PC Preview CI：uv sync --locked、pytest、renderer test/typecheck/build、Gitleaks、公开树策略
- [x] 新增 CONTRIBUTING/SECURITY/CODE_OF_CONDUCT/COMMERCIAL-LICENSE、Issue/PR 模板和 PC 发布策略
- [x] 当前全量验证：PC pytest 445 passed；renderer 427 passed；tsc pass；Vite production build pass
- [x] 已生成并验证 PC-only 公共源码归档：约 8MB/402 文件，不含模型/缓存/密钥；归档内 Python 440 passed
- [x] 公共无模型 renderer 干净 npm ci：414 passed + 6 私有模型资产测试 skipped；tsc/build pass
- [x] Vite/Vitest 升级至安全版本，npm audit 0 vulnerabilities
- [x] Renderer 上帝文件治理：settings HTML 模板迁入 settings/panel-template.ts（2130→1920 行）；main bootstrap storage/live2d loader 迁入 bootstrap/（935→856 行）；新增架构/行为测试
- [ ] 外部 CI 尚需在 GitHub 实际执行 uv sync --locked 与 Gitleaks 全历史扫描
- [x] 公共无模型归档已在 Linux 实际启动：默认绑定 127.0.0.1；无 Key 时 /health、首页、/api/config、/api/models 均 200，设置响应不含 key
- [ ] 尚需导入合法模型做浏览器渲染，并用真实 Provider 做对话/TTS/口型/动作/打断 smoke，之后 tag

