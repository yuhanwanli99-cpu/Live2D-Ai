# PC Web 设置中心重构规格（2026-08-20）

> 本文件是后端逻辑链 + 前端 UI 并行重构的共享契约。后端子代理与 UI 子代理**必须**以此为准，
> 冲突时以本规格为准；完成后更新本文件状态。
> 前置权威审计：`Live2D-Ai-pc/open-llm-vtuber/SETTINGS_SYSTEM_AUDIT.md` + `docs/plans/settings-fix-plan.md`。
> 范围：仅 PC 端（open-llm-vtuber Web 设置中心）。Android 不在本期。

---

## 0. 环境事实（本机 WSL2）

- 仓库根：`/home/administrator/deepseekharness/Live2D-Ai`
- venv：仓库内 `.venv`（`/home/administrator/deepseekharness/Live2D-Ai/.venv`，Python 3.12）。
  - ⚠️ `Live2D-Ai-pc/start.sh` 默认 `$HOME/Live2D-Ai/.venv` —— 本机不存在 → **start.sh 断链，必须修**。
- 后端入口：`Live2D-Ai-pc/open-llm-vtuber/run_server.py` → `uvicorn 0.0.0.0:12393`（端口被写死）。
- 前端：`renderer/src/*.ts`（Vite+TS）→ `npm run build` 输出到 `frontend/`（后端静态托管）。
  - 重建命令：`cd Live2D-Ai-pc/open-llm-vtuber/renderer && npm ci && npm run build`（npm registry 可达）。
- 服务当前运行：PID 335（`.venv/bin/python run_server.py`）。后端改动需重启才生效；前端重建后刷新即见。

---

## 1. 后端 API 契约（重构后）

### 1.1 `GET /health`（新增）
- 返回 `200 {"status":"ok","version":"v1.2.1","uptime_s":<int>}`。WSL2 迁移验收项。

### 1.2 `GET /api/config`
- 返回 normalize 后的完整视图（与 `POST /api/config` 成功响应同构）。字段沿用现有形状（`llm_provider`/`llm_configs[]`/`tts_configs[]`/`asr_model`/`character_name`/`port`/`live2d_fps`/`log_level`/`persona{...}`/`is_running` 可选）。
- 每个 provider 附 `has_key`、`configured`。（现有即如此，保持）

### 1.3 `POST /api/config`
- 请求：现有 `collectPayload()` 产出（`llm_provider`、`llm_configs[]`、`tts_configs[]`、`asr_model`、`character_name`、`port`、`live2d_fps`、`log_level`、`persona{...}`、`api_keys{...}` 可清空项用 `null`）。
- 处理顺序（关键修复，全部落盘前完成）：
  1. **normalize**：合并 UI 未传字段与已保存配置；TTS 选中 provider 若缺必填（voice/model）自动补目录默认值，补不出 → 400 并指明字段。
  2. **校验**：`AppConfig(**candidate)` 完整校验；设定树/结构非法 → 400，**不落盘**。
  3. **写盘**：conf.yaml（原子写 tmp+rename）、.env、shared/persona.yaml。
  4. **热重载/apply**：`apply=true` 时先 `load_from_config` 到共享 cache，再对已连 `client_contexts` 逐会话重跑；`self.system_config/character_config` 赋值移到 `init_agent` 之前。若即时热更无法覆盖已连会话 → 返回 `{"applied":false,"requires_restart":true}`。
  5. **响应**：`{"ok":true,"config":<normalize后完整视图>,"applied":bool,"requires_restart":bool,"message":str}` —— 前端据此 re-populate。
- 失败/校验失败：`{"ok":false,"error":<人类可读>,"field_errors":{...}}`，YAML 未被改写。

### 1.4 `POST /api/config/tree`
- 写盘前执行 `AppConfig` schema 校验；非法 → 400 不落盘（修复 P0-3/B10）。
- 成功返回 normalize 后 config 同构。

### 1.5 显式清空语义
- UI 传 `null`（或空串 + `clear:true`）→ 后端删除该字段/key；区分“未传”（保留旧值）与“显式清空”（删除）。persona 字段、`.env` key 均支持清除。

