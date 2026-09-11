# CosyVoice 3 TTS 接入（2026-09-10）

> 状态：**接入能力已就绪；本机不部署**（用户裁决）。
> 选定模型：[FunAudioLLM / Fun-CosyVoice3-0.5B-2512](https://github.com/FunAudioLLM/CosyVoice)。
> 本文只描述「怎么接进来」，不含部署步骤的执行。

---

## 1. 两条接入路径

| | 路径 A：OpenAI 兼容层（**推荐**） | 路径 B：官方 FastAPI |
|---|---|---|
| 实现 | 社区 CosyVoice3-API（Docker） | 官方仓库自带 runtime/python/fastapi/server.py |
| 端点 | POST /v1/audio/speech（JSON） | POST /inference_zero_shot 等（**multipart 表单**） |
| 协议 | **OpenAI 标准** —— 与我们的 LLM 同一套心智 | 私有协议，非 OpenAI |
| 返回 | 实测**裸 PCM（s16le）**；`stream=true` 无流式效果（整段一次返回） | 裸 PCM（s16le）流 |
| 缺省端口 | 8080 | 50000 |
| 本项目现状 | ✅ **已支持**（`response_format = "pcm"`） | ⚠️ 未内置 provider（见 §5） |

结论：**走路径 A**，且 `response_format` 必须填 **`"pcm"`**。

> ⚠️ **2026-09-10 实测更正**：本文早期版本写「非流式 = WAV → 填 `wav`」，与实际部署不符。
> 直连本机 CosyVoice3-API 实测：
> - `response_format` = `pcm` → **200**；`wav` / `mp3` / `opus` / `flac` → **一律 400**；
> - `stream: true` **无流式效果**：响应头 3 ms 到达，但整个 body 在整句合成完成后一次性返回；
> - 真实端点不是 `POST /v1/voices/register`，而是 `GET /v1/audio/voices`
>   （实测返回 `{"voices":[{"id":"skystar","mode":"zero_shot"}, ...]}`）。
>
> 因此**照抄旧版本配置会在第一次合成就 400 失败**。接入前请先用
> `curl -s http://<host>:8080/openapi.json` 探明真实路径与 `response_format` 支持面。
> 代码侧仍保留 WAV 容器解析（`crates/live2d-ai-runtime/src/audio/wav.rs`），
> 供返回 WAV 的其它服务端使用——但那条路径必须**整段缓冲**才能解析 RIFF。

---

## 2. 路径 A：接线步骤（你部署时照做）

### 2.1 起服务

```bash
docker run -d --gpus all --shm-size=2g \
  -p 8080:8080 \
  -v /你的目录/cosyvoice3-models:/root/.cache/models \
  -e HF_ENDPOINT=https://hf-mirror.com \
  --name cosyvoice3-api ghcr.io/hsiang-han/cosyvoice3-api:latest
```

> 首次启动会拉取模型（约 10GB）。硬件要求：NVIDIA GPU（FP16 约 4GB 显存起）。

参考：CosyVoice3-API 仓库 https://github.com/hsiang-han/CosyVoice3-API

### 2.2 注册一个克隆音色（**必须**）

CosyVoice3 **没有内置音色**，`voice` 字段填的是「注册过的克隆音色 id」。
先在部署上确认真实端点与已注册音色（2026-09-10 实测本机部署）：

```bash
curl -s http://127.0.0.1:8080/openapi.json      # 真实路径清单
curl -s http://127.0.0.1:8080/v1/audio/voices   # 已注册音色
# → {"voices":[{"id":"skystar","mode":"zero_shot"},{"id":"skystar_plain","mode":"zero_shot"}]}
```

注册接口随部署而异，以 `openapi.json` 为准（早期文档写的
`POST /v1/voices/register` 在本机部署上不存在）。音色模式为 `zero_shot`，
即每次合成都跑完整参考音频编码——这是 RTF > 1 的结构性原因之一。

### 2.3 打开配置

`live2d-ai.toml` 里取消注释（模板见 `live2d-ai.toml.example`）：

```toml
[tts]
base_url = "http://127.0.0.1:8080/v1"
voice = "my_voice"        # 2.2 列出的 voice_id
response_format = "pcm"   # 实测本机 CosyVoice3-API 只认 pcm（wav 会 400）
sample_rate = 24000       # CosyVoice3 输出 24 kHz
channels = 1
```

改完无需重启：服务端有配置热重载监听（`file_watcher`）。

### 2.4 验证

```bash
# 1) 端点就绪
curl -s http://127.0.0.1:8080/v1/models | head -c 200

# 2) 合成一段（pcm 时落盘的是裸 s16le，不是能直接打开的 WAV）
#    用 python3 校验：字节数 / 2 / 24000 = 音频秒数
printf %s '{"input":"你好，我是 CosyVoice 3","voice":"my_voice","response_format":"pcm"}' > cv3.json
curl -s -X POST http://127.0.0.1:8080/v1/audio/speech \
  -H "Content-Type: application/json" -d @cv3.json --output cv3.pcm \
  -w 'total=%{time_total}s bytes=%{size_download}\n'
ffplay -f s16le -ar 24000 -ch_layout mono cv3.pcm   # 直接试听裸 PCM

# 3) 我们侧：状态应显示 tts.configured=true
curl -s http://127.0.0.1:18080/api/v1/app/status | head -c 400
```

> **首字延迟基线（2026-09-10 实测，供回归对照）**：19 字中文 → 音频 4.72 s，
> 总耗时 5.71 s（RTF ≈ 1.2×）；抖动时 RTF 可到 4–6×。
> 注意 `curl -w '%{time_starttransfer}'` 报的是**响应头**时间（3 ms 级），
> **不能**当作 TTFA；要测首音频字节必须用原始 socket。
> 项目验收线见 `docs/verification/acceptance-metrics-report.md`（TTFA ≤ 1 s）。

---

## 3. 我们为此准备了什么（本轮已实现）

| 能力 | 位置 | 说明 |
|---|---|---|
| `[tts] response_format` 配置项 | runtime/src/settings.rs | 取值 pcm（默认）或 wav；非法值在 resolve 阶段报错 |
| **WAV（RIFF/WAVE）解析** | runtime/src/audio/wav.rs（新增） | 仅接受 PCM 16-bit（含 WAVE_FORMAT_EXTENSIBLE 子格式）；跳过 LIST 等未知块与填充位；规格以容器头为准 |
| WAV 句合成路径 | runtime/src/conversation/worker.rs | 整段收集 → 解析 → 按 chunk_samples 切块发事件；64 MiB 上限防内存打爆 |
| 格式门禁 | runtime/src/audio/mod.rs | 由「只认 pcm」放宽为「pcm + wav」；mp3/flac 仍明确拒绝 |
| ~~`local-tts` Mod 默认对准 CosyVoice~~ | ~~crates/live2d-ai-mod-local-tts~~ | **2026-09-11 已删除**：TTS 从 Mod 系统移出，改成核心链路。端点写 `[tts] base_url`。见 `docs/architecture/tts-is-core.md` |
| 缺省 Mod manifest | web_api/cli_entry.rs | 无 mods.json 时内建启用 local-llm(11434)；**TTS 不在 Mod 里** |
| TTS 可缺失 | runtime/src/conversation/worker.rs | 未配置 [tts] 时走「空句」路径：只出文本，不报错 |

### 口型链路（无需改动）

Flutter / 浏览器侧已经在消费 WS audio 帧并对同一份 PCM 计算真实 RMS
（50ms 窗 / attack 0.03 / release 0.10），下发 mouth.level 驱动 ParamMouthOpenY。
**CosyVoice 接通后口型自动生效**，前端零改动。

---

## 4. 已知取舍

- **WAV 路径不是流式的**：需收完整句再解码，首音延迟 = 单句合成时间。
  想换流式首音，可用 CosyVoice3-API 的 stream=true（返回裸 PCM）并把 response_format 改成 pcm ——
  但该字段是**非标准 OpenAI 扩展**，我们暂未接进请求体（避免污染标准端点）。
- 因此 response_format = "pcm" 时，请确认服务端确实返回裸 s16le，否则会解码成噪声。

---

## 5. 未做（标记待做）

1. **路径 B 的官方 /inference_* provider**：multipart 表单 + prompt_wav 文件上传，
   与本 crate「纯 OpenAI 协议、不做文件 IO」的边界冲突，需要单独的 provider 设计（含参考音频管理）。
2. **CosyVoice3-API 的 stream=true**（非标准扩展）：要接需先决定如何隔离「标准字段 vs 扩展字段」。
3. **音色注册的 UI / 脚本封装**：目前需要你手动 curl 注册。

---

## 6. 变更历史

- 2026-09-10：初稿（CosyVoice 3 选定；路径 A 已可接入，路径 B 标记待做；WAV 解码落地）。
