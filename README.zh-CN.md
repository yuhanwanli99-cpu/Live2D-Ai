# Live2D-Ai

**v0.1.0-rc.4** — 通用、最小的 **Live2D 形象 ↔ AI 对话** 接入平台。

Live2D-Ai 只打磨一条核心链路，其余能力放在 Mod 边界之后：

**文本 → LLM（纯对话）→ TTS → 口型 → Live2D 渲染 + Web UI**

**不捆绑、不绑定**任何角色、皮套或模型；模型由你合法取得后自行导入。桌宠、外部输入、本地推理、导演序列等增强能力走 Mod，失败只关掉该 Mod，不影响主链路。

> English (default): [README.md](./README.md)

---

## 功能

### 核心链路
- **LLM 纯对话** — OpenAI 兼容 `/chat/completions`（SSE）。请求体不含 tools / function calling（有回归测试）。
- **TTS 属于核心** — OpenAI 兼容 `/audio/speech`；端点来自 `live2d-ai.toml` 的 `[tts]`。
- **一句一单元** — 按真实句读切分；口型由逐句音频与音量包络驱动。
- **纯 reducer**（`live2d-ai-core`）— epoch 闸门 + 闩锁，无 IO、无 async。
- **Supervisor** — 独立线程 + 事件循环推进 turn。

### Live2D 与界面
- **渲染** — [Ayagami](https://github.com/AyagamiDev/ayagami)（MIT OR Apache-2.0）+ wgpu；Rust 基线**不依赖** Live2D Cubism 专有 SDK。
- **Web UI** — 主入口（`--web`）；舞台 + 对话区、设置面板，离线友好构建门禁。
- **WASM 渲染** — `/render` 与 `/models/*`（`l2d-wasm-demo`）。

### Mod
- trait 注册中心（`live2d-ai-mod-system`）：运行时 enable / disable / restart。
- 已编译进二进制的 Mod：外部输入、导演、桌宠骨架、本地 LLM 等。
- Mod 失败仅 disable 自身，主链路继续。

### 本基线平台
- 主力：**Linux / WSL2**。
- Android、原生 Windows 桌面不在 `0.1.0` RC 线范围内。

---

## 快速开始

| 工具 | 说明 |
|---|---|
| Rust | MSRV **1.92** |
| `wasm32-unknown-unknown` | `rustup target add wasm32-unknown-unknown` |
| trunk | WASM 构建（`cargo install trunk`）|

```bash
cp live2d-ai.toml.example live2d-ai.toml   # OpenAI 兼容 LLM/TTS
cargo build --release
./target/release/live2d-ai-desktop --web --http-port 18080
# 浏览器打开 http://localhost:18080/
```

Live2D 模型请放到本地 `assets/models/`（见 `assets/models/README.md`）。该目录下的模型二进制**不会**进入 git。

### 常用 CLI
```bash
--web [--http-port P]   # Web UI（主入口）
--chat                  # 终端对话闭环
--model-smoke [P]       # 渲染冒烟
--audio-smoke           # 音频冒烟
--benchmark [P]         # 渲染 benchmark
```

### 点火（WSL2 → Windows 浏览器）

**开发与「服务进程」都在 WSL2 侧**；Windows 只负责开浏览器（不跑二进制、不编 Flutter）。

```bash
./scripts/ignite.sh            # 预检 + 启动，默认端口 18080
./scripts/ignite.sh --build    # 先重建 Rust + Flutter Web 再启动
./scripts/ignite.sh --check    # 对**已启动**的服务做点火体检
```

然后打开 <http://127.0.0.1:18080/app/>（`/` 会 302 跳到 `/app/`）。
前端产物的单一真源是 `shell/flutter/build/web`；`LIVE2D_AI_FLUTTER_WEB_DIR`
请保持 **WSL 路径**（脚本已替你设好）。

`--check` 会断言 `GET /` = 302 → `/app/`、`GET /app/` = 200，以及服务吐出的
`index.html` / `main.dart.js` **不含** `gstatic.com/flutter-canvaskit` 引用
（断网红线——走了 CDN 就是断网白屏）。

### 测试
```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 架构（crates）

```
crates/
  l2d                      Live2D 渲染（Ayagami + wgpu）
  live2d-ai-core           纯 reducer 状态机
  live2d-ai-runtime        LLM / TTS / 对话 / 设置
  live2d-ai-desktop        Supervisor + Web API + 桌面壳
  l2d-wasm-demo            Web WASM 渲染
  live2d-ai-mod-system     Mod trait / 注册
  live2d-ai-mod-*          可选 Mod
```

文档索引：[docs/README.md](./docs/README.md)

---

## 许可与资产

- **项目自有代码：** [AGPL-3.0-only](LICENSE) © 2026 Sakura Motion Project  
  可选商业许可：[COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md)
- **第三方：** 见 [NOTICE](NOTICE)、[CREDITS.md](CREDITS.md)
- **Live2D 模型 / 贴图：** 不受 AGPL 覆盖；须合法取得并自行导入。本仓库**不**再分发 `.moc3` 与贴图二进制。范围说明：[LICENSE-SCOPE.md](LICENSE-SCOPE.md)

渲染栈使用开源 Ayagami，而非 Live2D Inc. 的专有 Cubism Core SDK。若你自行加入第三方 Cubism 二进制，它们仍受 Live2D EULA 约束，且不属于本项目许可范围。
