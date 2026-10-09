# TTS 选型再评估：8 GB 笔记本上的显存账（2026-10-08）

> 结论：**本机 8 GB 显存里，CosyVoice 3 常驻约 4.7 GB、峰值约 5.2 GB；Windows 桌面侧另有约 2.1–2.7 GB**。
> 空闲时可用显存不到 1 GB，合成中 570–744 MiB。TTFA（首个音频字节）实测 **2.0–9.8 s**，项目验收线是 **≤1 s**。
> 换引擎在工程上是**改一个 `[tts] base_url` + 一个 OpenAI 兼容 shim**（TTS 是核心链路、端点唯一权威），所以真正的问题是选哪个模型，而不是能不能换。
> 本文只读不改：没有动代码、配置、`live2d-ai.toml`、`mods.json`、`.env`。
> 与之配套的问答/口径记录见 [`tts-engine-mod-conversation-2026-10-08.md`](tts-engine-mod-conversation-2026-10-08.md)（含维护者对「本地 TTS Mod」的口径与待裁决项）。

---

## 1. 实测环境与方法

| 项 | 值 |
| --- | --- |
| GPU | NVIDIA GeForce RTX 5060 Laptop，**8151 MiB**，驱动 592.01 / CUDA 13.1（Windows 侧），WSL 内 torch 2.11.0+cu128（compute cap 12.0） |
| TTS 服务 | `/home/skystar/CosyVoice 3.0/shim/server.py --model-dir pretrained_models/Fun-CosyVoice3-0.5B --voices shim/voices.yaml --fp16 --warmup`（PID 380，:8080） |
| 模型 | `Fun-CosyVoice3-0.5B`，磁盘 **9.1 GB**；音色 `skystar` / `skystar_plain`（zero_shot，参考音频 30 字） |
| 项目侧 | `live2d-ai-desktop` :18080 在跑；前端是浏览器里的 Flutter Web（WebGL 舞台 → 占用算在 Windows 侧浏览器进程） |

**方法（可复现）**：WSL 的 `nvidia-smi --query-compute-apps` 对本机进程一律回 `N/A`，**拿不到进程归属**。改用 Windows 性能计数器按进程读专用显存：

```powershell
(Get-Counter '\GPU Process Memory(*)\Dedicated Usage').CounterSamples |
  Where-Object { $_.CookedValue -gt 30MB } | Sort-Object CookedValue -Descending |
  ForEach-Object { '{0}  {1} MB' -f $_.InstanceName, [math]::Round($_.CookedValue/1MB) }
```

TTFA 用原始 socket 读响应体首字节（`curl` 的 `time_starttransfer` 是**响应头**时间，3 ms 级，不能当 TTFA——`cosyvoice3-tts-integration.md:100-104` 已记过这条）。

> **口径边界（必须一起读）**：① 计数器的 InstanceName 是 `pid_<windows-pid>`，WSL 的显存全记在 **`vmwp`**（WSL2 VM worker）名下，看不到 WSL 内部是哪个 Python 进程——本机 WSL 里只有 CosyVoice 用 GPU，故记作它的占用；② 这是**点采样**，不是长时均值；③ WSL 内 `torch.cuda.mem_get_info()` 与 Windows 计数器**口径不同**（同一时刻一个说剩 5.7 GB、一个说剩 1.0 GB），本文一律以 Windows 计数器做横向比较。

---

## 2. 显存去哪了（实测）

### 2.1 两个大头

| 占用方 | 空闲时 | 长句合成中 | 说明 |
| --- | ---: | ---: | --- |
| `vmwp`（WSL VM = **CosyVoice 3**） | **4 724 MB**（合成结束后 `empty_cache` 的稳态） | **5 366 MB 峰值**（0.25 s 间隔采样） | 三态见 §2.1.1 |
| Windows 桌面（合计） | ~2 115 MB | ~2 600–2 700 MB | `dwm` 809–837、`livehime`（B 站直播姬）583–746、`chrome` 554–558、`WindowsTerminal` 131–152、`Code` 76–87、`explorer` 71、`QQ` 55–63、`cloudmusic` 50–54、`steamwebhelper` 41–47、`wallpaper64`（Wallpaper Engine）33–65… |
| **合计 / 8151 MiB** | ~6 851 MB | ~7 800–7 900 MB | `nvidia-smi` 空闲报 free **718–960 MiB**；合成中 570–744 MiB |

#### 2.1.1 三种状态（0.25 s 间隔采样，58 字长句）

