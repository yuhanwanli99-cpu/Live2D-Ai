# Live2D-Ai — PC 端本地 TTS（Melo）完成计划

> 版本：v1（仅 PC）
> 定稿日期：2026-08-21
> 执行状态：阶段 1–5 代码与单测/绿门已完成（2026-08-21）；Melo 已改为对接外部 conda 环境（`melo_worker.py` + `melo_external.py`），ZH 真机端到端合成 PASS；剩余 EN/ES/FR/JP/KR 首次下载合成与 cuda 并发冒烟
> 范围：仅 Live2D-Ai-pc/ 及其读取的 shared/ 数据。Android 冻结在 v0.1.0，不进入本计划。
> 本计划取代 docs/plans/PLAN-V1.md，是当前 PC 端唯一执行口径；计划完成后由项目作者裁决是否打 v1.0.0。
> 新增硬需求：提供本地 TTS 接口（Melo TTS，无需云端 Key）。

---

## 0. 一句话目标

在现有 PC 端 Open-LLM-VTuber 基础上，把 Melo TTS 接成一等公民的本地 TTS Provider，并补一个稳定、可被本机脚本调用的 HTTP 本地 TTS 接口；聊天链路、设置中心试听、/tts-ws 与新增 REST 接口都能用同一个引擎，且不依赖任何云端 TTS Key。

---

## 1. 需求解读与口径

1. 仅 PC：只改 Live2D-Ai-pc/ 与必要文档，不碰 Live2D-Ai-Android/。
2. 本地 TTS：Melo 是本地多语种音色模型（ZH/EN/ES/FR/JP/KR），首次使用从 hf-mirror 拉模型；合成不依赖网络和 API Key。
3. 不是参考音频克隆：用户提供的 API 是 Melo 的多语种合成，不是“给一段参考音频克隆音色”。本计划把 Melo 定位为本地多语种 TTS Provider；真正的参考音频克隆继续走既有 voice_library.py + CosyVoice/GPT-SoVITS 通道，不混进 Melo。
4. 四个韵律参数必须暴露：speed、sdp_ratio、noise_scale、noise_scale_w 直接影响韵律，不影响音色；language/device/format 也要可配置。
5. 零配置可用优先：默认 ZH + cpu + wav + 默认韵律应能直接试听，不要求用户先填 Key。

---

## 2. 现状勘察结论（本机已确认）

### 2.1 已存在、可复用

| 能力 | 位置 | 结论 |
| --- | --- | --- |
| Melo 引擎文件 | src/open_llm_vtuber/tts/melo_tts.py | 存在，但是旧接口：默认 EN-Default，只传 speed，未支持 ZH 与四个韵律参数 |
| TTS 工厂 | src/open_llm_vtuber/tts/tts_factory.py | 已有 melo_tts 分支，但只传 speaker/language/device/speed |
| Melo 配置模型 | src/open_llm_vtuber/config_manager/tts.py | 已有 MeloTTSConfig，字段不全（缺 sdp_ratio/noise_scale/noise_scale_w/format） |
| TTS 设置路由 | src/open_llm_vtuber/routes.py | 已有 TTS 目录/试听逻辑，但 TTS_DEFAULTS、TTS_VOICE_CATALOG、TTS_MODEL_CATALOG 均无 melo_tts |
| 聊天 TTS 调度 | conversations/tts_manager.py | 通用，Melo 引擎符合 TTSInterface 即可接入，无需改调度 |
| 前端设置 | renderer/src/settings-ui.ts | 已有 TTS Provider 卡片，但当前是通用 voice/model/ws_url 三件套，需为 Melo 增加语言/设备/韵律参数 |
| 现有本地 TTS 接口 | /api/test-tts、/tts-ws | 可复用；/api/test-tts 目前只试听，不适合作为稳定外部接口 |

### 2.2 缺口（本计划要补）

1. melo 依赖未安装、未写入依赖清单（当前 venv 无 melo）。
2. melo_tts.py 与用户给的 API 不匹配：未使用 spk2id[language]，未传 sdp_ratio/noise_scale/noise_scale_w/format。
3. MeloTTSConfig 缺字段，routes.py 目录没有 Melo，conf.yaml 没有 melo_tts 默认块。
4. 前端设置中心看不到“本地 Melo”入口，也没有韵律滑杆。
5. 没有独立稳定的 HTTP 本地 TTS 接口（现有 /tts-ws 是 WebSocket，/api/test-tts 是设置试听）。
6. 无 Melo 单测/配置解析测试/接口测试。