### 1.6 `POST /api/llm/discover-models` / `POST /api/test-llm` / `POST /api/test-tts` / `POST /api/providers/check`
- 请求允许携带当前 UI 值（provider、base_url、model、api_key、voice 等）；后端以“已保存 ∪ UI”合成视为准（修复 P1-3）。
- 按 provider 协议感知：OpenAI 形态（`/chat/completions`+`/models`）、Anthropic（`/v1/messages`）、Ollama（`/api/chat`）等（修复 B9）。无法覆盖的不做强求，但不得对全部一律 OpenAI。
- 失败返回明确错误（提取上游 message），不吞成 400 报错也让前端能展示。

### 1.7 `GET /api/config` 的 `log_level`
- 从 `.env` 读 `LIVE2DAI_LOG_LEVEL`，round-trip 提供；仅当用户改动才随保存提交（修复 A1）。

### 1.8 端口生效
- `run_server.py` 用 `config.system_config.port`（不再写死 12393）；`server.py /proxy-ws` 拼端口同源。移除/收敛 `_start_server.py` 分叉启动器（与 run_server bootstrap 对齐或删除，凡保留路径必须做 `${VAR}` 替换 + 人设注入）。

### 1.9 `.env` key 对运行进程可见
- apply/reload 前 `load_dotenv(override=True)` 重读，或让各 factory 的 key 解析支持运行时重读（修复 B4）。

### 1.10 引擎生命周期 / WS 断线
- engine/agent 支持 `close()`，reload 替换前 close 旧实例；`handle_disconnect` 先查后 pop（修复 B7/B8）；`init_vision` 支持热换。
- proxy 挂起：播放确认超时路径补发 `send_conversation_end_signal`（修复 B12）。

---

## 2. 前端 UI 规格（NEKO 内容结构 + Codex/DSH 深色精练风）

### 2.1 布局（对标 NEKO 的 Tab 分类 → 左侧导航）
```
┌──────────────┬──────────────────────────────────────┐
│ 左导航(固定)   │  内容区（一个 section 一次可见）        │
│ ────────────  │  ┌─ section header ──────────────┐  │
│ ● 对话/LLM    │  │ 标题 + 行内状态/操作            │  │
│ ● 语音/TTS    │  ├───────────────────────────────┤  │
│ ● 形象/Live2D │  │ Provider 卡片组（折叠行→展开编辑）│  │
│ ● 密钥        │  │ …                              │  │
│ ● 人设/提示词  │  └───────────────────────────────┘  │
│ ● 系统/高级    │                                      │
│ ────────────  │                                      │
│  状态简报       │                                      │
└──────────────┴──────────────────────────────────────┘
┌──────────────────────────────────────────────────────┐
│ 底部条：状态消息(左)    [保存全部]  [保存并应用] 关闭     │
└──────────────────────────────────────────────────────┘
```
- 保持对外接口 `initSettingsPanel(container, opts): SettingsPanel {open, close, destroy}` 与 `main.ts` 调用不变。
- 覆盖层 + 左侧导航 rail + 右侧内容区；模态式（overlay），深色。
- 6 个分区与 NEKO 对齐：对话/LLM、语音/TTS、形象/Live2D、密钥、人设/提示词、系统/高级。

### 2.2 视觉（DSH / Codex 深色精练风）
- 配色（DeepSeek neutral-bluish 暗色）：
  - 背景分层：`#0F0F0F`（最底）、`#151517`、`#1B1B1C`、`#212123`、`#2C2C2E`（hover/卡片）
  - 文本：主 `#FFFFFF`、次 `#D4D4D4`、弱 `#ADB2B8`、失效 `#979BA0`
  - 主色（品牌蓝）：`#4176E6`、hover `#5686FE`、浅蓝文字 `#93A8FE`/`#67A0FE`
  - 边框：`rgba(255,255,255,.08/.12/.16)` 分级；分隔用 `#26282C`
  - 成功 `#22C55E`/`#4ED17E`、危险 `#EC1313`/`#EF4444`、警告 `#F59E0B`
  - 字体：`-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", Helvetica, Arial, sans-serif`；代码/ID 用 mono `SF Mono, "JetBrains Mono", "Fira Code", Consolas, "Liberation Mono", Menlo, Courier`
  - 圆角 6–10px；紧凑间距；低饱和、克制的高亮；hover 过渡 120ms。
- 组件语言：紧凑行（provider 收起态）+ 点击展开编辑；badge（已配置/未配置 Key/测试中）；状态点；mono 展示 provider id / key 尾段；按钮分级（主=蓝实心、次=ghost 描边、危险=红）。
- 复用/延续：现有 `.settings-*` 类可改造，但**必须**用 grep 确认无其它模块（chat-ui/main）引用后再改名。

