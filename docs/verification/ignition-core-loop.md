# 点火自测清单：核心链路闭环（2026-09-10）

> 本轮范围：**Flutter 前端 + Rust 后端**，闭环「文本 → LLM（OpenAI 标准接口）→ TTS → 音频 → 口型 → Live2D 渲染」。
> **TTS 已上线**：CosyVoice 兼容服务在 `127.0.0.1:8080`（`[tts] response_format="pcm"`, `voice="skystar"`, 24 kHz）。
> LLM：DeepSeek `deepseek-flash`，密钥由 `DEEPSEEK_API_KEY`（见 `./.env`）注入 —— **必须用 `scripts/ignite.sh` 启动**，
> 它会导入 `.env`；直接 `nohup ./target/debug/live2d-ai-desktop` 会因缺 key 得到 401。
> 动作/表演本轮不做。

---

## 0. 前置

| 项 | 要求 |
|---|---|
| 本地 LLM | **任意 OpenAI 兼容实现**（ollama / llama.cpp / LM Studio / vLLM…），监听 `127.0.0.1`；缺省 `http://127.0.0.1:11434/v1` |
| 配置 | `live2d-ai.toml` 的 `[llm] base_url` / `model` 指向上面的服务 |
| 前端产物 | `shell/flutter/build/web/index.html`（`cd shell/flutter && flutter build web --release --base-href /app/`） |
| 渲染面产物 | `crates/l2d-wasm-demo/dist/`（`cd crates/l2d-wasm-demo && trunk build`） |
| 模型资产 | `assets/models/bai/runtime/bai.model3.json` 等（用户合法持有，不入库） |

---

## 1. 一键点火

```bash
./scripts/ignite.sh --build        # 首次：构建渲染面 + Flutter Web + Rust，然后启动
./scripts/ignite.sh                # 之后：只预检 + 启动
```

脚本会：读 `live2d-ai.toml` 探测 LLM 端点 → 确保产物存在 → 启动 `live2d-ai-desktop --web`，
并打印入口 URL：**http://127.0.0.1:18080/app/**

> 音频单源：web 模式默认「仅浏览器出声」（服务端不开 cpal）。
> 本轮 TTS 未配置，因此不会有音频；配置 `[tts]` 后浏览器会自动出声并驱动口型。

---

## 2. 自测清单

| # | 你要看到什么 | 判定 | 本轮 |
|---|---|---|---|
| 1 | `http://127.0.0.1:18080/app/` 打开后，**左侧舞台渲染出 Live2D 模型**（非黑屏/白屏/空白） | 模型可见 | ✅ 验收 |
| 2 | 在右侧输入框发消息 → 模型侧无关，**聊天区逐字流式显示 LLM 回复** | 文本流 | ✅ 验收 |
| 3 | 回复确实来自你的本地 LLM（断网也能答；日志 `/api/v1/logs` 可见请求） | 本地 LLM | ✅ 验收 |
| 4 | 刷新页面后模型仍能重新渲染（产物/路由稳定） | 可复现 | ✅ 验收 |
| 5 | 浏览器**出声**（服务端不出声：`LIVE2D_AI_WEB_AUDIO` 默认 browser） | 出声 | ✅ 验收 |
| 6 | 说话时**模型嘴型随语音开合**（真实 RMS，非固定动画） | 口型同步 | ✅ 验收 |

### 服务端已实测（2026-09-10，无需你操作）

```
POST /api/v1/chat {"text":"用一句话打个招呼"}  -> 200
WS 帧统计: {subscribe_ack:1, action_state:1, turn_state:2, text_delta:4, heartbeat:3, audio:34}
ASSISTANT_TEXT = "嗯。"
AUDIO frames=34  approx_bytes=32640  sample_rate=24000
```

即：DeepSeek 出文本 → CosyVoice 出 24 kHz PCM → WS 广播音频帧，服务端链路全通；
**「出声」与「口型」只剩浏览器侧呈现需要你目视确认。**

---

## 3. 失败排查

| 现象 | 原因 | 处理 |
|---|---|---|
| `/app/` 返回 503 `flutter_build_missing` | Flutter 产物缺失 | `cd shell/flutter && flutter build web --release --base-href /app/` |
| 页面白屏 / 资源 404 | 构建时漏了 `--base-href /app/` | 用上面的命令重新构建 |
| 模型区空白，状态栏报 GPU 错误 | 浏览器不支持 WebGPU/WebGL2 | 换 Chrome/Edge 最新版；或看 `/render` 单独页的状态栏文本 |
| 聊天报 503 | 配置缺失或 `[llm] base_url` 非法 | 检查 `live2d-ai.toml`；看服务端 stderr |
| LLM 报错 | 端点没起或不是 OpenAI 协议 | 手动验证：`curl <base_url>/models` 应返回 200 |
| 想用设备上的服务端出声 | — | `LIVE2D_AI_WEB_AUDIO=server ./scripts/ignite.sh`（这时浏览器不再出声） |
| 想确认 Mod 状态 | — | `curl http://127.0.0.1:18080/api/v1/mods` |

---

## 4. 本轮明确不做（后续）

- TTS 接入与口型驱动（等选型）。
- 6 基础动作 / 表演 / 编序。
- STT 语音输入、桌宠窗口、设置面板精修、多模型管理、纹理档位 UI。
- 桌面（Windows/Linux）与移动端。

---

## 5. 变更历史

- 2026-09-10：初稿（Flutter + Rust 闭环；TTS 选型待定，语音/口型不验收）。