---

## 3. 总体方案

    前端设置中心（本地/免费分组 + Melo 高级参数）
            │  保存 conf.yaml（melo_tts 块）
            ▼
    TTSFactory.get_tts_engine("melo_tts", ...)
            │
            ▼
    MeloTTSEngine（重写为 ZH 优先、四参数、spk2id 解析）
            │  async_generate_audio() → cache/*.wav
            ├──────────────► 聊天链路 TTSTaskManager
            ├──────────────► /tts-ws（既有本地 TTS WS 接口）
            └──────────────► 新增 POST /api/tts/local/synthesize（稳定本地 HTTP 接口）

Melo 只做合成引擎，不碰 TTS 调度、口型、音频帧封装；这样聊天链路、试听、外部调用共享同一实现。

---

## 4. 阶段划分

| 阶段 | 内容 | 完成判据 |
| --- | --- | --- |
| 阶段 1 | Melo 引擎与配置模型重写 | 后端可独立合成 ZH/EN 音频，四参数生效 |
| 阶段 2 | 路由/工厂/conf 接入 | 设置中心能识别 melo_tts，试听可用 |
| 阶段 3 | 本地 TTS HTTP 接口 | POST /api/tts/local/synthesize 可被 curl 调用 |
| 阶段 4 | 前端设置 UI | Melo 卡片 + 语言/设备/韵律参数可配置 |
| 阶段 5 | 测试/文档/绿门/收口 | 全量绿门通过，计划文档标记执行完成 |

---

## 5. 阶段 1：Melo 引擎与配置模型（后端基础）

### 5.1 依赖落地

- 选定并固定一个 Python 3.12 可用的 MeloTTS 安装源。
  - 优先验证 pip install git+https://github.com/myshell-ai/MeloTTS.git 或维护中 fork；
  - 若官方仓库与当前 numpy<2 / torch>=2.6 约束冲突，单独记录冲突项，不通过放宽主依赖来硬装。
- 在 pyproject.toml 增加可选依赖或注释，明确 Melo 是本地可选能力，安装失败不能影响默认启动。
- 模型缓存写入现有 .hf-cache/ 或 models/melo/，并确认 .gitignore 覆盖。

完成判据：干净 venv 按文档可安装 Melo；未安装 Melo 时选择该 Provider 应给出可读错误，不拖垮启动。

### 5.2 重写 melo_tts.py

目标签名与用户 API 对齐（伪代码）：

    class TTSEngine(TTSInterface):
        def __init__(
            self,
            language: str = "ZH",
            device: str = "cpu",      # cpu / cuda / auto
            speaker: str | None = None,
            speed: float = 1.0,
            sdp_ratio: float = 0.2,
            noise_scale: float = 0.6,
            noise_scale_w: float = 0.8,
            format: str = "wav",
        ):
            self.model = TTS(language=language, device=device)
            spk = self.model.hps.data.spk2id
            self.speaker_id = spk[speaker] if speaker and speaker in spk else spk.get(language, next(iter(spk.values())))

generate_audio 调用：

    self.model.tts_to_file(
        text=text,
        speaker_id=self.speaker_id,
        output_path=file_name,
        sdp_ratio=self.sdp_ratio,
        noise_scale=self.noise_scale,
        noise_scale_w=self.noise_scale_w,
        speed=self.speed,
        format=self.format,
    )

额外要求：

- file_extension = format，缓存文件名带正确后缀。
- 保留并修正 LookupError 时下载 nltk 词性标注器的兜底（仅 EN 场景需要，ZH 不应触发）。
- 所有参数做范围钳制：speed 0.5~2.0、sdp_ratio 0.0~1.0、noise_scale 0.0~1.0、noise_scale_w 0.0~1.0。
- 构造失败（模型未下载、依赖缺失、显存不足）抛出带中文指引的 RuntimeError，上层 tts_manager.py 已有 silent/降级逻辑承接。

完成判据：mock Melo 单测覆盖 spk2id 解析、参数传递、范围钳制、构造失败路径。

### 5.3 更新 MeloTTSConfig

在 config_manager/tts.py 中把字段改为：

    language      Literal["ZH","EN","ES","FR","JP","KR"] = "ZH"
    device        str = "cpu"          # cpu / cuda / auto
    speaker       Optional[str] = None
    speed         float = 1.0          # 0.5 ~ 2.0
    sdp_ratio     float = 0.2          # 0.0 ~ 1.0
    noise_scale   float = 0.6          # 0.0 ~ 1.0
    noise_scale_w float = 0.8          # 0.0 ~ 1.0
    format        str = "wav"

DESCRIPTIONS 补中英文说明，特别写清四参数只改韵律、不改音色。

完成判据：MeloTTSConfig.model_validate() 对合法/非法值行为符合预期，配置解析测试绿。

---

## 6. 阶段 2：路由/工厂/conf 接入

### 6.1 tts_factory.py

melo_tts 分支改为读取完整字段：

    MeloTTSEngine(
        language=kwargs.get("language") or kwargs.get("voice") or "ZH",
        device=kwargs.get("device") or kwargs.get("model") or "cpu",
        speaker=kwargs.get("speaker"),
        speed=kwargs.get("speed", 1.0),
        sdp_ratio=kwargs.get("sdp_ratio", 0.2),
        noise_scale=kwargs.get("noise_scale", 0.6),
        noise_scale_w=kwargs.get("noise_scale_w", 0.8),
        format=kwargs.get("format", "wav"),
    )

保留 voice→language、model→device 的兼容映射，降低旧设置迁移成本。

### 6.2 routes.py 目录与试听

- TTS_DEFAULTS["melo_tts"] = {"name": "Melo TTS（本地）", "key_env": None}。
- TTS_VOICE_CATALOG["melo_tts"] = ["ZH","EN","ES","FR","JP","KR"]（语言）。
- TTS_MODEL_CATALOG["melo_tts"] = ["cpu","cuda","auto"]（设备）。
- /api/test-tts 对 melo_tts 传全量参数，支持 UI 覆盖 language/device/speed/sdp_ratio/noise_scale/noise_scale_w/format。
- 试听文案固定为中文短句“你好，我是白～这是一段本地试听。”，验证中英混合可读。

### 6.3 conf.yaml

在 tts_config 增加默认块（默认不启用，仅作为可切换 Provider 配置）：

    melo_tts:
      language: 'ZH'
      device: 'cpu'
      speaker: ''
      speed: 1.0
      sdp_ratio: 0.2
      noise_scale: 0.6
      noise_scale_w: 0.8
      format: 'wav'

保持当前默认 tts_model: 'cosyvoice_cloud' 不变，避免破坏已有用户配置。

完成判据：后端 pytest 中设置 API 能识别 melo_tts，选择/取消/试听路径不报错；mock 引擎试听返回音频。

---

## 7. 阶段 3：本地 TTS HTTP 接口

新增稳定 REST 接口，作为对外“本地 TTS 接口”，供本机其他脚本/工具调用。

### 7.1 接口契约

POST /api/tts/local/synthesize

请求 JSON（字段与 Melo API 对齐）：

    {
      "text": "你好，这是本地合成测试。",
      "language": "ZH",
      "device": "cpu",
      "speaker": null,
      "speed": 1.0,
      "sdp_ratio": 0.2,
      "noise_scale": 0.6,
      "noise_scale_w": 0.8,
      "format": "wav"
    }

响应（v1 只做 wav，format 保留以兼容后续扩展）：

    {
      "ok": true,
      "audio_path": "cache/tts_local_xxx.wav",
      "audio_url": "/api/tts/local/audio?name=tts_local_xxx.wav",
      "format": "wav",
      "duration_ms": 1234
    }

失败响应：

    { "ok": false, "error": "Melo 模型未下载：首次使用请先在设置中心试听以触发下载，或检查网络/镜像。" }

### 7.2 配套下载端点

GET /api/tts/local/audio?name=...

- 只允许读取 cache/ 下由本接口生成的 *.wav。
- 拒绝路径穿越（..、绝对路径、非 cache/ 前缀）。

### 7.3 实现要求

- 引擎懒加载并缓存，避免每次请求重新加载模型。
- 合成放到 asyncio.to_thread，不阻塞事件循环。
- 文本长度硬上限（如 500 字符），超过返回 400。
- 并发用同一把锁/信号量，避免同时加载多份 Melo 模型。

完成判据：curl 可合成一段 WAV；路径穿越被 400/403 拒绝；重复调用不重复加载模型。

---

## 8. 阶段 4：前端设置 UI

- TTS_PROVIDER_LABELS 增加 melo_tts: 'Melo TTS（本地）'。
- TTS_CATEGORIES 的 local 组加入 melo_tts。
- Melo 卡片字段映射：
  - 语言 = voice 下拉（ZH/EN/ES/FR/JP/KR）。
  - 设备 = model 下拉（cpu/cuda/auto）。
  - 隐藏无意义的 WebSocket URL 输入。
  - 增加高级参数：speed、sdp_ratio、noise_scale、noise_scale_w 四个滑杆/数字输入，显示默认值与中文提示。
- 保存 payload 带上完整 melo_tts 参数，前端“试听/检查”走 /api/test-tts 并真正合成。
- 若本地未安装 Melo，试听失败时显示“未安装本地 Melo / 模型未下载”的可读错误，而不是 Key 未配置。

完成判据：切到 Melo 能改语言/设备/四参数并试听；保存后重启后端，聊天链路使用 Melo；前端 vitest 覆盖渲染与保存逻辑。

---

## 9. 阶段 5：测试 / 文档 / 绿门 / 收口

### 9.1 测试

新增：

- tests/test_melo_tts.py（或 PC 后端 tests/test_melo_tts.py）：mock melo.api.TTS，验证参数传递、spk2id 回退、缓存文件名、失败路径。
- tests/test_local_tts_api.py：REST 接口契约、路径穿越、空文本、超长文本、并发。
- 配置解析测试：MeloTTSConfig 字段默认值/范围。
- 前端 renderer/tests/settings-melo.test.ts：Melo 卡片渲染与 payload 构造。

### 9.2 文档

- Live2D-Ai-pc/README.md：增加“本地 Melo TTS”启用步骤、首次下载模型说明、四参数经验值。
- docs/README.md：计划索引指向本文件。
- PLAN.md：顶部当前执行口径指向本文件。
- CHANGELOG.md：记录本轮绿门数字与未验证项。

### 9.3 绿门

统一跑：

    python scripts/verify_all.py
    cd Live2D-Ai-pc/open-llm-vtuber && pytest tests/ -q
    cd renderer && npx vitest run && npx tsc --noEmit && npm run build
    python shared/check_cross_platform_consistency.py
    python shared/validate_registry.py

Melo 真实模型下载与发音属于真机/本机冒烟项，不阻塞单测绿门；无网络时允许登记跳过。

### 9.4 收口

- git status --short 逐行确认无 Key、缓存、运行时状态。
- 分语义 commit：依赖/引擎/配置/路由、REST 接口、前端、测试文档。
- 更新本文件“执行状态”，作者裁决后打 v1.0.0。

---

## 10. 完成判据（Definition of Done）

1. melo_tts 出现在 PC 设置中心“本地/免费”分组，可配置语言、设备与四个韵律参数。
2. 聊天链路可用 Melo 本地合成，不依赖任何云端 TTS Key。
3. POST /api/tts/local/synthesize 可被本机脚本 curl 调用并返回 WAV。
4. language=ZH 默认可用，中英混合句子可读；四参数在 mock 与真实冒烟中均被传递。
5. 前端 vitest、后端 pytest、tsc、build 全绿；路径穿越/空文本/超长文本被拒绝。
6. 文档、CHANGELOG、计划索引更新一致。

---

## 11. 明确不做

- 不碰 Android 代码与构建。
- 不做 Melo 参考音频克隆/换声；参考音频克隆继续走 CosyVoice/GPT-SoVITS + voice_library.py。
- 不做本地 GPU 模型优化（可支持 device=cuda 配置，但不为性能调优）。
- 不做跨设备云同步、插件市场、直播平台接入。
- 不在本计划内清零全树历史 ruff 债。

---

## 12. 风险登记

| 风险 | 缓解 |
| --- | --- |
| MeloTTS 官方仓库对 Python 3.12 / numpy<2 兼容性不确定 | 阶段 1 先做依赖验证，选可用的 fork/commit；安装失败只影响 Melo，不影响默认启动 |
| 首次下载模型需要网络/hf-mirror | 文档写清；设置中心试听触发下载；允许离线登记跳过 |
| CPU 合成速度慢 | 默认短句试听；聊天段级并发已有信号量，不在本计划压性能 |
| 通用 UI voice/model 字段与 Melo 语义冲突 | 用 voice→language、model→device 映射，避免大改现有设置 UI |
| 外部 HTTP 接口被滥用/路径穿越 | 文本长度上限 + 本地 cache/ 白名单 + 路径校验 |
| 当前工作树仍有大量第五轮未提交改动 | 阶段 5 分语义 commit，Melo 改动独立提交，不混入其他在途工作 |