| 状态 | `vmwp`（= CosyVoice） | 全卡总占用 | 余量 |
| --- | ---: | ---: | ---: |
| A 刚加载 / 预热后（allocator 缓存未清） | 5 192–5 254 MB | 7 386–7 448 MB | ~700 MB |
| **B 合成中峰值** | **5 366 MB** | **7 560 MB** | **~590 MB** |
| C 每次合成结束后（上游 `torch.cuda.empty_cache()`） | 4 724 MB | 6 917 MB | ~1 230 MB |

**读法**：**TTS 峰值 = 5.37 GB**，比稳态空闲（4.72 GB）高 **约 0.64 GB**（激活 / workspace 瞬态）。全卡同时刻总量 7.56 GB / 8.15 GB，**只剩 ~590 MB**——桌面侧再开一个重 GPU 应用就会开始换出（变慢）。

**两条推论**：

1. **TTS 不是唯一占用方**：桌面侧一个人就吃掉 2.1–2.7 GB（其中 `livehime` + Chrome/Edge 就能再挤出 1 GB）。
2. **WDDM 会超卖**：显存不够时驱动把分配换出到内存，不报错、只变慢——这正好解释了下面 1.24×→2.0× 的 RTF 抖动。

### 2.2 CosyVoice 3 自己的 4.7 GB 由什么组成（按权重体积 + 代码证据推算）

| 部件 | 磁盘 | 显存口径 | 证据 |
| --- | ---: | --- | --- |
| `llm.pt`（含 Qwen2 0.5B 骨干） | 2.02 GB fp32 | fp16 ≈ **1.0 GB** | `cli/model.py` 在 `fp16=True` 时 `llm.half()` |
| `flow.pt`（DiT dim 1024 depth 22） | 1.33 GB fp32 | fp16 ≈ **0.67 GB** | 同上 |
| `hift.pt`（声码器） | 83 MB | ≈ 0.04 GB | — |
| `speech_tokenizer_v3.onnx` | **969 MB fp32** | **≈ 0.9 GB，跑在 CUDA 上** | `cli/frontend.py:46-47`：`providers=["CUDAExecutionProvider" if torch.cuda.is_available() else "CPUExecutionProvider"]` |
| `campplus.onnx` | 28 MB | ≈ 0 | `frontend.py:45`：CPUExecutionProvider |
| `llm.rl.pt` | 2.02 GB | **0（没被加载）** | 磁盘冗余，不是显存问题 |
| 余量（CUDA context / cuBLAS·cuDNN workspace / allocator 碎片 / WDDM） | — | ≈ 1–2 GB | 由「各部件之和 < 实测 4.7 GB」反推 |

**不换引擎就能摘的一项**：把 speech tokenizer 从 CUDA 改到 CPU，**省约 0.9 GB**。代价只在「新注册音色 / 变声（`/v1/audio/voice-conversion`）」时变慢——正常合成走的是 `register_voices()` 启动时预编码好的 `spk2info`（`shim/server.py:201-233`），不碰这个 ONNX。这一项要改 shim 并重启服务，**本轮未执行**。

次要项：`PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True`（减碎片）；shim 每次合成后 `torch.cuda.empty_cache()`（`model.py` 尾部）降均值但抬高下一次延迟，值得开/关各测一轮。

---

### 2.3 项目自身占多少（2026-10-08 23:0x 实测）

| 项 | 实测 | 说明 |
| --- | --- | --- |
| Rust 服务 `live2d-ai-desktop --web` | **RSS 23.7 MB** / VSZ 690 MB / 11 线程 / 11 个 fd | `scripts/ignite.sh` 点火后实测 |
| 它的显存 | **0**（`/proc/<pid>/fd`、`maps` 里没有 nvidia 设备） | 舞台渲染在浏览器里，不在 Rust 进程 |
| 它的 CPU | 空闲 ≈0%；200 次 `GET /api/v1/app/status` 合计 50 ms（**0.25 ms/请求**） | — |
| 前端静态产物 | `shell/flutter/build/web` 30 MB（`canvaskit.wasm` 7.28 MB + `main.dart.js` 3.22 MB + 其余） | 一次性下载 |
| wasm 渲染面 | `crates/l2d-wasm-demo/dist` 5.4 MB | `/render` 用 |
| Live2D 资产 | `assets/models` 24 MB | 用户自带 |
| 浏览器里的舞台 | **量不出来（不可按标签页拆分）**：Windows 侧浏览器进程合计 554–810 MB 专用显存，含所有标签页 | 舞台的真实 GPU 成本在这份数字里 |

