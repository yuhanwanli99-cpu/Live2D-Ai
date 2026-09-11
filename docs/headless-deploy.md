# Headless 部署指南

> 本文档介绍如何在 **无显示服务 / 无桌面环境**（Headless）下运行
> `live2d-ai-desktop` 的 Web API 后台，以及在 **Termux**（Android）中部署的特点。
>
> 核心结论：**`--web` 本身已是纯 headless 模式，无需额外标志。**
> 文档末尾附「是否修改代码」结论。

---

## TL;DR

```bash
# 1) 构建
cargo build --release -p live2d-ai-desktop

# 2) 启动 headless Web API 服务（监听 127.0.0.1:18080）
./target/release/live2d-ai-desktop --web

# 3) 如需在浏览器中播放 TTS 音频（headless 环境通常无 ALSA/Pulse 设备）：
LIVE2D_AI_WS_AUDIO=1 ./target/release/live2d-ai-desktop --web

# 4) 客户端：同一局域网内浏览器打开
http://<你的IP>:18080/
```

---

## 1. 什么是「headless 模式」？

**Headful（有桌面）模式** 依赖 `winit` + `wgpu` 创建渲染窗口，`ksni` 提供系统托盘，
`cpal` 输出声音到物理声卡。典型启动参数为 `--pet-mode` / `--model-smoke` /
`--window-smoke` 等。

**Headless（无桌面）模式** 仅启动 **HTTP + WebSocket 控制面**，不创建任何窗口、
不初始化 `winit`/`wgpu`/`egui`/`ksni` 托盘。模型渲染通过浏览器端 WASM
(`/render` 路径) 承载；TTS 音频通过 WebSocket 浏览器播放。

**启动方式唯一：** `--web`

```
live2d-ai-desktop --web [--http-port P] [--dev-mode]
```

### `--web` 是如何「纯 headless」的？

在 `cli/entry.rs::run_web_mode()` 中：

1. **不调用 `backend::create_backend`** —— 也就是从不创建
   `WinitWgpuBackend` / `winit EventLoop` / `wgpu surface`。
2. **不调用 `print_info()` 中的 GUI 探测** —— `main.rs` 的
   `Command::Web` 分发直接进入 `run_web_mode`，跳过
   `LinuxSessionHint` / `RuntimeCapabilities` 探测。
3. **仅使用 `tiny_http`（HTTP） + `tungstenite`（WebSocket）** 作为网络层。
4. **音频**：`build_web_supervisor` 尝试 `AudioOutputFacade::open_default()`
   获取 cpal 输出设备；在 headless 环境下通常失败，代码路径
   `Err(e) if e.is_environment() => None` 会进入 **dry-run** ——
   supervisor 照常运行，TTS PCM 不输出到声卡。

> **结论：`--web` 无需 `--no-desktop` / `--skip-gui` 等额外标志，
> 它本身就是纯 HTTP+WS 服务。**

---

## 2. 环境变量

| 变量名 | 作用 | Headless 是否必须 |
|---|---|---|
| `LIVE2D_AI_WS_AUDIO=1` | 将 TTS PCM 推送到 WebSocket 广播，由浏览器前端播放。**Headless 环境无 ALSA/Pulse 设备时必须开启**（否则音频通道存在但无法播放）。 | **是** |
| `LIVE2D_AI_ALLOW_NO_ORIGIN=1` | 绕过 WebSocket Origin 同源校验，允许来自任意 Host 的 WS 连接。Loopback 监听下默认已关闭；如需远程调试可开启。 | 否 |
| `EXTERNAL_INPUT_TOKEN` | `external-input` Mod 的鉴权 Token（如启用该 Mod）。 | 否 |
| `RUST_LOG=info` | 日志级别（默认 `info`，可设为 `debug`/`trace`）。 | 否 |

### 音频路由说明

Headless 部署时，服务端 `cpal` 输出设备通常不可用（服务器无声卡、Docker
无设备映射、Termux 无 ALSA）。此时应：

1. **不要**尝试启动本地声卡输出（默认 dry-run 已处理）。
2. **必须**设置 `LIVE2D_AI_WS_AUDIO=1`，让 TTS 音频通过 WebSocket
   广播到浏览器，浏览器使用 Web Audio API 播放。

浏览器端收到音频帧的数据格式为 base64 编码的 `s16le` PCM（采样率/声道
从 `AudioSpec` 透传），由前端 `/static/app.js` 中的播放器消费。

