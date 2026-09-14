# REGISTER — voice-sidecar-v1（给主 agent 的接线清单）

> 分支 `mod/voice-sidecar-v1`（worktree `/home/skystar/Live2D-Ai-w2-voice`，基座 `429609f2`）。
> 本轮把 Wave 1 的 `voice-input` 骨架推成**可演示链路**：
> `POST /api/v1/voice/transcript` → `clean_transcript` → `supervisor.say`，
> 加一个**可跑**的 sidecar 示例（音频 → ASR → POST）。
>
> **预计不需要动 `AVAILABLE_MOD_FACTORIES` / `mod_count_*` / 缺省 manifest / 版本号**
> ——`voice-input` 已在 Wave 1 集成时注册（5 个工厂，`mod_count_is_five`，缺省停用）。
> 本清单只列**文档 / AGENTS 同步项**。

---

## 0. 本分支已经做了什么（主 agent 不必重做）

- **新文件** `crates/live2d-ai-desktop/src/web_api/voice_routes.rs`（338 行 ≤500）：
  `POST /api/v1/voice/transcript` 静态前置路由，自带 Origin / Content-Type /
  门禁 / token / 清洗 / 长度校验；落点 `ctx.try_get_supervisor()` → `supervisor.say`。
- **新文件** `crates/live2d-ai-desktop/src/web_api/voice_routes_tests.rs`（708 行 ≤800）：
  **25 条**回归，经 `#[cfg(test)] #[path = "voice_routes_tests.rs"] mod tests;` 挂在
  `voice_routes` 下（`mod.rs` 只多一行 `pub mod voice_routes;`，与计划 §1 的
  「一处 `pub mod` + 一段前置路由钩子」一致）。
- **`web_api/mod.rs`**：`pub mod voice_routes;` + 紧邻 `external_routes::handle_external_chat`
  钩子之后的一段前置路由（+ 头注路径表一行 + 模块列表一行注释）。
- **`docs/voice-input.md`**（新）：端点 / token 优先级 / 门禁 / 错误码表 / 6 条 curl /
  与 `/api/v1/external/chat` 的差异与「为什么选专用端点」/ 安全 / 实现索引。
- **`docs/examples/voice-sidecar/`**：README **整份重写**（依赖 / 环境变量 / 一条可复制
  命令 / `--dry-run` / 失败码表 / 与 external 的差异），新增
  `voice_sidecar.py`（仅标准库，`--transcriber fake|cmd:` / `--dry-run` /
  `--selftest`，退出码 0/2/3/4/5，token 打码）+ `fixtures/fake_zh.wav`（8044 字节）
  + `fixtures/fake_zh.txt`。
- **`docs/architecture/mod-product-chain.md` §5**：`voice-input` 行从「骨架 / 占位」
  改为「端点已接线」并指向本 REGISTER。
- **`crates/live2d-ai-mod-voice-input/src/lib.rs`**：**只改注释**（头注写「sidecar 路径
  已通，端点 = `POST /api/v1/voice/transcript`」）——无 socket、无网络依赖、无新依赖，
  文件仍 499 行。

**没碰**：`topics.rs` / `factory.rs` / `mod_registry.rs` / `mods_routes.rs` /
`supervisor.rs`（基座文件）、`main.rs` 的 `AVAILABLE_MOD_FACTORIES` /
`mod_count_*` / `mod_factory_ids_match_expected`、`cli_entry::default_mods_manifest`、
`Cargo.toml` 版本、`pubspec.yaml`、`AGENTS.md`。

---

## 1. 集成待办（勾选表）

- [ ] **FACTORIES：不用改**。`crates/live2d-ai-desktop/src/main.rs` 的
      `AVAILABLE_MOD_FACTORIES` 已含 `&live2d_ai_mod_voice_input::FACTORY`
      （Wave 1 集成时加入），`mod_count_is_five` / id 断言**保持原样**。
      收束轮若因 C 轨 memory 变成 `mod_count_is_six`，那是主 agent 的事，与本轨无关。
- [ ] **缺省 manifest：不用改**。`voice-input` 缺省**停用**是刻意的
      （`cli_entry::default_mods_manifest` 只收录 `external-input`）：
      端点在 Mod 停用时回 `403 mod_disabled`（见契约 §5）。若要改成缺省启用，
      必须同步 `AGENTS.md` + `mod-product-chain.md` §5 + 发布说明——**本轨不建议**。
- [ ] **`docs/README.md` 索引**：加一行 `docs/voice-input.md`（本轮**没改**该文件，
      避免与其它轨争抢索引；请主 agent 在收束时一次加到位）。
- [ ] **`AGENTS.md` 同步**（独占清单，主 agent 改）：
      - `voice-input` 的描述从「骨架 / ASR 未接线」改为「端点已通
        （`POST /api/v1/voice/transcript`），ASR 在 sidecar」；
      - 如有「Mod 能力现状」段落，补一句「语音输入可演示：sidecar → 清洗 → say」。
- [ ] **发布说明** `docs/releases/v0.2.0-rc.3.md`：写清「A 轨：语音 sidecar 闭环」
      与**演示步骤**：
      1. `./scripts/ignite.sh`（默认 18080）；
      2. `curl -X POST .../api/v1/mods/voice-input/enable`（缺省停用）；
      3. `python3 docs/examples/voice-sidecar/voice_sidecar.py --audio …/fixtures/fake_zh.wav`
         → 皮套开口说「把窗户关小一点」。
- [ ] **门禁**：`cargo test -p live2d-ai-desktop --bin live2d-ai-desktop` /
      `cargo test --workspace --all-targets` / `cargo test --doc --workspace` /
      `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings`
      / `cargo run -p xtask -- rust-ratio`。

## 2. 不要做

- 不要给 `voice-input` 加 ASR / 音频解码 / 网络依赖（whisper / onnx / reqwest…），
  也不要在 Mod 里开 socket 或读进程环境；
- 不要复用 `/api/v1/external/chat` 取代本端点（那会让 `clean_transcript`
  重新变成只有单测调用的摆设）——二选一已钉死（Wave 2 计划 §3A 选项 B）；
- 不要让端点回 5xx 表示忙碌（`busy` 刻意是 `200 + ok:false`）；
- 不要把 handler 的清洗换成 inline 实现（必须 `live2d_ai_mod_voice_input::clean_transcript`）；
- 不要在任何响应 / 日志 / dry-run 输出里回显 token 明文。

## 3. 契约提醒（合并后行为）

- **错误码**：`invalid_payload`（结构错）与 `empty_transcript`（清洗后为空）
  **刻意分开**；`text_too_long` 按**清洗后**长度判定（≤2000 字符）；
- **门禁三态**：在册且停用 → 403 `mod_disabled`；在册且启用 → 放行；
  **不在册**（单测 / 自定义装配）→ 不设门禁（与 external 同口径）；
- **token 优先级**：env `VOICE_INPUT_TOKEN` → Mod config `token` → 不鉴权；
  body `token` 与 `Authorization: Bearer` 二选一；
- **清洗语义**：零宽 / 控制符丢弃、空白（含全角空格）折叠、去首尾；空结果不进 `say`；
- **`backend` 字段仍是配置占位**：Rust 侧不连 sidecar（sidecar 是**推**模式，
  Rust 只在端点被调用时收文本）；`locale` 目前只影响 Mod 启动日志，
  **不**参与清洗或请求体——不要在文档里写成「会传给 ASR」。