**结论**：项目**后端**几乎不占资源（23.7 MB 内存、0 显存、0.25 ms/请求）；显存成本全在**浏览器渲染舞台**那一侧，量级是百 MB，不是 GB。

### 2.4 CPU 能不能分担（实测）

| 实验 | GPU | CPU | 结论 |
| --- | ---: | ---: | --- |
| speech tokenizer（`speech_tokenizer_v3.onnx`，969 MB）编码 13.4 s 参考音频 | 0.02 s | 1.38 s（1 线程，上游默认 `intra_op_num_threads=1`）/ **0.29 s（8 线程）** | **冷路径，可以挪 CPU**：只在注册音色 / 变声时跑 |
| 全链 CPU：12 字 | RTF 0.94–1.24× | **RTF 12.6×**（58.6 s 出 4.64 s 音频，TTFA 25.6 s） | 进程峰值 RSS **6.5 GB** / 机器 7.9 GB |
| 全链 CPU：3 字「你好。」 | — | RTF 15.2×（17 s 出 1.12 s 音频） | 同上 |

**结论**：CPU **可以**分担冷路径（语音 token 化：省 ~0.9 GB 显存，代价 0.3–1.4 s，且只在启动/变声时发生）；**不能**分担每句热路径——全链 CPU 是 13–15× 实时、还要 6.5 GB 内存，在这台 8 核 / 8 GB 机器上不可用。

---
## 3. 延迟实测（同一会话，串行合成）

| 文本 | 字数 | 响应头 | **TTFA（首个音频字节）** | 总耗时 | 音频时长 | RTF |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 你好。 | 3 | 0.00 s | **2.01 s** | 2.02 s | — | — |
| 今天天气不错，风有点凉。 | 12 | 0.01 s | **5.69 s**（另一次 7.51 s） | 7.03 s（另一次 7.51 s） | 6.08 s（291 840 B） | 1.24×（另一次 1.96×） |
| （上一句 + 公园散步/煮茶/慢慢聊，58 字） | 58 | 0.00 s | **9.81 s** | 24.05 s | — | ≈0.9× |

**要点**：

- **「流式」在本机没有兑现「首包百毫秒」**。上游阈值是 `token_hop_len=25` 个语音 token + `pre_lookahead_len`（`cli/model.py:341-357`：`while True: time.sleep(0.1)` → 凑够 25+pre_lookahead 才 `token2wav` 一次 `yield`）。首批要等 LLM 出 ~28 个 token + flow 匹配 + 声码器，实测 **2–10 s**。
- 12 字那句 **TTFA 5.69 s / 总 7.03 s**：能看出确实分批到达，但**首批本身太慢**，不是「客户端多加了一层等整段」。
- 与验收线对比：项目判据是 **TTFA ≤ 1 s**（`docs/verification/acceptance-metrics-report.md:217-223`，「>1 s 有打断感」）。**现状差 2–10 倍**。
- 结论：**显存和延迟是两个独立问题**，换引擎时两者都要实测；上游文档里 CosyVoice2 的「首包 150 ms」是厂家口径，本机不能照抄。

---

## 4. 候选必须满足的项目侧硬约束（先钉死，再谈模型）

| # | 约束 | 真源 |
| --- | --- | --- |
| 1 | 唯一路径 `POST {base_url}/audio/speech`，JSON `{input, model?, voice, response_format}` | `crates/live2d-ai-runtime/src/tts.rs:22-41` |
| 2 | `response_format = "pcm"` = 裸 s16le 单声道（唯一保证可解码）；`wav` 能解析但**要整段缓冲**；mp3/flac 明确拒绝 | `runtime/src/audio/`、`cosyvoice3-tts-integration.md:110-133` |
| 3 | 采样率/声道来自 `[tts] sample_rate`/`channels`（现为 24 000 / 1），前端按句拼 WAV | `live2d-ai.toml:38-48`、前端 `sentence_assembler` |
| 4 | **一句一单元**：只按真实句读切分，不做句中切；句边界由引擎的 `start`/`end` 分片给出 | AGENTS.md「语音输出约定」 |
| 5 | 现用音色是 **zero-shot 克隆**（`voice="skystar"`，参考音频 + 逐字一致 ref_text） | `CosyVoice 3.0/shim/voices.yaml` |
| 6 | TTS 不是 Mod，端点唯一权威 = `[tts]`；缺配置时链路降级为空句、不报错 | `docs/architecture/tts-is-core.md` |

