# 语音输入 sidecar（可用示例）

> 把**一段音频**变成角色开口说的一句话：
> `音频 → ASR（本目录脚本）→ 清洗 → POST /api/v1/voice/transcript → 主链路`。
>
> **ASR 本体不在 Live2D-Ai 主仓**：推理运行时（whisper / sherpa-onnx /
> 平台 SDK / 云服务 CLI）住在**这个 sidecar 进程**里；Rust 只收一段文本。
> 契约全文见 [`docs/voice-input.md`](../../voice-input.md)。

---

## 1. 数据流与职责边界

```
麦克风 / 录音文件
   │
   ├─ voice_sidecar.py  --transcriber fake            （读同名 .txt，开箱即跑）
   │                    --transcriber cmd:"<ASR 命令>"  （你自己的 ASR CLI）
   │
   ├─ clean_transcript()          去零宽/控制符、折叠空白、去首尾（与 Rust 同语义）
   │
   └─ POST /api/v1/voice/transcript  {"text": "...", "token"?: "..."}
          │
          └─ Live2D-Ai: 清洗（再洗一次，服务端是权威）→ supervisor.say
                 → LLM → TTS → 口型 → Live2D
```

| 位置 | 做什么 | 交付物 |
|---|---|---|
| **Live2D-Ai 主仓（Rust）** | loopback 校验、token、启停门禁、`clean_transcript`、`supervisor.say` | `crates/live2d-ai-desktop/src/web_api/voice_routes.rs` + `live2d-ai-mod-voice-input` |
| **sidecar（本目录，Python）** | 拿音频 → 调 ASR → 清洗 → POST | `voice_sidecar.py` + `fixtures/` |
| **主链路** | 与人设 / TTS / 口型共用一条链 | `supervisor` / `live2d-ai-runtime` |

**为什么 ASR 不进 Rust**：ASR 推理运行时动辄几十 MB～几 GB（模型 + onnx/torch），
把它静态链进本地优先的桌面核心会让「主链可编译 / 可分发」依赖于一个随时在变的
推理生态。sidecar 挂了只影响语音输入，**不影响主链路**。

---

## 2. 依赖

- **Python 3.8+**，**只用标准库**（`argparse / json / os / shlex / subprocess /
  sys / unicodedata / urllib`）——**不需要** `pip install`，没有 `requirements.txt`。
- 想接真实 ASR：自己装好那个 CLI，用 `--transcriber cmd:"..."` 指过来即可
  （见 §8）。**不要**把 ASR 依赖加进 Rust workspace。

---

## 3. 一条可复制命令

先点火服务（另一个终端）：

```bash
./scripts/ignite.sh          # 默认 127.0.0.1:18080
```

再跑 sidecar（**干跑**，只打印要发的请求）：

```bash
python3 docs/examples/voice-sidecar/voice_sidecar.py \
  --audio docs/examples/voice-sidecar/fixtures/fake_zh.wav --dry-run
```

输出：

```
[voice-sidecar] dry-run（未发请求）
  POST http://127.0.0.1:18080/api/v1/voice/transcript
  body {"text": "把窗户关小一点"}
```

去掉 `--dry-run` 就是真发送：

```bash
python3 docs/examples/voice-sidecar/voice_sidecar.py \
  --audio docs/examples/voice-sidecar/fixtures/fake_zh.wav
# [voice-sidecar] ok（HTTP 200）：已注入「把窗户关小一点」
```

内置 fixture：`fixtures/fake_zh.wav`（16 kHz / 单声道 / 16-bit，0.25 s 静音，
8044 字节）配同名 `fixtures/fake_zh.txt`（fake 后端的「转写结果」，首尾带空白，
用来演示清洗）。**换任意音频文件**都能跑；没有同名 `.txt` 时用
`--fake-text "..."` 或固定串。

> ⚠️ `voice-input` Mod **缺省停用**。没启用时端点回 `403 mod_disabled`——
> 在前端「Mod 管理」里打开，或：
> `curl -X POST http://127.0.0.1:18080/api/v1/mods/voice-input/enable`。

---

## 4. 命令行 / 环境变量

