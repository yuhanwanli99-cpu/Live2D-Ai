> 历史（2026-10-01 归档，勿当现网）。

# 调度提示词（Orchestrator）：密钥引用单一真源 + 设置逻辑精简

> 交给一个**拥有子 worker 调度与管理能力**的编排 agent。它是编排者，不是执行者。

## 0. 你的角色与目标

你是**编排者（orchestrator）**：你有启动后台子 agent、给它们发消息纠偏、收集回报、
必要时中断重发的能力。你的职责是：
1. 把任务块**原样**分发给子 worker；
2. 控制并发与文件所有权，**禁止两个 worker 同时改同一个文件**；
3. 每个波次收口时**由你亲自**跑集成门禁并独立核验，不采信子 worker 自述；
4. 全部完成后，给维护者一份可直接审查的交付报告（他会再转给审查方逐条复核）。

**任务真源**：`/home/skystar/Live2D-Ai-l1/docs/plans/IMPL-PROMPTS-key-single-source-and-settings-trim.md`。
里面每个 `===== 复制给 Worker Pn =====` 块都自成一体（含公共前置）。分发时用 read 工具读该文件，
把对应块**原样**作为子 worker 的 prompt 主体——不要转述、不要删减、不要把两个块合并。
目标一句话：把密钥引用收敛为单一真源、把 `has_api_key` 收敛为单一语义、拆掉 `clear_api_key` 注入层、
清掉重复实现与过期默认值，全程不破坏主链 LLM→TTS→口型→Live2D。

## 1. 环境事实（先读，别踩）

- 工作区 = `/home/skystar/Live2D-Ai-l1`（git worktree，分支 `mod/l1-product`）。
  另一个 worktree `/home/skystar/Live2D-Ai` 是**另一个分支，绝对不要碰**。
- 该工作树有 **约 126 个已修改文件、8400+ 行未提交改动（WIP）**。因此：
  - **禁止** `git worktree add` / `git checkout` / `git switch` / `git stash` / `git reset` /
    `git commit` / `git clean`。任何会动索引、分支或工作树状态的操作，**先问维护者**。
    所有改动**就地编辑**，不要为并行去开分支或 worktree（会丢掉未提交的 WIP）。
  - **禁止**不带 `--check` 的 `cargo fmt --all`（会把全仓重排成巨型 diff），
    以及仓库级 `dart format`。只允许 `cargo fmt --all -- --check` 这类只读检查。
- 运行中的服务：pid 21114，`--web --http-port 18080`，配置 `/home/skystar/Live2D-Ai-l1/live2d-ai.toml`，
  已热重载到 DeepSeek 直连。**不要重启它**（除非任务明确要求）。
  改 `live2d-ai.toml` / `.env` → 自动热重载；改 `mods.json` → **不会**（不在 file_watcher 监视集）。
- `live2d-ai.toml`、`.env`、`mods.json` 都是 gitignored 的运行时配置：改动不入版本库，只对本工作树生效。
- 并发编译：多个 Rust 子 worker 同时跑 `cargo test` 会在 `target/` 上串行等锁（安全但慢）。
  迭代期让 worker 只跑**聚焦测试**（`cargo test -p <crate>`），完整 `--workspace` 门禁留给你在波次收口时串行跑。

## 2. 波次编排（并发铁律）

### Wave A —— 4 路并行（文件零重叠，可同波次并发）
| 任务 | 独占文件 |
|---|---|
| **P0b** | `live2d-ai.toml`（**独占其所有编辑**）、`mods.json` |
| **P1** | `crates/live2d-ai-runtime/src/settings.rs`、`.../settings/settings_tests.rs`、`crates/live2d-ai-desktop/src/web_api/env_routes.rs` |
| **P3** | `shell/flutter/lib/api/settings_models.dart` 及其测试 |
| **P4** | `crates/live2d-ai-runtime/src/lib.rs`、`crates/live2d-ai-desktop/src/web_api/settings_routes/test_endpoints.rs`、`crates/live2d-ai-desktop/src/web_api/dto.rs` |

### Wave B —— 单路（P2）
`runtime/src/settings/view.rs`、`runtime/src/settings/patch_tests.rs`、
`web_api/settings_routes/mod.rs`、`web_api/dispatch.rs`、
`shell/flutter/lib/settings/sections/llm_section.dart`、`tts_section.dart`。
理由：P2 改 `handle_get`/`apply_and_write` 签名，与 P5 同文件；必须在 Wave A 全部收口后再跑。

### Wave C —— 单路（P5）
`web_api/settings_routes/mod.rs` + `tests.rs`、`runtime/src/settings/patch.rs`、
`settings_models.dart`、`settings_controller.dart`、两个 section、契约文档。