---

## 3. Termux 部署（Android）

Termux 是 Android 上的 Linux 模拟器/环境。`std::env::consts::OS` 在
Termux 下为 `"android"`。

### 3.1 平台兼容性说明

| 子系统 | Termux 可用性 | 说明 |
|---|---|---|
| `--web` (HTTP+WS) | ✅ | 纯 Rust / `tiny_http` / `tungstenite`，与 glibc/arm64 兼容。 |
| `winit` / `wgpu` | ❌ | Android 无 X11/Wayland 显示服务；WindowManager API 不在本工程适配范围。 |
| `ksni` (系统托盘) | ❌ | D-Bus / StatusNotifier 协议在 Android 不可用。 |
| `cpal` 声卡输出 | ❌ | Termux 无 ALSA/PulseAudio 设备（需额外安装 `pulseaudio` + 图形桌面）。 |
| Live2D 模型渲染 (`/render`) | ✅ (浏览器) | 通过 Web WASM，在浏览器端渲染。 |

> **架构隔离确认**：`winit`/`wgpu`/`ksni` 仅在 `backend/`、`app/`、`tray/`
> 等模块中初始化，且**只在 `--pet-mode`/`--model-smoke`/`--window-smoke`/`
> --chat`(桌宠模式) 等 GUI 模式下被调用**。`--web` 模式从不触达这些模块，
> 因此 `cargo build` 在 Termux/Android target 上不受 GUI 链路影响。

### 3.2 部署选项

#### 选项 A：Termux 内直接构建（推荐）

Termux 官方源提供 `rust` 包，包含 `cargo` + `rustup`：

```bash
# 1) 安装构建工具
pkg update && pkg install -y rust git

# 2) 克隆仓库
git clone https://github.com/your-org/Live2D-Ai.git
cd Live2D-Ai

# 3) 在 Termux 内原生编译 aarch64
cargo build --release -p live2d-ai-desktop

# 4) 启动（监听 127.0.0.1）
LIVE2D_AI_WS_AUDIO=1 ./target/release/live2d-ai-desktop --web --http-port 18080
```

> Termux 默认以 **aarch64** (arm64) 运行，`cargo` 会自动匹配当前架构。
> 无需交叉编译。构建产物位于 `target/release/`。

#### 选项 B：交叉编译 (aarch64-linux-android)

如需在桌面 Linux 上交叉编译 Android 二进制：

```bash
# 1) 安装 Android NDK + rustup target
pkg install -y ndk-sysroot  # 或手动下载 NDK
rustup target add aarch64-linux-android

# 2) 设置交叉编译环境
export ANDROID_NDK_HOME=$PREFIX/opt/android-ndk  # Termux 路径示例
export CC_aarch64_linux_android=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/\
linux-x86_64/bin/aarch64-linux-android21-clang
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$CC_aarch64_linux_android

# 3) 交叉编译
cargo build --release --target aarch64-linux-android -p live2d-ai-desktop

# 4) 将二进制 push 回 Termux
# scp 或 adb 传送，或直接在 Termux 内运行
```

> ⚠️ 交叉编译需要 Android NDK。**选项 A（Termux 内直接 `cargo build`）更简单，
> 推荐。**

#### 选项 C：cargo-apk（不推荐）

`cargo-apk` 用于构建 Android APK，需要 Gradle + Android SDK，
本工程**不是 Android 应用**（AGPL 桌宠服务器，无 Android UI），
因此不适用。

### 3.3 Termux 网络访问

- **Loopback**：`--web` 默认监听 `127.0.0.1`，仅本设备访问。在 Termux 内，
  浏览器需与 `termux-app` 同一设备。

- **局域网访问**：如需局域网设备访问，需修改监听地址。**当前 `--web`
  强制 loopback**（P0-3 安全红线），以保障 API 仅本地访问。
  如确需远程访问，请通过 **SSH 端口转发** 或 **反向代理**：

  ```bash
  # SSH 端口转发（从远程机器）
  ssh -R 18080:127.0.0.1:18080 user@你的手机IP

  # 或在手机局域网内运行反向代理（如 nginx）
  ```

---

## 4. 客户端访问

部署完成后，在浏览器中打开：

```
http://<host>:18080/
```

| 场景 | host | 说明 |
|---|---|---|
| 同设备 Termux | `127.0.0.1` | 直接访问 loopback |
| 局域网 | `<手机局域网 IP>` | 需 SSH 转发/反向代理（loopback 不直接暴露） |
| WSL | `127.0.0.1` | WSL2 loopback 自动转发 |