| 参数 | 等价环境变量 | 说明 |
|---|---|---|
| `--audio <file>` | — | 音频文件（wav 或任意文件）。`fake` 后端按**同名 `.txt`** 取转写 |
| `--transcriber fake` | — | 缺省。读同名 `.txt`；没有就用 `--fake-text` / 固定串 |
| `--transcriber 'cmd:"<命令>"'` | — | 把音频路径**追加**到命令末尾，读 stdout 当转写 |
| `--fake-text <text>` | — | fake 后端在没有同名 `.txt` 时使用 |
| `--url <url>` | `VOICE_INPUT_URL` | 缺省 `http://127.0.0.1:18080/api/v1/voice/transcript`；只给主机（如 `127.0.0.1:18080`）会自动补路径 |
| `--token <token>` | `VOICE_INPUT_TOKEN` | 可选；以 `Authorization: Bearer` 头发送；**dry-run 里打码成 `***`** |
| `--timeout <sec>` | — | 缺省 30 |
| `--dry-run` | — | 只打印 URL + JSON，**不发请求** |
| `--selftest` | — | 离线自检（见 §7），退出码 0 / 非 0 |

`--selftest` 不需要 `--audio`。

---

## 5. 失败码表（退出码）

| 退出码 | 含义 | 触发场景 |
|---|---|---|
| `0` | 成功 | HTTP 2xx 且响应 `ok:true` |
| `2` | **参数或依赖错** | 缺 `--audio`、音频文件不存在/读不到、`--transcriber` 写法非法、`cmd:` 里的可执行文件不存在 |
| `3` | **转写失败** | 同名 `.txt` 读失败、`cmd:` 非 0 退出或超时、转写清洗后为空 |
| `4` | **HTTP 非 2xx**（或请求发不出去） | `400` / `401` / `403` / `405` / `415` / `503`；连接被拒（服务没起）、超时 |
| `5` | **服务端 `ok:false`** | 目前只有 `busy`：主链忙碌，本条已被丢弃 |

服务端错误码 → 处置（脚本已内置同一张表，`ERROR_HINTS`）：

| HTTP | `error.code` | 退出码 | 处置 |
|---|---|---|---|
| 400 | `invalid_payload` | 4 | 请求体不合法（缺 `text` / 非字符串）；通常意味着脚本与服务端版本不匹配 |
| 400 | `empty_transcript` | 4 | 清洗后为空（空白 / 零宽字符）——这段音频没有可注入的文本 |
| 400 | `text_too_long` | 4 | 清洗后 > 2000 字符：切短再发 |
| 401 | `unauthorized` | 4 | token 缺失/不匹配：查 `--token` / `VOICE_INPUT_TOKEN` |
| 403 | `mod_disabled` | 4 | `voice-input` Mod 停用：先启用 |
| 403 | `origin_denied` / `origin_required` | 4 | Origin 非 loopback 同源 / 缺 Origin 且服务端未开 `allow_no_origin` |
| 405 | `method_not_allowed` | 4 | 只接受 POST（脚本不会犯） |
| 415 | `unsupported_media_type` | 4 | `Content-Type` 必须是 `application/json`（脚本已带） |
| 503 | `supervisor_unavailable` | 4 | supervisor 未就绪（配置不完整）：先让主链跑起来 |
| 200 | （无） | **5** | 响应是 `{"ok":false,"error":{"code":"busy"}}`——**刻意用 200**，本条已丢弃 |

**`busy` 为什么是 200 而不是 5xx**：请求格式没问题，是主链当前忙；发送方
**退避重试**即可，用 5xx 会诱导脚本/反向代理按「服务故障」重试到刷屏。
脚本收到 `busy` 打印一行处置建议并以 `5` 退出，不会自动重发。

---

## 6. `--dry-run`

只打印，不联网：

```bash
python3 docs/examples/voice-sidecar/voice_sidecar.py \
  --audio docs/examples/voice-sidecar/fixtures/fake_zh.wav \
  --url 127.0.0.1:18080 --dry-run
#   POST http://127.0.0.1:18080/api/v1/voice/transcript
#   body {"text": "把窗户关小一点"}
```

用途：确认 URL 归一化与清洗结果；**token 在 dry-run 输出里是 `***`**，
所以把 dry-run 贴进 issue / 日志不会泄露密钥。

---

## 7. `--selftest`（离线自检）

```bash
python3 docs/examples/voice-sidecar/voice_sidecar.py --selftest
# [selftest] 全部通过（49 项检查；离线，未发起任何请求）
```

