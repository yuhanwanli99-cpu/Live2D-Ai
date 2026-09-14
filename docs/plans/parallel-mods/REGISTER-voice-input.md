# REGISTER — voice-input（给集成 PR 的接线清单）

> 分支 `mod/w3-voice`（worktree `/home/skystar/Live2D-Ai-w3-voice`，基线 `118bd435`）。
> Wave 1 交付的是**骨架**（未注册）；Wave 2 集成注册 + 端点；**Wave 3 A 轨**把
> `backend` / `locale` 做成**可测行为**，Mod 拆成 `lib.rs` + `normalize.rs` +
> `tests.rs`。注册 / manifest / 版本号**均不需要动**。

---

## 0. 本分支已经做了什么（集成方不必重做）

- `crates/live2d-ai-mod-voice-input/`：
  - `DESCRIPTOR` = `voice-input` / `api_version = MOD_API_VERSION`；
  - 静态 `settings_spec`：`backend`(mock|sidecar) / `locale` / `token`(secret)，
    **不含** `enabled`（启停唯一真源 = manifest）；
  - **纯函数** `clean_transcript`（零宽 / 控制符 / 空白折叠）、
    `normalize_for_locale`（locale 档，幂等）、
    `prepare_transcript`（= 清洗 + 归一化，handler 与 Mod 唯一入口）、
    `locale_from_config` / `token_from_config` / `VoiceBackend::from_config`；
  - **结构性无网络**：`VoiceBackend::opens_network()` 恒 `false`；
    `RustRoute::{LocalInject,AcceptPush}` 只有本地变体；
  - `VoiceInputRuntime::inject_transcript`（清洗 + 归一化 → `say_tx`）+ mock 入口；
  - **21 条单测**，`cargo test -p live2d-ai-mod-voice-input` 绿；
  - 文件：`src/lib.rs`（333）/ `src/normalize.rs`（103）/ `src/tests.rs`（340），
    源码 ≤500 / 测试 ≤800。
- 根 `Cargo.toml`：`members` 已含本 crate（Wave 1）；desktop 已有 path 依赖。
- 端点（Wave 2 A 轨）：`crates/live2d-ai-desktop/src/web_api/voice_routes.rs`。
- 文档：`docs/voice-input.md`（Wave 3 已更新）；`docs/examples/voice-sidecar/`。
- `backend` / `locale` 的 Wave 3 细节见
  `REGISTER-voice-sidecar-v1.md` §0（同一轨的另一份清单）。

## 1. 集成 PR 待办（勾选表）

- [ ] **FACTORIES / mod_count / id 断言：不用改**。`voice-input` 已在
      `AVAILABLE_MOD_FACTORIES` 中（`mod_count_is_six`）。
- [ ] **缺省 manifest：不用改**（缺省停用是刻意的，用户装好 ASR 再开）。
- [ ] `docs/architecture/mod-product-chain.md` §5 表：`voice-input` 行说明更新为
      「端点已通 + backend/locale 可测」（独占清单，主 agent 改）。
- [ ] `AGENTS.md` Mod 段（独占清单，主 agent 改）。
- [ ] `docs/README.md` 索引（独占清单，主 agent 改）。
- [ ] 门禁（主 agent 收束全量跑）。

## 2. 不要做

- 不要把 ASR 依赖（whisper / onnx / 音频解码 / reqwest）塞进任何 Rust crate；
- 不要复活 Action / director；`action_tx` 保持休眠（回归
  `action_tx_stays_dormant_on_inject`）；
- 不要在 Mod 里开 socket（`opens_network` 必须恒 `false`），也不要直读进程环境 /
  密钥（密钥只经 `live2d_ai_runtime::secrets::lookup`，由 host 注入）；
- 不要把 `locale` 写成「会传给 ASR」或「选 ASR 引擎」。

## 3. 契约提醒（当前行为）

- 清洗 + 归一化后为空的转写**不进** `say_tx`（避免空输入回合，只记一条 warn）；
- `backend = sidecar` 是**推模式**：Rust 不会去连 sidecar；两个 backend 的本地
  落点相同，响应回显 `backend` 供观察；
- `locale` 只影响 text 归一化（`zh-CN` 删 CJK 词间空格 / `en-US` 补中英边界）；
- `token` 是 secret：GET 只回「是否已设置」，永不回明文；
- v0 **不订阅**任何 host 事件；`ModServices.action_tx` 保持休眠。

## 4. 证据

```text
$ cargo test -p live2d-ai-mod-voice-input
test result: ok. 21 passed; 0 failed

$ cargo clippy -p live2d-ai-mod-voice-input --all-targets -- -D warnings
# 0 warning
```
