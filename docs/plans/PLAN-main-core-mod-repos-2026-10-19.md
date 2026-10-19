# 执行单：主仓只留核心，每个 Mod 一个本地仓库（2026-10-19）

> 确认稿 [`CONFIRM-repo-split-2026-10-19.md`](CONFIRM-repo-split-2026-10-19.md) 已确认。交给执行 agent。
> **版本号未指定。** 不要写成 `0.3.11`，不要并进 `0.4.0`。本地提交，不打 `v0.3.*` / `v0.4.*` tag，不 push，不在 GitHub 建仓库。
> 不改 `.env`、`mods.json`、`live2d-ai.toml`。不提交 `official_live2d_samples.7z`。不把 `/home/skystar/live2d-bili-sidecar/` 加进任何仓库。

## 0. 做完是什么样

只克隆主仓，填好 `[llm]` 与 `[tts]`，`cargo build -p live2d-ai-desktop` 能过，核心能跑。根 `Cargo.toml` 里没有 `Live2D-Ai-mods`，也没有指向各 Mod 仓库的 path 或 git 依赖。

`AVAILABLE_MOD_FACTORIES` 为空。缺省 manifest 不再启用 `external-input` 和 `local-tts-melo`。没有这些挂件时，核心对话、导演、TTS、前端仍在。

七个实现各自一个本地仓库（还没有远端）：

| 仓库目录 | crate | id |
|---|---|---|
| `/home/skystar/live2d-ai-mod-external-input` | `live2d-ai-mod-external-input` | `external-input` |
| `/home/skystar/live2d-ai-mod-persona` | `live2d-ai-mod-persona` | `persona` |
| `/home/skystar/live2d-ai-mod-voice-input` | `live2d-ai-mod-voice-input` | `voice-input` |
| `/home/skystar/live2d-ai-mod-memory` | `live2d-ai-mod-memory` | `memory` |
| `/home/skystar/live2d-ai-mod-local-tts` | `live2d-ai-mod-local-tts` | `local-tts-melo`（工厂 `MELO_FACTORY`） |
| `/home/skystar/live2d-ai-mod-local-engine` | `live2d-ai-mod-local-engine` | `local-engine` |
| `/home/skystar/live2d-ai-mod-a2a` | `live2d-ai-mod-a2a` | `a2a` |

主仓文档写上这七个本机路径，并写明远端未建。旧目录 `/home/skystar/Live2D-Ai-mods` 留着不要删。

## 1. 禁止

- 不把 Mod 实现搬回主仓，不用必选 git 依赖，不做 CI 双 checkout。
- 不建动态加载器。本单的「挂上」只写在文档里：要挂某个 Mod，由维护者以后把该 crate 加回依赖和工厂表。缺省提交里不要加。
- 钥匙仍是主进程 `.env` + `secrets::lookup`。不给每个仓库一份密钥，不做新的 secret 代理。`a2a` 继续拿不到密钥值。
- 不复制 Melo / CosyVoice 权重、`.venv`、`.cache`、`__pycache__`。`checkpoint.pth` 不进新仓库。
- 不删导演，不复活 `action_tx`，不改 `[llm]` / `[tts]` 端点。`live2d-ai-mod-system`、`template`、主仓里的 `director`（只剩 `motion_bias`）留在主仓。
- 不提高 `DOCS_BUDGET_LINES`，不改 `RATCHET_DART_800`。不改 `AGENTS.md` 历史段。

## 2. 主仓先断开 path

今天 `live2d-ai-desktop` 不只是登记工厂，还直接调用了三个 crate：

- `web_api/external_routes.rs` 调用 `live2d_ai_mod_external_input`
- `web_api/voice_routes.rs` 调用 `live2d_ai_mod_voice_input`
- `web_api/a2a_gate.rs` 调用 `live2d_ai_mod_a2a`

这三处改成不依赖那些 crate 也能编译。未挂载时对应路由回 **404**，body 用已有的 `mod_disabled` 码，不打 LLM、不 `say`。不要把 Mod 的实现抄进主仓来凑编译。

然后：

- 根 `Cargo.toml` 的七条 `path = "../Live2D-Ai-mods/..."` 删掉。
- `crates/live2d-ai-desktop/Cargo.toml` 里这七个 `workspace = true` 依赖删掉。
- `AVAILABLE_MOD_FACTORIES` 改为空切片。断言改名为 `mod_count_is_zero`。
- `default_mods_manifest` 不再启用 `external-input` 与 `local-tts-melo`。
- 主仓测试里凡是引用这七个 crate 的，改成测「未挂载即 404」，或者把依赖实现的用例挪到对应新仓库。`grep Live2D-Ai-mods` 在主仓清单里必须是 0。
- Flutter：`registered_mods.dart` 和工厂数断言改为 0。`kModsHiddenFromShell` 可以留着。扩展区没有任何 Mod 卡是对的。`flutter analyze` 与 `flutter test`。

证明：在主仓执行 `cargo metadata -p live2d-ai-desktop --no-deps --format-version 1` 不需要第二仓目录也能成功（清单里已无那条 path）。再跑主仓门禁：

```bash
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio
cargo run -p xtask -- code-stats --check
```

`live2d-ai-desktop` 没有 lib target，相关测试不要加 `--lib`。

## 3. 再拆七个本地仓库

从 `/home/skystar/Live2D-Ai-mods/crates/<crate>` **复制**（不要移动、不要删旧仓）到第 0 节的目录，各自 `git init` 一次提交。

每个新仓库的 `Cargo.toml` 只允许 path 指向主仓，例如 `../Live2D-Ai/crates/live2d-ai-mod-system` 和用到的 `../Live2D-Ai/crates/live2d-ai-runtime`。方向只有 Mod → 主仓。不要 path 回旧的 `Live2D-Ai-mods`。

`local-tts` 只复制源码、`melo/serve.py`、`start.sh`、`download.sh` 和许可证说明。不复制 `melo/weights/`、`engine/` 里的权重和虚拟环境。

每个新仓库在主仓已经能单独编译之后跑自己的 `cargo test`、`fmt --check`、`clippy -D warnings`。主仓 pytest `tests/melo_speakable_test.py` 改指新路径的 `melo/serve.py`。

## 4. 文档

改活文档，不改历史正文：

- `docs/architecture/CURRENT.md`：主仓工厂数为 0；七个本机仓库路径；远端未建；缺省不再启用那两个 Mod。
- `docs/architecture/two-repo-mods.md`：改成「一 Mod 一本地仓库」。删掉「编译必须有第二仓」的现状句。
- `README.md`：核心单独能跑；Mod 列表改成文档跳转，不写「克隆主仓还要再克隆 Live2D-Ai-mods」。
- `docs/README.md` 或 `DOC-MAP.md` 加一行指向本单。

界面若只是扩展区变空，要让正在跑的 `/app/` 一致就重编 web：`flutter build web --release --base-href /app/ --no-web-resources-cdn`，再 `prune_web_artifacts.sh --check`。不要杀已有的 `ignite.sh`。没在跑就不必为了文档去点火。

## 5. 交付

写明主仓提交 hash、七个新仓库的路径和首提交、`cargo metadata` 是否在没有 `Live2D-Ai-mods` 字样的清单上成功、工厂数 0、未挂载路由的状态码。旧第二仓还在。远端未建、未 push。`0.4.0` 仍是未开始。
