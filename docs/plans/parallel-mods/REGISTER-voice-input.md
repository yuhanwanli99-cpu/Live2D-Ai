# REGISTER — voice-input（给集成 PR 的接线清单）

> 分支 `mod/voice-input`（基线 `2d492447`）交付的是**骨架**：crate 已建、已进
> workspace、已有 desktop path 依赖与测试，但**故意没有注册**。
> 集成 PR 按 `PARALLEL-PROTOCOL-2026-09-14.md` §3 / §5，用本文件把下面勾全。

## 0. 本分支已经做了什么（集成方不必重做）

- `crates/live2d-ai-mod-voice-input/` 新 crate（自 `live2d-ai-mod-template` 复制）：
  - `DESCRIPTOR` = `voice-input` / `api_version = MOD_API_VERSION`；
  - 静态 `settings_spec`：`backend`(mock|sidecar) / `locale` / `token`(secret)，
    **不含** `enabled`（启停唯一真源 = manifest）；
  - 纯函数 `clean_transcript` / `locale_from_config` / `token_from_config` /
    `VoiceBackend::from_config`；
  - `VoiceInputRuntime::inject_transcript`（清洗 → `say_tx`）+ mock 入口；
  - **13 条单测**，`cargo test -p live2d-ai-mod-voice-input` 绿；
  - `src/lib.rs` 共 499 行（源码闸门 ≤500）。
- 根 `Cargo.toml`：`members` 已加一行。
- `crates/live2d-ai-desktop/Cargo.toml`：已加 path 依赖（**未使用**，只为随
  workspace 编译；`cargo check -p live2d-ai-desktop --all-targets` 通过）。
- `docs/examples/voice-sidecar/README.md`：占位说明（**无脚本**）。

## 1. 集成 PR 待办（勾选表）

- [ ] `crates/live2d-ai-desktop/src/main.rs` 的 `AVAILABLE_MOD_FACTORIES` 加入
      `&live2d_ai_mod_voice_input::FACTORY`
- [ ] `main.rs::mod_count_is_three` → 改名 `mod_count_is_four`，断言 3 → 4，
      注释列出 `external-input, pet-desktop, persona, voice-input`
- [ ] `main.rs::mod_factory_ids_match_expected` 的 `expected` 加 `"voice-input"`
- [ ] 决定**缺省启用与否**：
      - 推荐**缺省停用**（ASR 后端尚未接线）：只注册，不动
        `cli_entry::default_mods_manifest`，用户在前端「Mod 管理」里手动启用；
      - 若缺省启用：同步 `default_mods_manifest` +
        `docs/architecture/mod-product-chain.md` §5 表 + `AGENTS.md`
- [ ] `docs/architecture/mod-product-chain.md` §5 表加一行（id / 缺省 / 语言 / 说明）
- [ ] `AGENTS.md` 的 Mod 段同步（「3 个 Mod」等计数与清单）
- [ ] 若选 sidecar 专属端点（README §3 选项 B）：web_api 新增 handler +
      契约文档 + 回归，并把 `docs/examples/voice-sidecar/README.md` §3 的
      「未实现」改成真契约
- [ ] 门禁（见 `AGENTS.md` 一张表）：`cargo test --workspace --all-targets`、
      `cargo test --doc --workspace`、`cargo fmt --all -- --check`、
      `cargo clippy --workspace --all-targets -- -D warnings`、
      `cargo run -p xtask -- rust-ratio`

## 2. 不要做

- 不要把 ASR 依赖（whisper / onnx / 音频解码）塞进任何一个 Rust crate；
- 不要复活 Action / director；
- 不要在 Mod 里开 socket，也不要直读进程环境 / 密钥（密钥只经
  `live2d_ai_runtime::secrets::lookup`，由 host 注入）；
- 不要在本分支预期之外改 `FACTORIES` / `mod_count_*` / 缺省 manifest / 全局版本。

## 3. 契约提醒（集成后行为）

- 清洗后为空的转写**不进** `say_tx`（避免空输入回合，只记一条 warn）；
- `backend = sidecar` 目前只是配置占位：Rust 侧不会去连 sidecar；
- `token` 是 secret：GET 只回「是否已设置」，永不回明文；
- v0 **不订阅**任何 host 事件；`ModServices.action_tx` 保持休眠，不接。