### Wave D —— 单路（P6）
两个 section、`settings_controller.dart`、`settings_models.dart`。**必须在 P5 收口后**（同一批文件）。

**冲突热点**（放行任何任务前，先确认没有别的 worker 正在改这些文件）：
`web_api/settings_routes/mod.rs`、`web_api/dto.rs`、`shell/flutter/lib/api/settings_models.dart`、
`shell/flutter/lib/settings/sections/llm_section.dart`、`tts_section.dart`。

## 3. 子 worker 的调用规范

- 同波次用后台方式**并行启动**；等它们全部 settle 再进入下一波，不要边跑边改下游。
- 每个子 worker 的 prompt = 交接文件里对应块的**原文**（公共前置 + 任务），原样粘贴。
- 回报必须包含四项，缺一不可：**改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题**。
  子 worker 若声称「不触发门禁」，要求它给出可复核证据（例如 `find ... -newermt` 输出），否则不放行。
- 子 worker 上报「应该没问题」「大概通过」= 没通过；要求它贴原始命令输出。
- 纠偏用发消息；跑偏或反复失败 → 中断、把**具体失败点**写清后重发，不要无限重试。
- 子 worker **不得**扩大范围：发现范围外的问题只记录进回报，不动手改。

## 4. 集成与门禁（由你在每个波次收口时亲自跑）

在同一工作树上**串行**执行：
- `cargo test --workspace --all-targets`
- `cargo test --doc --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo run -p xtask -- rust-ratio`（门槛 ≥95%）
- 若该波次改了 `shell/flutter/**`：`cd shell/flutter && flutter analyze && flutter test`，
  **并且必须重建产物**：`cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn`。
  `analyze`/`test` 不是产物——不重建，界面改动在 `/app/` 上根本不生效（`main.dart.js` 里还能搜到已删除的旧字段）。
  重建后复核：产物 mtime 晚于最后一个 `.dart` 改动，且 `grep -c clear_api_key build/web/main.dart.js` == 0。

任一红 → **不要进入下一波**；把红的原始输出发回该波次负责人修好，再重跑**整组**。
逐波记下门禁数字，最终报告要用。

## 5. 独立核验（不采信自述，硬要求；全部完成后逐条自测并贴原始输出）

1. `GET /api/v1/env` 的键名集合 == `[llm]/[tts]/[performance]` 实际声明的 `api_key_env`；
   `PUT /api/v1/env` 只接受这份名单（curl 打一个未声明的键名，期望 400）。
2. `GET /api/v1/settings` 与 `GET /api/v1/app/status` 对同一配置给出**同一个** `has_api_key`；
   构造「声明了键名但 `.env` 无值」的场景，两个端点都必须回 false。
3. `grep -rn "std::env::var" crates/ --include='*.rs'`：只允许 `secrets.rs`（`.env` 路径/回退）、
   纯环境探测（XDG/DISPLAY/HOME 等），以及**明确记账豁免**的两处本地令牌：
   `web_api/external_routes.rs` 的 `EXTERNAL_INPUT_TOKEN`、`web_api/voice_routes.rs` 的 `VOICE_INPUT_TOKEN`。
   除此之外出现即红。（这两处是已知既有问题，维护者已裁决另开任务，**不在本包内改**。）
4. curl PATCH：`{"llm":{"api_key_env":null}}` → 磁盘 toml 该键消失；
   `{"llm":{"api_key_env":"NEW_KEY"}}` → 设为新值。
5. 端到端：`POST /api/v1/chat` + 挂 `/ws/state`，确认 `text_delta`、真实 PCM 音频帧、
   `turn_state=completed`、`error` 帧 0；`POST /api/v1/settings/test/llm` **读 body 的 ok**（不看 HTTP 状态，恒 200）。
6. `grep -rn 18181` → 只剩 `docs/` 下的历史记录；源码与运行时配置为零。

## 6. 失败与边界

- 子 worker 之间冲突 / 门禁反复红 / 需要动本包点名之外的模块 → **停下问维护者**，
  说清「哪个任务、哪个文件、什么现象、试过什么」。不要自行扩大范围。
- **不要为凑绿改测试期望**（除非任务明确要求改语义，且把理由写进报告）。
- 不要顺手做 P0b/P1–P6 之外的重构。
- 不要把密钥明文、`.env` 内容、请求体写进任何报告或日志。

## 7. 最终交付（给维护者，他会转给审查方）

按任务分节，每节包含：
- 改动文件（关键 diff 片段即可，不要贴整文件）；
- 新增/修改的测试名；
- 该波次的完整门禁数字；
- **独立核验 1–6 的原始输出**（命令 + 结果，逐条）；
- 未决问题与已知豁免（例如上面第 3 条的两处令牌）。

审查方会照「声明 → 实测」的方式逐条复核：**凡是自述、没有原始输出的结论都会被退回**。
