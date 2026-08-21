# Live2D-Ai 可观测性 / 健康自检骨架

> 目标：不依赖 API Key，也能在本地快速判断“配置、模型、双端一致性、核心依赖”是否健康。

## 1. 健康自检分层

| 层 | 检查内容 | 无 Key 可跑 |
| --- | --- | --- |
| 配置层 | shared 文件存在、可解析、双端副本一致 | ✅ |
| 模型层 | model_registry 合法、Live2D 资产存在 | ✅ |
| 协议层 | emotion-protocol / adapter / PC model_dict 三方一致 | ✅ |
| 核心逻辑层 | Android 状态机、TTS 降级链、PC TTS 队列 | ✅（单测） |
| 服务层 | LLM / TTS 云端连通性 | ❌ 需要 Key / 网络 |

## 2. 本地健康检查脚本

入口：`scripts/health_check.py`

当前检查项：

1. `shared/` 关键文件存在
2. `shared/persona.yaml` 可被 YAML 解析
3. `shared/model_registry.json` 通过 schema 校验（若 jsonschema 可用）
4. Android assets 中的 `persona.yaml`、`mcp_tools.json` 与 shared 字节一致
5. PC `conf.yaml` 存在且引用 `shared/persona.yaml`
6. PC Live2D 模型目录存在（`live2d-models/bai/runtime/bai.model3.json`）
7. 不读取任何 API Key，不发起网络请求

## 3. 日志规范（已落地：logsetup）

入口 `run_server.py` 调用 `open_llm_vtuber.logsetup.setup_logging()`，双 sink：

| sink | 形态 | 用途 |
| --- | --- | --- |
| 控制台（stderr） | `HH:mm:ss | 级别 | 标签 │ 消息`（彩色紧凑） | 运行态一眼可读 |
| 文件 `logs/server.log` | 毫秒时间戳 + `模块:函数:行号`，10MB 轮转保留 5 份 | 排障回放全貌 |

- 级别由 `LIVE2DAI_LOG_LEVEL` 控制（默认 INFO；设置中心可改）；非法值回落 INFO 并提示。
- uvicorn 访问日志逐请求刷屏：非 DEBUG 时压到 WARNING。
- 各模块用 `get_logger("标签")` 取 logger：`tts` / `audio` / `flow` / …，未绑定落 `app`。
- 逐 chunk 的耗时明细（封装/口型等）为 DEBUG 级单行格式；INFO 层只留关键节点。
- `logs/` 与 `*.log` 均已 gitignore；根目录遗留的 `server.log` 为历史手工重定向产物，可删。

## 4. 关键路径日志建议（部分落地）

- LLM：首 token 延迟、请求耗时、失败原因、降级 provider
- TTS：首音延迟、合成耗时、降级原因、播放中断次数
- Live2D：后端（LEGACY/ENGINE）、帧率、模型加载状态
- 状态机：异常 fail-safe 次数、非法事件次数

## 4. 验收

- `python scripts/health_check.py` 无 Key、无网络可运行，退出码 0 表示本地基础健康。
- 健康检查不修改任何文件。