覆盖：**清洗语义**（折叠空白 / 全角空格 / 零宽与控制符 / 纯空白 → 空）、
**payload 形状**（`text` 必带、`token` 非空才带、可 JSON 往返、打码）、
**错误码映射**（HTTP + `ok:false` → 退出码，每个服务端错误码都有处置建议）、
**URL 归一化**、**transcriber 解析**、**fixtures 存在**。
退出码 `0` = 全过，非 `0` = 有失败项（逐条打印到 stderr）。**不发起任何网络请求**。

---

## 8. 接真实 ASR（`--transcriber cmd:`）

把你的 ASR CLI 包一层即可；脚本会把音频路径**追加**在命令末尾，读 stdout：

```bash
# openai-whisper（本地模型）
python3 voice_sidecar.py --audio mic.wav \
  --transcriber 'cmd:"whisper --model small --language zh --output_format txt --output_dir /dev/stdout"'

# sherpa-onnx / 自研 CLI 同理：命令末尾会被补上音频路径
python3 voice_sidecar.py --audio mic.wav --transcriber 'cmd:"/opt/asr/bin/transcribe --lang zh"'
```

约定：

- 命令 stdout = 转写文本（多余日志请写到 stderr）；
- 命令非 0 退出 / 超时 → 退出码 `3`；可执行文件不存在 → 退出码 `2`；
- 脚本自己也会清洗一遍（与服务端同语义），服务端仍会**再洗一次**并做长度判定
  （权威在服务端）。

**红线**：不要为了「省一个进程」把 ASR SDK / 模型 / 推理运行时加进任何 Rust crate，
也不要在 Mod 里开 socket 或读进程环境——那是 `PLAN-voice-input.md` §Forbidden 与
`AGENTS.md` 的边界。

---

## 9. 与 `POST /api/v1/external/chat` 的差异

两条端点**落点相同**（`supervisor.say` → 同一条主链），差别在**输入侧语义**：

| 维度 | `/api/v1/external/chat` | `/api/v1/voice/transcript` |
|---|---|---|
| 语义 | 任意外部事件（弹幕 / 礼物 / webhook） | **语音转写**（一段音频 → 一句话） |
| 上游 | `external-input` Mod | `voice-input` Mod |
| 清洗 | 无（只有 Mod 的 `prefix` / `text_template` 渲染） | **`clean_transcript`**（零宽 / 控制符 / 空白折叠） |
| 错误面 | `invalid_payload` / `text_too_long` | 多一个 **`empty_transcript`**（清洗后为空单独报） |
| token env | `EXTERNAL_INPUT_TOKEN` | `VOICE_INPUT_TOKEN` |
| Mod config | `token` / `prefix` / `text_template` | `token` / `locale` / `backend` |
| 成功响应 | `{"ok":true,"endpoint":"external.chat"}` | `{"ok":true,"text":"<清洗后文本>"}` |
| 缺省启用 | **是** | **否**（ASR 后端在用户机器上，装了再开） |

**为什么选专用端点（而不是复用 external）**——已钉死（Wave 2 计划 §3A 选项 B）：

1. **语义不同**：语音侧有自己的 `token` / `locale` / `backend`，塞进弹幕端点会让
   两种输入共用一份配置，谁也说不清哪条开关管哪边；
2. **要在主链路上证明清洗真的跑了**：复用 external 等于让 `voice-input` crate
   继续当摆设（它的 `clean_transcript` 只被自己的单测调用）；专用端点把
   「转写 → 清洗 → say」变成一条可演示、可 curl、可回归的链路；
3. **错误面不同**：语音特有「有音频但没听清」= `empty_transcript`，
   与「请求体不合法」分开，发送方才知道该重说还是该改脚本；
4. **启停独立**：`voice-input` 缺省停用（用户没装 ASR），`external-input`
   缺省启用（直播刚需）——两条端点各有各的门禁。

---

## 10. 相关文档

| 内容 | 位置 |
|---|---|
| 端点契约全文（curl / 错误码 / 门禁 / 安全） | [`docs/voice-input.md`](../../voice-input.md) |
| HTTP handler + 回归 | `crates/live2d-ai-desktop/src/web_api/voice_routes.rs`（+ `voice_routes_tests.rs`） |
| Mod（静态 schema / `clean_transcript` / `inject_transcript`） | `crates/live2d-ai-mod-voice-input/src/lib.rs` |
| 同类 sidecar（B 站弹幕） | [`docs/examples/bilibili-sidecar/`](../bilibili-sidecar/) |
