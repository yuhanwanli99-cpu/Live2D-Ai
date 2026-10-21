# 一 Mod 一本地仓库（2026-10-19）

> 状态：**活**。这一页只讲**仓库边界**；协议、WS 帧、缺省启用集合仍以
> [CURRENT.md](CURRENT.md) 与源码为准。拆分口径来自
> [PLAN-main-core-mod-repos-2026-10-19.md](../plans/PLAN-main-core-mod-repos-2026-10-19.md)
> 与确认稿 [CONFIRM-repo-split-2026-10-19.md](../plans/CONFIRM-repo-split-2026-10-19.md)。

## 1. 布局

| 仓库 | 路径（本机） | 内容 |
| --- | --- | --- |
| 主仓 | `/home/skystar/Live2D-Ai` | `l2d`、`live2d-ai-core`、`live2d-ai-runtime`、`live2d-ai-desktop`、`l2d-wasm-demo`、`xtask`、`live2d-ai-mod-system`、`director`（已退役，仅留 `motion_bias`）、`template`、Flutter 壳、白模资源、协议文档 |
| `external-input` | `/home/skystar/live2d-ai-mod-external-input` | crate `live2d-ai-mod-external-input`，id `external-input` |
| `persona` | `/home/skystar/live2d-ai-mod-persona` | crate `live2d-ai-mod-persona`，id `persona` |
| `voice-input` | `/home/skystar/live2d-ai-mod-voice-input` | crate `live2d-ai-mod-voice-input`，id `voice-input` |
| `memory` | `/home/skystar/live2d-ai-mod-memory` | crate `live2d-ai-mod-memory`，id `memory` |
| `local-tts` | `/home/skystar/live2d-ai-mod-local-tts` | crate `live2d-ai-mod-local-tts`，id `local-tts-melo`（工厂 `MELO_FACTORY`），含 `melo/` 与已封存的 `engine/` |
| `local-engine` | `/home/skystar/live2d-ai-mod-local-engine` | crate `live2d-ai-mod-local-engine`，id `local-engine`（spawn-only） |
| `a2a` | `/home/skystar/live2d-ai-mod-a2a` | crate `live2d-ai-mod-a2a`，id `a2a` |

七个 Mod 仓库都是**独立 git 仓库**，仓库根 = crate 根。公开地址在 [CURRENT.md](CURRENT.md) §8，分支 `main` 已推。
每个都是主仓的 sibling checkout，不是 git submodule，也不是 vendored 副本。
旧目录 `/home/skystar/Live2D-Ai-mods` **保留**（历史来源），已不被主仓引用；主仓清单、
脚本与 pytest 都不再引用它（pytest 的 Melo 路径已改指 `../live2d-ai-mod-local-tts/melo/serve.py`）。

## 2. 依赖方向

- 只允许 **Mod → 主仓**：每个 Mod 仓库根 `Cargo.toml` 用 `path` 指
  `../Live2D-Ai/crates/live2d-ai-mod-system` 与用到的
  `../Live2D-Ai/crates/live2d-ai-runtime`；不指回旧的 `Live2D-Ai-mods`，
  也不在 Mod 仓库之间互相 `path`。
- 主仓根 `[workspace.dependencies]` 与 `crates/live2d-ai-desktop/Cargo.toml`
  **没有**任何指向 Mod 仓库的 `path` / `git` 依赖；主仓 members 不含这七个 crate。
- **无包级循环**：desktop → mod → mod-system/runtime，依赖图仍是 DAG。
  跨仓库引用的是不同 package，不是同一个 package 的环。

## 3. 主仓 standalone（2026-10-19 已落地）

克隆主仓、填好 `[llm]` 与 `[tts]`，核心就能编译并跑起来；不需要七个 Mod 仓库中的
任何一个，也不需要旧的 `Live2D-Ai-mods`。云端 TTS 时连 Melo 启动器也不用
（`[tts].mode=cloud`）。

```bash
# 主仓（缺省：工厂 0，manifest 空）
cargo build -p live2d-ai-desktop
cargo metadata --no-deps --format-version 1   # 输出里不应出现 Mod 仓库 / Live2D-Ai-mods
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio
cargo run -p xtask -- code-stats --check
(cd shell/flutter && flutter analyze && flutter test)
```

主仓 `rust-ratio` 只统计主仓文件系统，不再把 Mod crate 计入分子/分母。七个 Mod 仓库
各自跑自己的门禁（不另设占比门槛）：

