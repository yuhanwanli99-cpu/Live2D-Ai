# Live2D-Ai

**v0.3.1** — 本地优先的 **Live2D 皮套 ↔ AI 对话** 接入服务。`Cargo.toml` 的版本串是 **0.3.1**。

产品核心只有三块，其余都在 Mod 边界后面：

1. **一路表演 LLM**（`[llm]`，纯对话）
2. **二路导演**（按句清洗，这一句上屏、送 TTS，并按原文出动作）
3. **一个 Web 前端**（渲染、动作、设置、对话）

**文本 → LLM（纯对话）→ TTS → 口型 → Live2D 渲染 + Web UI**

出厂不设人设。不绑定任何单一模型；出厂带白模型（条约见 [`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md)，**不是**作者授权再分发）。其余模型由你合法取得后自行导入，只留在本机模型库，不写入仓库。

克隆主仓、填好 `[llm]` / `[tts]` 就能编译并跑，不需要旁边再克隆 Mod。云端 TTS 也不需要本机 Melo。

> English (default): [README.md](./README.md)
> 现行契约：[docs/architecture/CURRENT.md](docs/architecture/CURRENT.md)

---

## 功能

### 核心链路
- **LLM 纯对话** — OpenAI 兼容 `/chat/completions`（SSE）。请求体不含 tools / function calling（有回归测试）。`[llm].mode` 只有 `cloud | local`。
- **TTS 属于核心** — OpenAI 兼容 `/audio/speech`；端点来自 `live2d-ai.toml` 的 `[tts]`，可以是云端 API。`[tts].mode` 缺省 `local`（本机 Melo 地址 `http://127.0.0.1:8091/v1`）。
- **一句一单元** — 按真实句读切分；口型由逐句音频与音量包络驱动。
- **导演在核心里**，常开，没有启用开关。动作走 `action_cue` → 渲染面 `preset`。`action_tx` 仍休眠。
- **纯 reducer**（`live2d-ai-core`）— epoch 闸门 + 闩锁，无 IO、无 async。
- **Supervisor** — 独立线程 + 事件循环推进 turn。

### Live2D 与界面
- **渲染** — [Ayagami](https://github.com/AyagamiDev/ayagami)（MIT OR Apache-2.0）+ wgpu；Rust 基线**不依赖** Live2D Cubism 专有 SDK。
- **Web UI** — 主入口（`--web`）。`GET /` 与 `GET /index.html` 返回**启动器页**（200）：勾选本次确认哪些已注册 Mod、预启动开或关，点「进入」后再到现有 `/app/`。Flutter 产物是 `shell/flutter/build/web`。
- **WASM 渲染** — `/render` 与 `/models/*`（`l2d-wasm-demo`）。
- **0.3.11 已落地**：启动前这一页只有三类操作——勾选本次确认哪些**已编进二进制的** Mod、预启动开或关（缺省关）、以及由此算出的三个数字（已注册 / 本次确认 / 本次拉起）。点「进入」后播放加载动画，按勾选调用现有启停接口，再对当前 `[llm]` / `[tts]` 地址各探测一次（只报告，不改档），然后进入 `/app/`。自动预启动撤掉，也不再为本地 TTS 空等 600 秒。执行单 [`docs/plans/PLAN-0.3.11-launcher-2026-10-20.md`](docs/plans/PLAN-0.3.11-launcher-2026-10-20.md)。
- **0.3.12 已落地**：启动器只读扫描工作目录 `mods/<id>/mod.toml`，在上一页列出「已发现 / 不可用（原因）」；**只预检清单**——不执行里面的程序、不 spawn，也不把 Mod 编回缺省二进制（工厂仍 0）。点「进入」时文件夹只记下勾选（写 `mods.json`），已编进二进制的 Mod 仍走上一版的启停。执行单 [`docs/plans/PLAN-0.3.12-mod-folder-scan-2026-10-20.md`](docs/plans/PLAN-0.3.12-mod-folder-scan-2026-10-20.md)。

### Mod
- trait 注册中心（`live2d-ai-mod-system`）留在主仓。**缺省构建的工厂数是 0**（`AVAILABLE_MOD_FACTORIES` 为空，`default_mods_manifest` 的 `mods` 是空对象）。`external-input` 与 `local-tts-melo` 都不再缺省启用，也没有被编进缺省二进制。
- 七个实现各自一个公开源码仓库，主仓不依赖它们。没有动态加载器，也没有打包好的二进制 Mod。要加进正在跑的进程，把仓库放到主仓同级，再按 [`docs/architecture/two-repo-mods.md`](docs/architecture/two-repo-mods.md) §4 重新编译。`mods.json` 只能开关**已经编进去**的 id。往 `mods/` 放文件夹、让启动器看见并记住勾选的步骤见 [`docs/mods-user.md`](docs/mods-user.md)：本版不运行文件夹里的程序。

  | 公开仓库 | id |
  | --- | --- |
  | [live2d-ai-mod-external-input](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-external-input) | `external-input` |
  | [live2d-ai-mod-persona](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-persona) | `persona` |
  | [live2d-ai-mod-voice-input](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-voice-input) | `voice-input` |
  | [live2d-ai-mod-memory](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-memory) | `memory` |
  | [live2d-ai-mod-local-tts](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-local-tts) | `local-tts-melo` |
  | [live2d-ai-mod-local-engine](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-local-engine) | `local-engine` |
  | [live2d-ai-mod-a2a](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-a2a) | `a2a` |

- 未挂载时这些路由回 **404**，`error.code = "mod_disabled"`，不打 LLM、不 `say`：`POST /api/v1/external/chat`、`POST /api/v1/voice/transcript`、`GET /.well-known/agent-card.json`、`POST /api/v1/a2a`。
- `local-tts-melo` 只拉起外部 Melo 进程，不写 `[tts]`。权重不在公开仓库里。缺省构建不会自动拉起 Melo。
- `local-engine` 只拉起进程（program / args_json / workdir / launch_mode），不写 `[llm]` / `[tts]`，不探活。设置里不显示这张卡。
- CosyVoice 的 `local-tts`、`local-llm`、`wallpaper`、`pet-desktop` 已删除或封存，**不要挂回**。见 [`docs/architecture/ARCHIVED-mods.md`](docs/architecture/ARCHIVED-mods.md)。用户自己的舞台/壳背景（`DisplayPrefs`）还在。
- 导演 crate 还在主仓，但已退出工厂表，**不要再注册成 Mod**。
- Mod 失败只关掉该 Mod，主链路继续。

### 平台
- 主力：**Linux / WSL2**。服务进程只在这边起；Windows 只开浏览器。
- 原生第二壳已于 2026-10-01 移出构建。`0.4.0`（Windows 上跑同一套 `--web`）和 `0.4.1`（新的桌面透传）都还没开始。

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
已存在时**不会重编 Flutter**，所以第一行必须先跑。

**脚本现在等的是启动器页**：`GET /` 返回 200（启动器页；仍是 302 的旧构建回落等 `GET /app/` = 200）才打印 `http://127.0.0.1:<port>/` 并打开浏览器。**不再等本机 TTS**——旧行为是 `[tts].mode=local` 时等 `GET {tts}/models` 返回 200（最长 600 秒），0.3.11 已删掉。缺省二进制不拉起 Melo；点火也不会清理你自己拉起的 Melo 进程。脚本不退出；体检另开终端跑 `./scripts/ignite.sh --check`。

出厂带白（bai）模型，克隆后即可渲染——资产说明见 [`assets/models/README.md`](assets/models/README.md)，
条约见 [`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md)
（作者条约，**不是**授权再分发）。

### 常用 CLI

主入口是 `scripts/ignite.sh` 拉起的这一条：

```bash
--web [--http-port P]   # Web UI（主入口）
```

原生第二壳已经不在构建里：`--chat` / `--window-smoke` / `--model-smoke` / `--pet-mode` / `--benchmark*` 都没有了。还在的工具项：

```bash
--audio-smoke [--audio-smoke-secs S] [--audio-smoke-silence]   # 音频冒烟
```

### 点火（WSL2 → Windows 浏览器）

**开发与服务进程都在 WSL2**；Windows 只开浏览器（不跑二进制、不编 Flutter）。

标准构建 + 启动（仓库根执行，顺序不可换）：

```bash
(cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build
```

另开终端做点火体检：

```bash
./scripts/ignite.sh --check    # 对已启动的服务做点火体检
```

然后打开 <http://127.0.0.1:18080/>——启动器页。勾选本次确认的 Mod、按需打开预启动，点「进入」后进入现有 `/app/`（直接打开 `/app/` 也不会跳回启动器）。
前端产物的单一真源是 `shell/flutter/build/web`；`LIVE2D_AI_FLUTTER_WEB_DIR`
请保持 **WSL 路径**（脚本已替你设好）。

`--check` 会断言 `GET /` = 200 启动器页（正文含 `live2d-ai-launcher`）、`GET /app/` = 200，
以及启动器正文与服务吐出的 `index.html` / `main.dart.js` / `flutter_bootstrap.js`
**都不含** `gstatic.com/flutter-canvaskit` 引用（断网红线——走了 CDN 就是断网白屏）。

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
  live2d-ai-desktop        Supervisor + Web API
  l2d-wasm-demo            Web WASM 渲染
  live2d-ai-mod-system     Mod trait / 注册
  live2d-ai-mod-director   规则表已迁入核心；不要再注册成 Mod
  live2d-ai-mod-template   新 Mod 脚手架
```

七个 Mod 实现不在这棵树里。文档索引：[docs/README.md](./docs/README.md)

---

## 许可与资产

- **项目自有代码：** [AGPL-3.0-only](LICENSE) © 2026 Sakura Motion Project——项目自有代码的**唯一**公开许可证。照 AGPL 履约即可商业使用，不需要另买许可。
- **想避开 AGPL 的源码义务？** 那不是第二份公开许可证，而是**商业付费 + 单独书面协议**，见 [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md)。
- **第三方：** 见 [NOTICE](NOTICE)、[CREDITS.md](CREDITS.md)
- **Live2D 模型 / 贴图：** 不受 AGPL 覆盖；须合法取得并自行导入。出厂白模型**渲染必需的那几份已跟踪**（`bai.moc3`、`texture_00_4096.png`），条约见 [`assets/models/bai/MODEL_LICENSE.md`](assets/models/bai/MODEL_LICENSE.md)——**不是**作者授权再分发；用户导入的模型只留本机模型库、不进仓库。范围说明：[LICENSE-SCOPE.md](LICENSE-SCOPE.md)

渲染栈使用开源 Ayagami，而非 Live2D Inc. 的专有 Cubism Core SDK。若你自行加入第三方 Cubism 二进制，它们仍受 Live2D EULA 约束，且不属于本项目许可范围。
