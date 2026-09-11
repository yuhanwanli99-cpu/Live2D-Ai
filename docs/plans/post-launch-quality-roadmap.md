# 发布后工程质量路线图

> 创建时间：2026-08-09  
> 范围：PC 端（`Live2D-Ai-pc/open-llm-vtuber`）

## 目标

完成 uv 可复现构建、`routes.py` / `settings-ui.ts` 服务化拆分、lint 与跨平台一致性自动化。

---

## Phase 0：uv sync 与 CI 可复现（发布前必须完成）

- [ ] 0.1 重新 `uv sync --locked` 并验证通过
- [ ] 0.2 `uv run pytest tests/ -q` → 437+ passed
- [ ] 0.3 `uv run ruff check .` 并整理剩余告警
- [ ] 0.4 更新 CI cache（`setup-uv`）
- [ ] 0.5 文档化 uv 工作流

---

## Phase 1：`routes.py` 服务化拆分

- [ ] 1.1 创建目录结构
  - `src/open_llm_vtuber/services/`
  - `src/open_llm_vtuber/validators/`
  - `src/open_llm_vtuber/api_schemas/`
- [ ] 1.2 提取 `utils/atomic_io.py`（原子写入）
- [ ] 1.3 提取 `services/config_service.py`（conf.yaml、.env、persona 读写）
- [ ] 1.4 提取 `services/llm_service.py`（discover-models、test-llm、信任 URL 校验）
- [ ] 1.5 提取 `services/tts_service.py`（TTS 配置、试听、provider 归一）
- [ ] 1.6 添加 `api_schemas/*.py` Pydantic 请求/响应模型
- [ ] 1.7 重写 `routes.py` 为薄 handler 层
- [ ] 1.8 拆分并补充 `tests/test_routes_security.py` 与 `tests/services/` 单测
- [ ] 1.9 验证 `pytest` 与 `ruff` 全绿

### 验收标准
- `routes.py` 行数降至当前 40% 以下（当前约 2066 行）
- 每个 endpoint handler ≤ 30 行
- `ruff check src/open_llm_vtuber/routes.py` 全绿
- pytest 总数 > 450

---

## Phase 2：`settings-ui.ts` 模块化

- [ ] 2.1 创建 `renderer/src/settings/` 目录
- [ ] 2.2 提取 `settings/api-client.ts`（统一 fetch、错误处理）
- [ ] 2.3 提取 `settings/validators.ts`（log_level、URL、数字范围）
- [ ] 2.4 提取 `settings/state-store.ts`（配置草稿与 dirty 标记）
- [ ] 2.5 提取 `settings/form-bindings.ts`（DOM 双向绑定）
- [ ] 2.6 提取 `settings/event-router.ts`（按钮/tab/快捷键）
- [ ] 2.7 重构 `settings-ui.ts` 为入口 orchestrator
- [ ] 2.8 新增 `renderer/tests/settings/` 单测
- [ ] 2.9 验证 `vitest` 与 `tsc --noEmit`

### 验收标准
- `settings-ui.ts` 不再包含裸 fetch 与裸 DOM 查询
- vitest 总数 > 430
- `tsc --noEmit` 通过

---

## Phase 3：lint 与一致性自动化

- [ ] 3.1 修复剩余 `BLE001` 等历史告警（`routes.py` 拆分后、`single_conversation.py`、`director.py`）
- [ ] 3.2 在 CI 中关闭 ruff 的 `continue-on-error`
- [ ] 3.3 在 renderer CI 中加入 eslint/biome lint 步骤
- [ ] 3.4 在 CI 中加入 `shared/persona.yaml` 与 Android assets 一致性校验
- [ ] 3.5 可选：pre-commit hook 自动同步 persona.yaml

---

## Phase 4：文档与发布 sign-off

- [ ] 4.1 更新 uv 工作流文档
- [ ] 4.2 在 `docs/architecture/core-contracts.md` 补充 `model` vs `normalized` 参数空间约定
- [ ] 4.3 提交并打 tag（如 `v0.2.0-rc1`）
- [ ] 4.4 最终全量验证
  - PC Python tests
  - renderer Vitest + tsc
  - health_check.py
  - cross-platform consistency
  - 手动 smoke test settings UI