### 2.3 交互与行为（对齐设置逻辑链修复）
- **打开面板不自动 discover**：移除 `autoDiscoverConfiguredModels` 无条件调用；「获取可用模型」改手动（每 provider 卡内按钮）+ 结果缓存（修复 P2-1）。
- **保存后 re-populate**：`save()` 成功用响应 `config` 重新填充表单（UI=磁盘=运行时一致，修复 P1-1）。
- **busy/disable 防护**：加载完成前禁用 保存/保存并应用；所有异步按钮（discover/test/check/scan/save）加 busy+disable；双击只发一次 POST（修复 A2/A6）；共享状态用请求 token/AbortController，最后一次点击胜出。
- **下拉占位**：`fillSelect` 加“未选择”占位，空 `llm_provider` 不允许自动选第一个（修复 A7）；POST 前 `checkValidity()` 挡 `port:0`。
- **显式清空**：人设字段/API Key 提供「清除」按钮（置 null 发送）。
- **TTS 检查语义**：要么真实合成一次，要么标注“仅检查 Key 是否已配置”（修复 P2-2，诚实提示）。
- **测试用 saved∪UI 合成视图**：testLlm/testTts 把当前 UI 未保存值一并带上（修复 P1-3）。
- **`Audio.play()` 拒绝处理** + 预览 URL 统一后端相对路径约定（修复 A11）。
- **log_level round-trip**：renderSystem 从 `/api/config` 填下拉，仅用户改动才提交（修复 A1）。
- **「保存并应用」文案诚实**：响应 `requires_restart:true` 时展示“已保存，重启/重连后生效”，不吹嘘 applied。
- **错误呈现**：后端 400 `field_errors` → 字段级红框 + 顶部错误条；网络错误 → 状态条。

### 2.4 代码结构（可测性）
- 拆出纯逻辑模块 `renderer/src/settings-logic.ts`：`collectPayload(uiState)`、`normalizeUi(uiState)`、`validatePort`、`buildModelOptions` 等纯函数，供 vitest 直接测试。
- `settings-ui.ts` 只做 DOM 装配与事件接线（结构瘦身；保留 initSettingsPanel 导出契约）。
- 新增 `renderer/tests/settings-logic.test.ts` 覆盖：payload 构建（可清空字段 null）、port 校验、占位下拉、busy 语义（若抽出）。
- 现有的 `renderer/tests/*` 必须全部继续通过。

---

## 3. 测试要求

- 后端：`tests/conftest.py` 提供 FastAPI `TestClient`/ASGITransport + 临时 conf/.env/persona fixture；`tests/test_settings_api.py` 覆盖：
  - GET/POST `/api/config` round-trip(含 normalize 后 config 一致性)
  - 校验失败不落盘（TTS 空配置、tree 非法结构 → 400 且文件未变）
  - 显式清空（null）删除逻辑
  - `apply=true` 已连会话行为（mock）或 `requires_restart` 诚实返回
  - `log_level` round-trip、`/health` 200
  - check/test 用 saved∪UI（respx mock 外呼）
- 前端：上文 2.4。全部跑 `pytest` + `vitest run` 通过。
- 回归：现有测试（`scripts/`、`tests/`、`renderer/tests/`）不得因重构而失败；若契约形状变化导致现有断言失败，须同步更新并说明。

---

## 4. 交付验收（Definition of Done）

- [ ] `start.sh` 在本机可直接启动（默认找到仓库内 `.venv`）
- [ ] `curl localhost:12393/health` = 200
- [ ] 设置里改 `port` 后按该端口监听（重启后端验证）
- [ ] `POST /api/config` 校验失败回滚，YAML 未被写坏
- [ ] 保存成功 → 前端 re-populate，UI=磁盘=运行时
- [ ] `.env` 新 key 对运行引擎可见（重启后）；log_level round-trip
- [ ] 打开设置面板不自动 discover、不卡顿；discover/check/test 可取消、busy 正确
- [ ] 深色 DSH 风格 UI 渲染正常，6 分区导航可用
- [ ] pytest + vitest 全绿；README/AGENT/CHANGELOG 更新
- [ ] 浏览器刷新 `localhost:12393` 后确认新 UI + 模型渲染 + WS 会话正常
