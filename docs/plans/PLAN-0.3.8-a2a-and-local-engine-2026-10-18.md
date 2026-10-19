# 执行单：隐藏 local-engine，再做 0.3.8 A2A（2026-10-18）

> **2026-10-18 已验收。** 隐藏 `ac8a3e9`（无 tag）。A2A `1f43467` / `240f008` / `16bbfad`，附注 tag `v0.3.8-rc.1`…`rc.3`。HEAD `16bbfad`，未 push。`code-stats --check` 仍红在既有的 `live2d_stage.dart` 844 行，本块未改。真远端 agent、真 Ollama、浏览器未验。

> 交给执行 agent。本单就是范围。不要再调研产品取舍，不要扩大。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。HEAD 写单时是 `121e8ca`（`v0.3.7-rc.3`，未 push）。
> 两件事分开提交。隐藏不是一个版本号，不打 tag。`0.3.8` 只做 A2A。

## 0. 裁决

**`local-engine` 隐藏，不删除。** 它仍是 0.3.4 的第六个工厂，缺省停用。设置 → 扩展不再画这张卡。注册、`GET /api/v1/mods`、`mods.json` 启停都留着。

它只做一件事：按用户填写的 `program` + `args_json`（JSON 字符串数组）拉起一个外部进程，不用 shell，并能停掉。四个设置是 `program` / `args_json` / `workdir` / `launch_mode`（缺省 `on_apply`）。没有 `base_url`，不探活，不写 `[tts]` / `[llm]`，`apply_settings` 恒为 0。出厂出声仍是 `local-tts-melo`。对话的云端 / 本地仍是 `[llm].mode` + `base_url`（0.3.7）。第一次启用走 `StartCause::Enable`，缺省 `on_apply` 不会拉起进程，宿主状态仍可能是「运行中」而 `state_json.running=false`。这是既有语义，本单不改。

**`0.3.8` 是另一块功能：把当前这一路 `[llm]` 暴露成 A2A agent。** 不并进 `0.4.0`（Windows）或 `0.4.1`（桌面透传）。不把隐藏算进 `0.3.8`。不另开清债版本。`rc.z` 由执行 agent 按代码拆；最后一个 `rc.(z+1)` 只清这一块。每个 rc：一次本地提交 + 一个本地附注 tag，不 push。`Cargo.toml` 仍是 `0.3.1`，`pubspec.yaml` 仍是 `0.3.1+22`。

## 1. 禁止

- 不注销 `local-engine`，不改 `mod_count_is_six`，不改 `AVAILABLE_MOD_FACTORIES`，不删第二仓 crate，不降 desktop 依赖棘轮（实测 24，`PLAN_DESKTOP_DEPS` 仍是 22）。
- 不改 `.env`、`mods.json`、`live2d-ai.toml`。不改 `launch_mode` 缺省，不改 Enable / Apply 的拉起条件。
- 不把 A2A 接进 `supervisor.say`、句子装配、TTS、口型、`action_cue`。皮套回合的 `/chat/completions` 请求体不加 `tools`。工具结果不写进台词。
- 不新增第二套对话模型，不内置推理运行时，不挂回 `local-llm`、CosyVoice `local-tts`、`pet-desktop`、`wallpaper`。不接 `action_tx`。
- 不实现 `message/stream`、取消、列表、推送、文件 part、data part、gRPC、HTTP+JSON REST。这些方法回 JSON-RPC 错误，不要半截 SSE。
- A2A 响应、agent card、日志都不出现密钥值。不新建 Mod 密钥代理（S2 保持原样）。
- 不提高 `DOCS_BUDGET_LINES`。不改 `CHANGELOG.md`、`docs/audit/**`、`AGENTS.md` 历史段。
- 不 push。不移动 `v0.2.7` / `v0.2.6-rc.1`。不重建远端 `v0.3.1`。不提交 `official_live2d_samples.7z`。
- 不要 `git add -A`。写单时工作树里已有未提交的版本口径（`CURRENT.md`、阶段单、`docs/README.md`、`AGENTS.md` 现行句、`ANALYSIS-0.3.8-before-0.4.0-2026-10-18.md`）。那些留在工作树，不收进本单的提交，也不要还原。
- 不点火，不杀 `ignite.sh`。`llm.rs` 599 行、`web_api/mod.rs` 700 行，都不要把实现正文加进去。新文件 ≤500。`>500` 文件数不增加，`>1000` 保持 0。

## 2. 先做：界面隐藏（无版本号）

一次本地提交，不打 tag，不改版本串。提交说明写明只隐藏界面。

落点：`shell/flutter/lib/settings/sections/dev_tools_mods.dart` 的 `ModsSection`。在 `for (final ModInfo m in mods)` 之前滤掉 id `local-engine`。常量只用 ASCII：`kModsHiddenFromShell = {'local-engine'}`。不要新的中文文案。`dev_mode` 也隐藏（开发模式会把空名字画成 id）。

空列表只在接口真的返回空时显示「没有注册任何 Mod」。接口非空、可见列表为空时，不显示这句。

不要在 API 客户端里过滤。`GET /api/v1/mods` 仍返回这个 id。

