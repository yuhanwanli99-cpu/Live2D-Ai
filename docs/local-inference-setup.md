# 本地 TTS/LLM 接入实操指南

> ## ⚠️ 部分作废（2026-09-11）
>
> **TTS 已从 Mod 系统移出**，改成核心链路。本文中一切
> 「`POST /api/v1/mods/local-tts/enable`」「`[mods.local-tts]` 配置段」
> 「`local-tts` 探测到服务就绪后写 base_url」的说法**都不再成立**。
>
> 现在配 TTS 只有一条路：**写 `live2d-ai.toml` 的 `[tts]` 段**
> （或在 Flutter 的「语音合成」分区里改，它 `PATCH /api/v1/settings` 写回同一段）：
>
> ```toml
> [tts]
> base_url = "http://127.0.0.1:8080/v1"   # 任何 OpenAI 兼容 /v1/audio/speech
> voice = "skystar"
> response_format = "pcm"                  # pcm（默认）或 wav
> sample_rate = 24000
> channels = 1
> ```
>
> 理由与后果见 `docs/architecture/tts-is-core.md`。
> `local-llm` 仍然是一个 Mod，本文 LLM 相关部分继续有效。
>
> 另注：本文写作时的 TTS 例子是 kokoroi-rs；当前默认面向 **CosyVoice 3** 的
> OpenAI 兼容层（见 `docs/architecture/cosyvoice3-tts-integration.md`）。

> 用户实测卡在「没有 TTS，所以无法验证口型/出声」。
> 本文档以**照抄即可执行**的命令为导向，带领你从零搭建本地 LLM + TTS 推理服务，
> 并把 LLM 接入 `local-llm` Mod、把 TTS 接入核心链路，最终打通
> **发消息 → LLM 回复 → TTS 合成 → 浏览器播放 → 口型动**的完整闭环。
>
> **前提**：已按 `headless-deploy.md` 启动 `live2d-ai-desktop --web`，
> 监听 `127.0.0.1:18080`。如未部署，请先完成 headless 部署。

---

## TL;DR

```bash
# 0) 确认服务端已启动（headless-deploy.md）
LIVE2D_AI_WS_AUDIO=1 ./target/release/live2d-ai-desktop --web --http-port 18080

# 1) 启动 Ollama (Windows) 并拉模型
winget install Ollama.Ollama
ollama pull qwen3.5:4b
ollama serve   # PowerShell：设 OLLAMA_HOST=0.0.0.0 再启动

# 2) 启动 TTS 上游（任何 OpenAI 兼容 /v1/audio/speech）
#    当前默认面向 CosyVoice 3：/home/skystar/CosyVoice 3.0/scripts/3-start.sh
#    （2026-09-11 起）TTS **不再是 Mod** —— 把端点写进 live2d-ai.toml：
#      [tts]
#      base_url = "http://127.0.0.1:8080/v1"
#      voice    = "skystar"

# 3) 启用 LLM Mod（TTS 没有 Mod 可启用）
curl -X POST http://127.0.0.1:18080/api/v1/mods/local-llm/enable   \
  -H 'Content-Type: application/json' -H 'Origin: http://127.0.0.1:18080'

# 4) 验证闭环
curl http://127.0.0.1:18080/api/v1/app/status     # tts.base_url 应来自 [tts]
```

---

## 1. 快速闭环（推荐路径）

### 1.1 LLM 侧：Windows Ollama

#### 安装

```powershell
winget install Ollama.Ollama
```

#### 拉取模型

```powershell
ollama pull qwen3.5:4b
```

> 模型名称以 `ollama pull` 输出为准。文档固定写 `qwen3.5:4b` 与
> `live2d-ai-mod-local-llm` 默认 `model = "qwen3.5:4b"` 一致。

#### 启动 Ollama 服务

默认 `ollama serve` 仅监听 `127.0.0.1`，WSL 内**无法**访问。在 Windows PowerShell 中：

```powershell
$env:OLLAMA_HOST = "0.0.0.0"
ollama serve
```

然后在 Ollama 服务管理器（或 `services.msc`）中把 `OLLAMA_HOST=0.0.0.0`
写入环境变量并重启服务，避免每次手动设置。

#### WSL 访问 Windows Ollama

WSL2 默认通过 NAT 连向 Windows。获取 Windows 主机 IP：

```bash
# 在 WSL 内运行
ip route | awk '/default/ {print $3}'
```

输出形如 `172.22.80.1`，然后验证：

```bash
curl http://172.22.80.1:11434/api/tags
```

---

#### local-llm Mod 配置