**第 6 条是这次选型的最大红利**：换引擎 = 换一个 HTTP 服务 + 改 `base_url`，Rust/Flutter 一行不用改。**代价**是要顺带解决音色平移（参考音频、注册流程）与上面 1–5 的形状对齐。

---

## 5. 候选矩阵（证据分三级：本机实测 / 按体积推算 / 公开资料）

| 方案 | 许可 | 期望显存 | 首包/延迟 | 中文克隆 | 接入成本 | 证据等级 |
| --- | --- | --- | --- | --- | --- | --- |
| **CosyVoice 3-0.5B（现状）** | Apache-2.0 | **常驻 4.7 GB / 峰值 5.2 GB** | TTFA 2.0–9.8 s | ✅ zero-shot | 0 | **本机实测** |
| CosyVoice 3 + tokenizer 挪 CPU | Apache-2.0 | ≈3.7–3.8 GB | 不变 | ✅ | 改 shim 一处 + 重启 | 推算（−0.9 GB 有文件与代码实锤） |
| **CosyVoice2-0.5B** | Apache-2.0 | ≈2–3 GB | 上游称首包 150 ms（**未在本机实测**） | ✅ | **shim 已支持**（`load_model` 按 `cosyvoice2.yaml` 自动选类）；换模型目录 + 重注册音色 | 推算 + 公开资料 |
| Qwen3-TTS-0.6B | Apache-2.0 | 公开资料：4 GB 起 | 官方口径端到端 97 ms（需流式后端） | ✅ 3 秒克隆 | 需写新 shim（协议不同） | 公开资料 |
| GPT-SoVITS | MIT | 社区常称 2–4 GB | 非流式、句子级 | ✅ 强 | 需写 shim + 音色准备 | 公开资料（弱） |
| F5-TTS | MIT | 文档：**最低 4 GB 空闲、建议 6 GB** | 生成式，非实时 | ✅ | 需写 shim | 公开资料 |
| IndexTTS2 | Apache-2.0 | 公开资料：**FP16 ≈7.8 GB** | 1.2 s/次（RTX 3060 口径） | ✅ | 需写 shim | 公开资料 |
| MeloTTS | MIT | **0（CPU）** | CPU 实时性一般 | ❌ 无克隆 | 需写 shim + 下模型 | 本机已装（`melo` conda env，melotts 0.1.2 / torch 2.11.0+cu126；**权重当前不在盘上**，`~/tts/test_zh.wav` 是 2026-08-21 的产物） |
| Kokoro / sherpa-onnx VITS | Apache-2.0 | 0（CPU）或 <1 GB | 可流式 | ❌ 克隆弱 | 需写 shim | 公开资料 |
| 云端（阿里云 CosyVoice / Qwen TTS Realtime、火山、MiniMax） | 商业 | **0** | 200–500 ms 级 | ✅（各家音色/克隆） | 协议兼容则**只改 `[tts]`**；否则加轻 shim | 公开资料 |

**不推荐档（在这台机器上）**：IndexTTS2（FP16 7.8 GB）、F5-TTS（≥4 GB 空闲）、VoxCPM2（2B）、GPT-SoVITS 大版——桌面侧已经占 2.1–2.7 GB，这些一上来就是 4 GB+，**共存必打穿**（超卖后表现为卡顿与 RTF 抖动，不是干净的报错）。

---

## 6. 推荐路线（按「这台机器」的预算规则）

**预算规则**：8 GB − 桌面实测 2.1–2.7 GB − 浏览器/舞台（含在桌面里）≈ **可用 5.4–6.0 GB**；留 1.5 GB 给抖动与浏览器增长 → **TTS 目标常驻 ≤3.5 GB**。

1. **止血（今天可做，不换引擎）**：speech tokenizer → CPU（**实测** −0.9 GB、代价 0.3–1.4 s 且只在注册音色/变声时发生）→ 峰值约 4.4–4.5 GB（现测 5.37 GB）；顺手关掉 Windows 侧 GPU 大户（`livehime` 0.6–0.75 GB、Chrome/Edge、Steam、Wallpaper Engine）还能再挤出 1–1.5 GB。可选 `expandable_segments:True`。
2. **主线（换模型，效果与显存同时改善）**：**CosyVoice2-0.5B**。同门、许可相同、shim 已支持、且它本身就是为流式设计的 0.5B 档；期望 2–3 GB。**切换前必须实测三个数**：常驻/峰值显存（同一套计数器）、TTFA（原始 socket 首字节）、RTF（≥58 字长句）。三个都优于现状再改 `[tts] base_url`；不达标就退回现状并走第 3 条。
3. **0 显存档（要「桌面和渲染永远不抢显存」时）**：
   - **CPU ONNX**（MeloTTS / sherpa-onnx Kokoro 或 VITS-zh）：离线可用、0 显存、写一个同形状 shim 即可；代价是**没有零样本克隆**（内置音色）与表现力下降——与「桌宠安静说话」的定位并不冲突。
   - **云端**（阿里云 CosyVoice / Qwen TTS Realtime 等）：0 显存 + 低延迟，且本项目 LLM 本来就走 DeepSeek 云端，"TTS 上云"不是新增依赖类别；代价是费用、断网不可用，与「本地优先」口径**冲突，需维护者裁决**。

