# Live2D-Ai

**v0.2.4-rc.1** — 通用、最小的 **Live2D 形象 ↔ AI 对话** 接入平台。

Live2D-Ai 只打磨一条核心链路，其余能力放在 Mod 边界之后：

**文本 → LLM（纯对话）→ TTS → 口型 → Live2D 渲染 + Web UI**

**不绑定**任何单一模型；出厂带白模型（条约见 [`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md)，**不是**作者授权再分发），其余模型由你合法取得后自行导入。外部输入、语音输入、会话记忆、导演序列等增强能力走 Mod，失败只关掉该 Mod，不影响主链路。

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
- 已编译进二进制的 Mod（**6 个注册**）：`external-input`、`persona`、`voice-input`、
  `memory`、`director`、`local-tts-melo`。**缺省启用 `external-input` 与
  `local-tts-melo`**（后者开机拉起仓库内 MeloTTS；见编译期
  `cli_entry::default_mods_manifest`），其余四个都要手动开
  （`memory` 会写 `persona.system_prompt`，必须由用户明确打开）。
- `local-tts`（CosyVoice3）**已封存、未注册**：crate 与 `engine/` 留在树上（可编译可测），
  **未适配前不要挂回**。
- `director`（2026-09-14 起的**决策/按句 cue**版）：规则层按**用户输入**判情绪 / 意图选动作包，
  经 host `ModServices.cues` 广播 WS `action_cue`（**唯一驱动舞台**；中性轮发
  `preset_id=="none"` 撤销哨兵），本机 `mods.json` 启用后**确实驱动动作**；
  `latest.preset_id` 仅供面板只读（前端拉取驱动的通道已退役，D12）。异步第二路 LLM
  （priority 40）**默认关**，其真实 HTTP 客户端已接线（`staging_http.rs`）。
  现状真源：`AGENTS.md` §「动作与表演的现行状态（2026-09 实测）」③。
- `wallpaper` 与 `pet-desktop` **已封存（ARCHIVED，本波不做）**：**不再注册、不再
  编译进 binary**；crate 已于 2026-10-01 **物理删除**（只存在于 tag
  `checkpoint/pre-d1-dormant`），**禁止挂回**。
  理由与恢复条件见 [`docs/architecture/ARCHIVED-mods.md`](docs/architecture/ARCHIVED-mods.md)。
  用户手动的舞台/壳背景能力（`DisplayPrefs`）**保留**——与被封存的 wallpaper **Mod** 是两回事。
- `local-llm` **已废除启动**（`0.2.0-rc.1` 移出注册表；crate 已于 2026-10-01 删除）。
  rc.2 删除的那个**动作驱动** director 的归档分支 `archive/action-layer-p6` **已不存在**
  （2026-10-06 复核：本地 / 远端 / 历史 bundle 里都没有）；被删内容可从 `main` 历史取回：
  `git show 98469df^:crates/live2d-ai-mod-director/src/lib.rs`。现在的 `director` 是
  2026-09-14 同名**不同职责**的决策 / 按句 cue 版（见上一条）。
- 语音输入：本机 ASR **sidecar**（你自己起的进程）把转写 POST 到
  `POST /api/v1/voice/transcript`；**ASR 不进 binary**。
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
(cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build
```

`flutter` 是 `~/flutter` 里的 Flutter SDK（`scripts/ignite.sh` 会把 `$HOME/flutter/bin` 放进 `PATH`）。
`./scripts/ignite.sh --build` 编的是**调试版后端**并启动；`shell/flutter/build/web/index.html`
已存在时**不会重编 Flutter**，所以第一行必须先跑。最后一行会 `exec` 进服务、不退出——
另开终端跑 `./scripts/ignite.sh --check`，浏览器打开 <http://127.0.0.1:18080/app/>。

出厂带白（bai）模型，克隆后即可渲染——资产说明见 [`assets/models/README.md`](assets/models/README.md)，
条约见 [`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md)
（作者条约，**不是**授权再分发）。

### 常用 CLI
```bash
--web [--http-port P]   # Web UI（主入口）
--audio-smoke           # 音频冒烟
```

### 点火（WSL2 → Windows 浏览器）

**开发与「服务进程」都在 WSL2 侧**；Windows 只负责开浏览器（不跑二进制、不编 Flutter）。

标准构建 + 启动（仓库根执行，顺序不可换）：

```bash
(cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build
```

`flutter` 是 `~/flutter` 里的 SDK。`./scripts/ignite.sh --build` 编的是**调试版后端**并启动；
`shell/flutter/build/web/index.html` 已存在时**不会重编 Flutter**——所以第一行必须先跑。
最后一行 `exec` 进服务、不退出；另开终端做点火体检：

```bash
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
- **Live2D 模型 / 贴图：** 不受 AGPL 覆盖；须合法取得并自行导入。出厂白模型**渲染必需的那几份已跟踪**（`bai.moc3`、`texture_00_4096.png`），条约见 [`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md)——**不是**作者授权再分发。范围说明：[LICENSE-SCOPE.md](LICENSE-SCOPE.md)

渲染栈使用开源 Ayagami，而非 Live2D Inc. 的专有 Cubism Core SDK。若你自行加入第三方 Cubism 二进制，它们仍受 Live2D EULA 约束，且不属于本项目许可范围。
