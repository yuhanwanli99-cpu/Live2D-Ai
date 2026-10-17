# 两仓：核心与 Mod 实现（0.3.1-rc.5，2026-10-17）

> 状态：**活**。这一页只讲**仓库边界**；协议、WS 帧、缺省启用集合仍以
> [CURRENT.md](CURRENT.md) 与源码为准。拆分口径来自
> [PROMPT-3.0-motion-first-2026-10-17.md](../plans/PROMPT-3.0-motion-first-2026-10-17.md)
> 的「分仓库」一节（原 v0.3.3，落到 0.3.1-rc.5）。

## 1. 布局

| 仓库 | 路径（本机） | 内容 |
| --- | --- | --- |
| 主仓 | `/home/skystar/Live2D-Ai` | `l2d`、`live2d-ai-core`、`live2d-ai-runtime`、`live2d-ai-desktop`、`l2d-wasm-demo`、`xtask`、`live2d-ai-mod-system`、`director`（已退役，仅留 `motion_bias`）、`template`、Flutter 壳、白模资源、协议文档 |
| 第二仓 | `/home/skystar/Live2D-Ai-mods` | 五个 Mod 实现：`external-input`、`persona`、`voice-input`、`memory`、`local-tts`（含 `melo/` 与已封存的 `engine/`） |

第二仓是主仓的 **sibling checkout**，不是 git submodule，也不是 vendored 副本。

## 2. 依赖方向

- 主仓根 `Cargo.toml` 的 `[workspace.dependencies]` 用
  `path = "../Live2D-Ai-mods/crates/..."` 指五个 Mod；`live2d-ai-desktop`
  用 `workspace = true` 继承。主仓 members 不再含这五个 crate。
- 第二仓五个 Mod 用 `live2d-ai-mod-system = { workspace = true }`，根清单把
  `live2d-ai-mod-system` / `live2d-ai-runtime` 指向
  `../Live2D-Ai/crates/...`；persona 的 dev-dep memory 用仓内路径。
- **无包级循环**：desktop → mod → mod-system/runtime，依赖图仍是 DAG。
  两个 workspace 互相引用的是不同 package，不是同一个 package 的环。

## 3. 主仓不再 standalone（已接受的代价）

主仓的 `cargo metadata` / `cargo test` 需要 `../Live2D-Ai-mods` 存在；
缺目录会直接失败。这是拆分的代价，不靠 `exclude` 或伪造的空 crate 兜底。
第二仓单独构建时同样需要 `../Live2D-Ai` 存在。

两树各自的门禁：

```bash
# 主仓
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio
cargo run -p xtask -- code-stats --check
(cd shell/flutter && flutter analyze && flutter test)

# 第二仓
cd /home/skystar/Live2D-Ai-mods
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

主仓 `rust-ratio` 只统计主仓文件系统，不再把五个 Mod crate 计入分子/分母；
第二仓的门禁是 `cargo test/fmt/clippy`，不另设占比门槛。

## 4. 不变性（拆分不改变产品）

- 工厂仍 **5**：`mod_count_is_five` / `mod_factory_ids_match_expected` 不动。
- 缺省启用仍 `external-input` + `local-tts-melo`。
- 协议、WS 帧、`ModServices` 契约不动；`action_tx` 仍休眠。
- Mod **不写** `[tts]` / `[llm]`，不接 `action_tx`。
- `director` 与 `template` 留主仓：desktop 仍用 `director::motion_bias`；
  template 是脚手架。
- `docs/examples/voice-sidecar/` 留主仓；voice-input 的测试 fixture 在第二仓
  内复制一份（`crates/live2d-ai-mod-voice-input/fixtures/fake_zh.wav`）。

## 5. 密钥：已知偏差 S2（本轮不修）

- 现状：`external-input` 与 `memory` 仍通过 `live2d_ai_runtime::secrets::lookup`
  在自己进程内读取 `.env` 快照；**没有新增暴露面**，Mod 拿不到密钥值，
  也不把值写进 `state_json` / 日志 / WS。
- 未做：`ModServices` 侧的 secret 代理（S1）。本轮不重做代理，避免在拆仓时
  同时改协议。
- 后续 S1：由宿主提供受限 secret 查询能力（只回「已配置/未配置」或按需签名），
  Mod 不再直接依赖 runtime 的 secrets 实现；在此之前保持现状并在评审中标注。

## 6. 权重与 LFS

- 第二仓 `.gitattributes` 只给
  `crates/live2d-ai-mod-local-tts/melo/weights/checkpoint.pth` 加 LFS；
  `config.json` 是普通 git 文件。
- `melo/.venv/`、`melo/.cache/`、`melo/.src/`、CosyVoice 的 `weights/`
  都在第二仓 `.gitignore`；它们留在本机，**不入库、不进 LFS**。
- 第二仓首提交含 LFS 指针；本地 `git lfs install --local` 后克隆方需
  `git lfs pull` 取权重。

## 7. 公开前的切换计划

当前只在本地两仓、**不建公开远端、不 push**。公开前必须做且只做其中一条：

1. **换 git 依赖**：主仓把五条 path 依赖换成固定 `git = "..."` + `rev`
   （或 tag），第二仓同理指回主仓的 `mod-system` / `runtime`；两仓各自可独立
   `cargo fetch`。
2. **CI 双 checkout**：CI 在同一父目录 checkout 两仓，保留 path 依赖；在
   `pr-checks.yml` 增加第二仓的 test/fmt/clippy job。公开主仓的 README 必须
   写明「需同父目录 checkout 第二仓」，否则主仓不是可独立构建的仓库。

切换前不得声称主仓可独立构建，也不得把第二仓权重/缓存推进公开历史。