**重要**：`local-llm` Mod 的探活逻辑**固定使用 `127.0.0.1`**（见
`crates/live2d-ai-mod-local-llm/src/lib.rs` 行 258），不能配置主机 IP。
这意味着：

- **Windows 本机运行桌面程序**：`local-llm` 探活 `127.0.0.1:11434` → 命中
  Windows Ollama。✅ 无需额外配置。
- **WSL 内部运行桌面程序**：`local-llm` 探活 `127.0.0.1:11434` → 命中 WSL
  本地回环，**无法**直接访问 Windows Ollama。需要端口转发。

##### Windows 主机 IP → WSL 端口转发

在 **Windows PowerShell（管理员）**下将 11434 端口转发到 WSL 的 localhost：

```powershell
# 假设 WSL 的 11434 端口已用来跑 ollama，或转发到 Windows 11434
netsh interface portproxy add v4tov4 listenport=11434 listenaddress=127.0.0.1 connectport=11434 connectaddress=172.22.80.1
```

> `connectaddress` 替换为 WSL 主机 IP（从 `ip route` 获得）。
> 仅在当前用户有权限时生效；重启后需重新运行。

---

### 1.2 TTS 侧：kokoroi-rs

#### 下载二进制

访问 [doiito/kokoroi-rs GitHub releases](https://github.com/doiito/kokoroi-rs/releases)
下载对应平台的二进制：

- **Windows**：`kokoroi-rs-x86_64-pc-windows-msvc.zip`
- **Linux（WSL）**：`kokoroi-rs-x86_64-unknown-linux-musl.zip`

解压后将 `kokoroi-rs` 放入 PATH 或任意目录。

#### 启动 TTS 服务

```bash
# 监听 8000 端口（默认端口）
./kokoroi-rs serve --port 8000
```

#### 下载模型与声线

按 kokoroi-rs README 下载 **Kokoro 82M ONNX 模型** + 声线文件。
（具体命令以其 README 为准。）

```bash
# 示例（以 kokoroi-rs README 为准）
kokoroi-rs model download
```

#### 验证 TTS API

```bash
curl http://127.0.0.1:8000/v1/models
```

应返回 JSON 包含可用声线模型列表。

---

### 1.3 启用 Mod

#### 启用 HTTP API（必需）

`enable` API 是**纯路径操作**，不需要请求体。虹软浏览器 Origin 校验生效。

```bash
curl -X POST http://127.0.0.1:18080/api/v1/mods/local-llm/enable \
  -H 'Content-Type: application/json' \
  -H 'Origin: http://127.0.0.1:18080'

curl -X POST http://127.0.0.1:18080/api/v1/mods/local-tts/enable \
  -H 'Content-Type: application/json' \
  -H 'Origin: http://127.0.0.1:18080'
```

> **⚠️ 注意**：`enable` 接口**不接受 config body**。
> `mods_routes.rs` 行 129 明确 `let _ = body;` —— body 被丢弃，
> 仅从 URL 解析 `{id}` 和 `{action}`。
> Mod 的 config 来自 `ModRegistry` 初始化时读取的 manifest。

#### config 来自何处？

`cli_entry.rs` 行 130 构造 ModRegistry 时传入的 manifest 是**空 JSON**：

```rust
ModRegistry::new(crate::AVAILABLE_MOD_FACTORIES, &serde_json::json!({}))
```

`parse_mod_config(manifest, id)`（`mod_registry.rs` 行 368）从 manifest
的 `[mods.<id>].enabled` 和 `[mods.<id>].config` 字段读取。
但启动时 manifest 是 `{}`，所以：

- 所有 Mod 初始 `enabled = false`，`config = {}`（空对象）。
- `enable` API 调用后，`enabled` 被设为 `true`，`start_one()` 使用 **空 config**
  启动 Mod —— 即 `local-tts` 使用 `command="kokoroi-rs"`, `port=8000`,
  `voice="af_heart"` 等**默认值**。
- **当前没有任何 API** 用于在运行时修改 Mod config。
  `reload_config` 方法存在（`mod_registry.rs` 行 290）但**未暴露 HTTP 路由**。

#### 如何自定义 config？

**唯一途径**：修改 `live2d-ai.toml` 的 `[mods]` 段，**然后重启桌面程序**。
但当前 `cli_entry.rs` 传入空 manifest `{}`，所以 `[mods]` 段**不会被读取**。
这是一个已知缺陷（详见 §4 已知边界）。

换句话说：

- `local-tts` Mod：只能使用默认 `command="kokoroi-rs"`, `port=8000`,
  `voice="af_heart"`, `externally_managed=false`。
- `local-llm` Mod：只能使用默认 `command="ollama"`, `port=11434`,
  `model="qwen3.5:4b"`, `externally_managed=false`。

如需 `externally_managed=true`，目前**无法**通过 API 设置 ——
需要代码改动（P3 计划）。

---

## 2. externally_managed 的替代方案

由于 `enable` API 不透传 config，且当前 manifest 为空（`{}`），
`externally_managed` 字段只能通过配置文件生效。但 `cli_entry.rs`
传入空 manifest，导致 `[mods.local-llm]` / `[mods.local-tts]`
段**永远不会被解析**。

### 临时解决：直接配置 live2d-ai.toml

绕过 Mod，直接在 `live2d-ai.toml` 中配置 LLM/TTS base_url：

```toml
# live2d-ai.toml
[llm]
base_url = "http://127.0.0.1:11434/v1"
model = "qwen3.5:4b"

[tts]
base_url = "http://127.0.0.1:8000/v1"
voice = "af_heart"

[persona]
system_prompt = "你是桌面上的 Live2D 桌宠。用简短、口语化的中文回答。"
max_history_pairs = 2
```

配置文件搜索顺序（`config_path_for_web()`）：

1. 当前工作目录下的 `./live2d-ai.toml`
2. `$HOME/.config/live2d-ai/live2d-ai.toml`

```bash
# 创建配置目录
mkdir -p ~/.config/live2d-ai
cp live2d-ai.toml.example ~/.config/live2d-ai/live2d-ai.toml
# 编辑 base_url / model / voice
```

然后重启 desktop 程序。Mod 探活 + 写回 settings 的流程将**自动触发**：
local-llm/local-tts 探测到服务就绪后，会通过 `event_tx` 发送
`{"__apply_settings":true,"patch":{"llm":{"base_url":"...","model":"..."}}}`
事件，host 复用 settings PATCH 内核写盘 + `supervisor.reload()`。

---

## 3. 零依赖降级路径（备选）

> **未来增强，当前未实现**。

如果没有任何本地 TTS 服务，可考虑浏览器 `speechSynthesis` API 出声。
当前代码**不支持**浏览器端 TTS fallback：TTS 音频仅通过
`LIVE2D_AI_WS_AUDIO=1` WebSocket 流式推送，浏览器端只负责播放收到的 PCM。
浏览器 `speechSynthesis` 路径属于 P3 增强，**不承诺实现时间**。

---

## 4. 验证清单

### 4.1 LLM 闭环

```bash
# 1) 发消息 → LLM 回复（需要 LLM 配置生效）
curl -X POST http://127.0.0.1:18080/api/v1/chat \
  -H 'Content-Type: application/json' \
  -H 'Origin: http://127.0.0.1:18080' \
  -d '{"text":"你好，喝水提醒！"}'

# 2) 检查 settings 是否被 local-llm Mod 自动写入
curl http://127.0.0.1:18080/api/v1/settings | python3 -m json.tool
# 预期：llm.base_url = "http://127.0.0.1:11434/v1"，llm.model = "qwen3.5:4b"
```

### 4.2 TTS 闭环

```bash
# 1) 确保 WS 音频广播开启
export LIVE2D_AI_WS_AUDIO=1

# 2) 发消息 → 浏览器出声 → 口型动
#    打开浏览器 http://127.0.0.1:18080/，发送聊天消息
#    若 TTS 配置生效，浏览器应播放语音，Live2D 口型同步

# 3) 检查 settings 中的 tts.base_url
curl http://127.0.0.1:18080/api/v1/settings | python3 -m json.tool
# 预期：tts.base_url = "http://127.0.0.1:8000/v1"，tts.voice = "af_heart"
```

### 4.3 逐步排查表

| 症状 | 可能原因 | 检查命令 |
|---|---|---|
| `POST /api/v1/mods/local-tts/enable` 返回 404 | Mod ID 不匹配 | `curl http://127.0.0.1:18080/api/v1/mods`（列出所有注册的 Mod） |
| TTS 探测超时 / `ready=false` | kokoroi-rs 未启动或端口不对 | `curl http://127.0.0.1:8000/v1/models` |
| LLM 探测超时 / `ready=false` | ollama 未启动或未设置 OLLAMA_HOST | `curl http://127.0.0.1:11434/api/tags` |
| settings 中 base_url 为空 | Mod 探活失败，未 write 回 | `curl http://127.0.0.1:18080/api/v1/settings` |
| 浏览器无声 / 无口型 | `LIVE2D_AI_WS_AUDIO=1` 未设置 | 检查环境变量 |
| `POST /api/v1/chat` 返回 503 | supervisor 未装配（无配置文件） | 确认 `live2d-ai.toml` 存在且 base_url 合法 |
| WSL 无法访问 Windows Ollama | 端口未转发 | `netsh interface portproxy show all` |

---

## 5. 已知边界

### 5.1 探活固定 127.0.0.1

`local-llm` 和 `local-tts` Mod 的健康探活**硬编码**使用 `127.0.0.1:{port}`
（`lib.rs` 行 199-202 / 258-259）。`base_url` 在探测通过后写入 settings
为 `http://127.0.0.1:{port}/v1`，**不能配置为其他 host**。

这意味着：

- `externally_managed=true` 时，TTS/LLM 服务**必须**监听 `127.0.0.1`
  或 `0.0.0.0`，且 desktop 程序与服务在**同一主机**上。
- 跨主机（如 WSL ↔ Windows）需要 **系统端口转发** 才能打通探活。

**Windows PowerShell 端口转发示例**（将 Windows 8000 → WSL 8000）：

```powershell
netsh interface portproxy add v4tov4 listenport=8000 listenaddress=127.0.0.1 connectport=8000 connectaddress=<WSL_HOST_IP>
```

> P3 计划支持任意 `host` 配置的探活；当前 P2 阶段尚未实现。

### 5.2 enable API 不透传 config

`POST /api/v1/mods/{id}/enable` **不读取请求体**，不接受任何 config。
Mod 的 config 仅在 `ModRegistry::new()` 初始化时从 manifest 解析，
而 `cli_entry.rs` 传入空 manifest `{}`，因此：

- **任何** Mod 的 config 都是空对象 `{}` —— 使用所有默认值。
- **无法**通过 API 在运行时更改 `port`、`command`、`voice`、
  `externally_managed` 等字段。
- `ModRegistry::reload_config()` 方法存在但**未暴露 HTTP 路由**。

**唯一可行路径**：直接编辑 `live2d-ai.toml` 的 `[llm]` / `[tts]` 段
（绕过 Mod，直接配置 supervisor）。

### 5.3 manifest 未接入配置文件

`cli_entry.rs` 行 130/221 均传入 `&serde_json::json!({})` 作为 manifest，
导致 `[mods.local-tts]` / `[mods.local-llm]` 配置段**永远不被解析**。
这意味着即使用户在 `live2d-ai.toml` 中写了 `[mods.local-tts]`，
也**不会生效** —— Mod 启动时 config 始终为空对象。

### 5.4 真实 TTS 合成验证留给用户

P0-4 验收仅校验到 API 探测层面（TCP 通 + HTTP `/v1/models` 返回 2xx）。
**完整的 TTS 合成闭环**（发送语音合成请求 → 收 PCM → 浏览器播放 →
Live2D 口型动画）的端到端验证，需用户在本地完成实际发声测试。

---

## 6. 参考

- **Mod 注册表**：`crates/live2d-ai-mod-local-llm/src/lib.rs`（默认 `ollama`/`11434`/`qwen3.5:4b`）
- **TTS（核心链路，不是 Mod）**：`live2d-ai.toml` 的 `[tts]` 段 →
  `crates/live2d-ai-runtime/src/settings.rs` → `runtime/src/conversation/worker.rs`。
  论证见 `docs/architecture/tts-is-core.md`。
- **Mod 注册中心**：`crates/live2d-ai-mod-system/src/lib.rs`
- **Mod 管理 API**：`crates/live2d-ai-desktop/src/web_api/mods_routes.rs`
  - `POST /api/v1/mods/{id}/{action}`（action ∈ enable/disable/restart）
  - enable/disable/restart 仅操作 `enabled` 标志 + 生命周期，不透传 body
- **ModRegistry**：`crates/live2d-ai-desktop/src/mod_registry.rs`
  - `parse_mod_config` 从 `manifest["mods.<id>"]` 读取 `enabled` / `config`
  - `enable(id)` → `start_one(id)` → `factory.create(config)` → `runtime.start()`
  - `spawn_and_probe` 硬编码 `127.0.0.1:{port}` 探活
- **配置路径**：`crates/live2d-ai-desktop/src/web_api/cli_entry.rs::config_path_for_web()`
  - 优先 `./live2d-ai.toml`，否则 `$HOME/.config/live2d-ai/live2d-ai.toml`
- **设置 PATCH**：`crates/live2d-ai-desktop/src/web_api/settings_routes/mod.rs`
  - `PATCH /api/v1/settings` 用于手动编辑 base_url/model/voice 等
- **Headless 部署**：`docs/headless-deploy.md`
- **外部输入**：`docs/external-input.md`