测试：在 `shell/flutter/test/mods_section_test.dart` 导演卡那组旁边加一条。用 `registeredModIds()` 灌进 `ModsSection`，`name` 留空。断言 `find.text('local-engine')` 为 0，`find.text('external-input')` 仍在（空名字会回落成 id）。`mod_state_surface_test.dart` 与 `test/support/registered_mods.dart` 继续认为工厂里有 `local-engine`，工厂数仍是 6。

检查：`cd shell/flutter && flutter analyze && flutter test`。然后

```bash
cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn
cd ../.. && ./scripts/prune_web_artifacts.sh && ./scripts/prune_web_artifacts.sh --check
```

`ignite.sh --build` 在 `build/web/index.html` 已存在时不会重编 Flutter，所以必须先跑上面的构建。服务若已在 18080，构建完刷新 `/app/` 即可，不要重启。Rust 这一步零改动，不跑整库 `cargo test`。

## 3. 然后做：0.3.8 A2A

对外只有这一路已配置的 `[llm]`（含 `mode = cloud | local`）。`mode` 仍不参与地址解析，失败不回退到另一个模式。调用方是外部 agent。皮套自己的对话回合不调工具。

协议钉在 A2A 的 JSON-RPC 2.0 绑定。发现文档是 `GET /.well-known/agent-card.json`（不是旧的 `agent.json`）。卡片在请求时生成：`version` 用 Cargo 的 `0.3.1`；`url` / `supportedInterfaces` 只指向本服务的 `POST /api/v1/a2a`，不写上游 `base_url`，不写密钥，不写 `.env`。`streaming` 与推送都是 false。一个 skill，id `llm-chat`，说明是「把文本交给当前已配置的对话模型，回文本」。

`POST /api/v1/a2a`：`Content-Type: application/json`，与其它 mutating 路由同一套 loopback + Origin 校验。实现放新文件 `crates/live2d-ai-desktop/src/web_api/a2a_routes.rs`。`web_api/mod.rs` 只加与 `external_routes` 相同形态的几行转发。

本版只收两个方法：

- `message/send`：只要一条 `role=user` 的消息，parts 只收 `kind=text`。拼成一段文本。空文本回 JSON-RPC invalid params，不打上游。
- `tasks/get`：按本进程内存里的 task id 取回。上限 32 条，不落盘。未知 id 回 JSON-RPC 错误。进程重启后任务消失。

其它方法回 JSON-RPC method-not-found 或 unsupported，不打上游。

上游复用现有 `OpenAiClient::chat_stream`（`stream=true`）。收集 `TextDelta`，丢掉 `ReasoningDelta`（思考不进返回正文）。不要往 `llm.rs` 的 `chat_request_body` 加 `tools`。收集逻辑若需要新函数，放新文件，例如 `crates/live2d-ai-runtime/src/llm_collect.rs`。成功时 JSON-RPC result 是一个 `completed` Task，里面一条 agent 消息、一个 text part，正文等于收集到的正文。上游失败：Task `failed` 或 JSON-RPC error；响应体与日志都不含密钥，不把上游 body 原样回出去。401/403 不带 key。

这条路径的函数签名里不要出现 `supervisor`、`say`、句子装配或 cue。用注入的上游调用做测试，证明一次 `message/send` 只打这一下 LLM。另加一条：`chat_request_body` 的 JSON 没有 `tools` 键（断言放新测试文件，不把 `llm.rs` 再撑大）。

真远端 agent 与真 Ollama 标「未验」。测试用本机 mock HTTP，不要求本机有 Ollama。

建议拆法（中间步可以合并，最后一步必须是清理）：

- `v0.3.8-rc.1`：agent card + `message/send` 文本进、文本出。与 `say` 隔离。
- `v0.3.8-rc.2`：`tasks/get`、拒绝范围外方法、密钥不回显、local 失败不回退 cloud。
- `v0.3.8-rc.3`：只清理。拆超长文件、补测试、把 `CURRENT.md` 里「0.3.8 未开始」改成已落地的两句、短发布说明 `docs/releases/v0.3.8-rc.3.md`。不加方法，不加界面。

每个 rc 换提交。tag 用附注 tag，例如 `git tag -a v0.3.8-rc.1 -m "0.3.8-rc.1: A2A agent card and message/send"`。不 push。

0.3.8 的检查（每个含 Rust 的 rc 都跑）：

```bash
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio
cargo run -p xtask -- code-stats --check
```

`live2d-ai-desktop` 没有 lib target，相关测试走 `--bin live2d-ai-desktop`。本块不改 Dart、不改 wasm 时，不必重编 Flutter，也不必重编 wasm。浏览器不必开。交付里写「浏览器未看，本块无界面」。

## 4. 交付

写明：隐藏那次提交的 hash（无 tag）；每个 `v0.3.8-rc.z` 的 hash 与 tag；`flutter analyze` / `flutter test` / 上述 cargo 的退出码；`code-stats --check` 的 docs 行数与 `>500` 文件数；Flutter web 是否重建。真 Ollama 与真远端 agent 写「未验」。不要把隐藏的提交打成 `0.3.8`。