**换引擎的验收清单（缺一不可）**：常驻/峰值显存（Windows 计数器）· TTFA（原始 socket）· RTF（长句）· 协议形状（`pcm` s16le / 采样率 / 声道）· 同一参考音频的音色 A/B · 失败面（400/500/超时/空输入走静音句）· 断网可用性。

---

## 7. 未做 / 边界（如实登记）

- **没改任何代码或配置**，也没重启 TTS 服务；第 2.2 节的 0.9 GB 释放量与第 6 节第 2 条的 2–3 GB 都是**推算**，需实测确认。
- 外部候选的显存/延迟数字来自公开文章与文档（见 §8），**均未在本机实测**，只用于排除明显不合适的档位。
- Windows 计数器是点采样；`vmwp` 归属是「本机 WSL 里只有 CosyVoice 用 GPU」这一事实的推论，不是驱动级归属。
- 未评估：音质主观分、并发（shim 用 `GEN_LOCK` 串行，天然单请求）、长时运行显存泄漏（上游每次合成后 `empty_cache()`）。

---

## 8. 证据索引

| 内容 | 位置 / 来源 |
| --- | --- |
| 本机显存与进程归属 | Windows `\GPU Process Memory(*)\Dedicated Usage` 计数器，2026-10-08 22:2x（脚本见 §1） |
| 本机 TTFA / RTF | 原始 socket 探针（`POST /v1/audio/speech`，读响应体首字节），2026-10-08 |
| shim 配置与音色注册 | `/home/skystar/CosyVoice 3.0/shim/server.py`、`shim/voices.yaml` |
| tokenizer 跑 CUDA | `/home/skystar/CosyVoice 3.0/vendor/CosyVoice/cosyvoice/cli/frontend.py:45-47` |
| 流式阈值与 `time.sleep(0.1)` | `…/cosyvoice/cli/model.py:341-357` |
| 项目 TTS 契约 | `crates/live2d-ai-runtime/src/tts.rs`、`src/config.rs`、`docs/architecture/tts-is-core.md` |
| TTFA ≤1 s 判据 | `docs/verification/acceptance-metrics-report.md:217-223` |
| 旧基线（19 字 → 5.71 s，RTF 1.2×） | `docs/architecture/cosyvoice3-tts-integration.md:100-104` |
| F5-TTS 显存要求 | <https://opensource.labijie.com/Mediapipe4u-plugin/speech/f5_tts/>（「最低显存：4G 空闲；建议 6G」） |
| IndexTTS2 FP16 ≈7.8 GB | <https://cloud.baidu.com/article/4012923> |
| Qwen3-TTS 4 GB 起 / Apache-2.0 | <https://qwen-ai.com/qwen-tts/> |
| 2026 开源 TTS 横向对比（族群与参数） | <https://juejin.cn/post/7649764223339495475> |
| 旧 TTS 选型调研（2026-08，含 GPT-SoVITS/ChatTTS/Kokoro/sherpa-onnx 与云端） | `docs/research/tts-research-report.md` |

---

## 9. 变更历史

- 2026-10-08：首版。因「本项目 + 本地 CosyVoice3 显存占用过多」而做；只读实测 + 选型矩阵 + 三路线建议。
- 2026-10-08（补测）：0.25 s 间隔采到 **TTS 峰值 5 366 MB**（全卡 7 560 / 8 151 MB，余 ~590 MB）、稳态空闲 4 724 MB；补项目自身占用（后端 RSS 23.7 MB、0 显存、0.25 ms/请求）与 CPU 分担实测（tokenizer 冷路径 0.29–1.38 s、省 ~0.9 GB；全链 CPU RTF 12.6–15.2×、RSS 6.5 GB，不可用）。补测期间为取样重启过 8080 上的 shim 与 18080 上的服务。