浏览器端功能：

- **Live2D 模型渲染**：通过 `/render` 路径的 WASM 渲染页面，浏览器端
  `l2d-wasm-demo` 负责模型加载 + 实时动画。
- **TTS 音频播放**：当 `LIVE2D_AI_WS_AUDIO=1` 时，浏览器订阅
  `/ws/runtime` WebSocket，收到音频帧后通过 Web Audio API 播放。
- **对话**：`POST /api/v1/chat` → LLM → TTS → WS 音频广播 → 浏
  览器播放 → Live2D 口型/表情动画同步。

---

## 5. 局限性

| 能力 | Headless 支持 | 说明 |
|---|---|---|
| Web API (HTTP) | ✅ | `--web` 提供完整 REST 接口 |
| WebSocket 事件流 | ✅ | `/ws/runtime` / `/ws/state` |
| Live2D 模型渲染 | ✅ (浏览器) | WASM `/render`，非服务端 wgpu |
| TTS 音频播放 | ✅ (浏览器) | `LIVE2D_AI_WS_AUDIO=1` +浏览器 Web Audio |
| 本地声卡输出 | ❌ | Headless 无 ALSA/Pulse，dry-run |
| 桌面窗口壳 | ❌ | P2 F4/G1 才提供，依赖 winit+wgpu |
| 系统托盘 | ❌ | ksni 在 headless/Android 不可用 |
| 桌宠点击穿透 | ❌ | 依赖窗口管理器 |

> **P2 G2 (本次任务)** 仅要求 headless 运行能力 + Termux 支持。
> 桌面窗口壳 (F4/G1) 仍需图形环境，属于 P3 阶段。

---

## 6. 快速验证

```bash
# 验证 1：Web API 服务正常启动（无需配置文件）
LIVE2D_AI_WS_AUDIO=1 ./target/release/live2d-ai-desktop --web --dev-mode
# 预期输出：web: 已监听 127.0.0.1:18080

# 验证 2：API 状态端点
curl -s http://127.0.0.1:18080/api/v1/app/status | python3 -m json.tool
# 预期：{"running":true,"configured":false,"has_api_key":false,...}

# 验证 3：能力端点
curl -s http://127.0.0.1:18080/api/v1/app/capabilities
```

---

## 7. 生产部署建议

```bash
# 使用 systemd 服务（示例）
cat > /etc/systemd/system/live2d-ai-web.service <<'EOF'
[Unit]
Description=Live2D-Ai Web API (headless)
After=network.target

[Service]
Type=simple
ExecStart=/opt/live2d-ai/live2d-ai-desktop --web --http-port 18080
Environment=LIVE2D_AI_WS_AUDIO=1
Environment=RUST_LOG=info
WorkingDirectory=/opt/live2d-ai
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

systemctl enable --now live2d-ai-web
```

---

## 代码变更结论

> **`--web` 已纯 headless，未修改任何代码。**

详细说明：

1. **`cli_entry.rs` — 未修改**：`run_web_mode()` 仅调用
   `start_server()`（`tiny_http` HTTP + `tungstenite` WS）和
   `build_web_supervisor()`（cpal 音频，无设备时 graceful dry-run）。
   从不初始化 `winit` / `wgpu` / `egui` / `ksni` 托盘。

2. **`main.rs` — 未修改**：`Command::Web` 分发直接进入
   `cli_entry::run_web_mode`，不经过 `print_info()` /
   `backend::create_backend` 等 GUI 探测。

3. **`platform/` — 未修改**：Linux 会话线索探测
   (`LinuxSessionHint` / `RuntimeCapabilities`) 仅在 `main.rs::print_info`
   使用，`--web` 模式不调用。

4. **音频 — 未修改**：`build_web_supervisor` 已实现
   `Err(e) if e.is_environment() => None`（dry-run）降级，
   且已原生支持 `LIVE2D_AI_WS_AUDIO=1` WebSocket 音频广播。

5. **环境变量 — 已存在**：`LIVE2D_AI_WS_AUDIO` 在 `cli_entry.rs`
   行 199 已读取，无需新增代码。

> 任务要求「仅加 headless 启动路径/flag，若已有则跳过」：
> `--web` 本身就是 headless 启动路径，无需新增 `--no-desktop`
> 或 `--headless` 等标志。