```bash
cd /home/skystar/live2d-ai-mod-<name>
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

## 4. 缺省态与挂载步骤

`0.3.11` 的启动器（已落地，见 [`PLAN-0.3.11-launcher-2026-10-20.md`](../plans/PLAN-0.3.11-launcher-2026-10-20.md)）只能勾选**已经编进本次二进制**的 id（工厂 0 时列表为空）；它不代替下面五步，也不能凭页面上的数字把 Mod 装进来。`0.3.12`（已落地，见 [`PLAN-0.3.12-mod-folder-scan-2026-10-20.md`](../plans/PLAN-0.3.12-mod-folder-scan-2026-10-20.md)）只预检 `mods/<id>/mod.toml`（只读、不执行、不 spawn）；**扫到文件夹不等于挂载**，挂载仍是下面五步（重新编译进二进制）。用户放文件夹的步骤见 [`mods-user.md`](../mods-user.md)。

缺省提交里：`AVAILABLE_MOD_FACTORIES` 为空（工厂 **0**），`default_mods_manifest` 的
`mods` 为空对象——**一个 Mod 都不启用**（`external-input` / `local-tts-melo`
也不再缺省启用）。未挂载时 `POST /api/v1/external/chat`、
`POST /api/v1/voice/transcript`、`GET /.well-known/agent-card.json`、
`POST /api/v1/a2a` 回 **404**，body `error.code = "mod_disabled"`，不打 LLM、不 `say`。

挂载**没有动态加载器**，由维护者手动完成：

1. 根 `Cargo.toml` 的 `[workspace.dependencies]` 加
   `live2d-ai-mod-<name> = { path = "../live2d-ai-mod-<name>" }`；
2. `crates/live2d-ai-desktop/Cargo.toml` 用 `workspace = true` 加依赖；
3. `crates/live2d-ai-desktop/src/main.rs` 的 `AVAILABLE_MOD_FACTORIES` 注册
   `&<crate>::FACTORY`，并同步 `mod_count_is_*` / `mod_factory_ids_match_expected`；
4. 恢复该 Mod 的宿主路由 / 集成代码（如 external/chat、voice/transcript、A2A 门）；
5. 若要在缺省 manifest 里启用，同步 `cli_entry::default_mods_manifest`。

缺省提交里**没有**以上任何一条；每次挂载都是显式改动，评审时逐条看。

## 5. 不变性（拆仓不改变产品）

- 协议、WS 帧、`ModServices` 契约不动；`action_tx` 仍休眠。
- Mod **不写** `[tts]` / `[llm]`，不接 `action_tx`；`local-engine` 也没有
  `base_url`，只按 4 项 schema（program / args_json / workdir / launch_mode）spawn。
- `director` 与 `template` 留主仓：desktop 仍用 `director::motion_bias`；
  template 是脚手架。
- `docs/examples/voice-sidecar/` 留主仓；voice-input 的测试 fixture 在它自己的仓库
  （`fixtures/fake_zh.wav`）。

## 6. 密钥：已知偏差 S2（本轮不修）

- 现状：`external-input` 与 `memory` 仍通过 `live2d_ai_runtime::secrets::lookup`
  在自己进程内读取 `.env` 快照；**没有新增暴露面**，Mod 拿不到密钥值，
  也不把值写进 `state_json` / 日志 / WS。
- 未做：`ModServices` 侧的 secret 代理（S1）。本轮不重做代理，避免在拆仓时
  同时改协议。
- 后续 S1：由宿主提供受限 secret 查询能力（只回「已配置/未配置」或按需签名），
  Mod 不再直接依赖 runtime 的 secrets 实现；在此之前保持现状并在评审中标注。

## 7. 权重与本地产物

- 不复制 Melo / CosyVoice 权重、`.venv`、`.cache`、`__pycache__`、
  `checkpoint.pth` 到新仓库；它们留在本机，**不入库**。各自 Mod 仓库的
  `.gitignore` 负责自己的安装产物与权重。
- `local-tts` 仓库只含源码 / 脚本 / `melo/serve.py` / `start.sh` / 许可证说明，
  不含 `melo/weights/`、`engine/` 里的权重和虚拟环境。
- 公开仓库只是源码索引。缺省构建仍只用主仓自己的树，不需要 CI 双 checkout，也不把 Mod 写成必选 `git` 依赖。
