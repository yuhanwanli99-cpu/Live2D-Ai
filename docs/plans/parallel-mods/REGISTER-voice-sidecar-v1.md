# REGISTER — voice-sidecar-v1（给主 agent 的接线清单）

> 分支 `mod/w3-voice`（worktree `/home/skystar/Live2D-Ai-w3-voice`，
> 基线 `118bd435` = 0.2.0-rc.3 + Wave 3 基座）。
> Wave 2 A 轨把端点 + sidecar 做成**可演示**；**Wave 3 A 轨**把 `backend` /
> `locale` 从死配置做成**可测行为**，并补齐 sidecar 的 6 类失败退避。
>
> **不需要动** `AVAILABLE_MOD_FACTORIES` / `mod_count_*` / 缺省 manifest /
> 版本号（`voice-input` 已在 Wave 1 集成时注册，缺省停用）。
> 本清单只列**文档 / AGENTS 同步项**。

---

## 0. 本分支已经做了什么（主 agent 不必重做）

### Wave 2 A 轨（已合并，此处仅备查）

- `crates/live2d-ai-desktop/src/web_api/voice_routes.rs`：`POST /api/v1/voice/transcript`
  静态前置路由，自带 Origin / Content-Type / 门禁 / token / 清洗 / 长度校验；
  落点 `ctx.try_get_supervisor()` → `supervisor.say`。
- `voice_routes_tests.rs`：25 条回归（Wave 2 时；Wave 3 增至 **27 条**）。
- `docs/voice-input.md`、`docs/examples/voice-sidecar/`（Python 仅标准库 +
  `--selftest` / `--dry-run` + fixtures）。

### Wave 3 A 轨（本轮，2026-09-14）

1. **`backend=mock|sidecar` 行为可测**
   - handler 读 Mod config 的 `backend`（`VoiceBackend::from_config`），
     成功响应回显 `backend`（`mock` / `sidecar`）——分支可观察、可回归；
   - **Rust 不碰网络是结构性的**：`VoiceBackend::opens_network()` 恒 `false`、
     `RustRoute` 枚举只有 `LocalInject` / `AcceptPush` 两个本地变体。
     回归：`no_backend_opens_network`（Mod）、
     `backend_branches_never_wait_on_network`（handler，黑洞 LLM 端点下仍立即 200）；
   - sidecar **失败码表 / 退避**落在发送方（Rust 无网络可失败）：
     Python `SIDECAR_FAILURES` + `RETRY_POLICY`（6 类），`--selftest` 逐条断言。
2. **`locale` 真影响归一化**
   - 新模块 `crates/live2d-ai-mod-voice-input/src/normalize.rs`：
     `LocaleProfile::{Cjk,Latin}` + `normalize_for_locale`（幂等）+
     `prepare_transcript`（= `clean_transcript` + 归一化，handler 与 Mod **唯一**入口）；
   - `zh-CN` 删 CJK 词间空格（`"你好 世界"` → `"你好世界"`）；
     `en-US` 保留空格并补中英边界（`"打开空调wifi"` → `"打开空调 wifi"`）；
   - 文档钉死「`locale` 只影响 text 归一化，**不影响 ASR 引擎选型**、不发给 sidecar」。
3. **Mod crate 拆文件**（源码 ≤500 行纪律）：
   `src/lib.rs`（333 行）+ `src/normalize.rs`（103 行）+ `src/tests.rs`（340 行，
   **21 条**回归，原 13 条 + 8 条）。
4. **sidecar 演示 + 退避**：README §3 三步端到端（点火 → 启用 → `--audio` 真发）
   + 麦克风替代；§5.1 退避表；`voice_sidecar.py` 区分 `timeout` / `transport`
   （`report_send_failure` 判定 `TimeoutError` / `socket.timeout`）。
5. **`docs/voice-input.md`**：新增 §1.1 branch / §1.2 locale / §4.4 失败退避 /
   §6.1 locale 归一化 / §8 **职责边界表**（何时用 voice vs external）。

**没碰**：`topics.rs` / `factory.rs` / `mod_registry.rs` / `mods_routes.rs` /
`supervisor.rs`（基座文件）、`main.rs` 的 `AVAILABLE_MOD_FACTORIES` /
`mod_count_*` / `mod_factory_ids_match_expected`、
`cli_entry::default_mods_manifest`、根 `Cargo.toml` 版本、`pubspec.yaml`、
`AGENTS.md`、`docs/README.md`、`docs/releases/` 目录——全部未动。

---

## 1. 集成待办（勾选表）

- [ ] **FACTORIES：不用改**。`voice-input` 已在册，`mod_count_is_six` / id 断言保持原样。
- [ ] **缺省 manifest：不用改**。`voice-input` 缺省**停用**是刻意的；端点停用时回
      `403 mod_disabled`。改成缺省启用要同步 AGENTS + `mod-product-chain.md` +
      发布说明——**本轨不建议**。
- [ ] **`docs/README.md` 索引**：加一行 `docs/voice-input.md`（本轨**没改**该文件）。
- [ ] **`AGENTS.md` 同步**（独占清单，主 agent 改）：
      - `voice-input` 描述从「骨架 / ASR 未接线」改为「端点已通 + `backend`/`locale`
        可测（Wave 3）」；
      - 「Mod 能力现状」补一句「语音输入：sidecar → 清洗 + locale 归一化 → say」。
- [ ] **Wave 3 收束报告**（`WAVE3-CLOSEOUT-2026-09-14.md`，主 agent 写）：
      A 轨：tip SHA / 闭环达成 / 未决。
- [ ] **门禁**（主 agent 收束时全量跑）：`cargo test --workspace --all-targets` /
      `cargo test --doc --workspace` / `cargo fmt --all -- --check` /
      `cargo clippy --workspace --all-targets -- -D warnings` /
      `cargo run -p xtask -- rust-ratio`。

## 2. 不要做

- 不要给 `voice-input` 加 ASR / 音频解码 / 网络依赖（whisper / onnx / reqwest…），
  也不要在 Mod 里开 socket 或读进程环境（`opens_network` 必须恒 `false`）；
- 不要复用 `/api/v1/external/chat` 取代本端点（那会让 `clean_transcript` 重新
  变成只有单测调用的摆设）；
- 不要让端点回 5xx 表示忙碌（`busy` 刻意是 `200 + ok:false`）；
- 不要把 handler 的处理换成 inline 实现（必须 `prepare_transcript`）；
- 不要把 `locale` 写成「会传给 ASR」或「选 ASR 引擎」（它只影响 text 归一化）；
- 不要在任何响应 / 日志 / dry-run 输出里回显 token 明文。

## 3. 契约提醒（合并后行为）

- **成功响应新增两个字段**：`{"ok":true,"text":…,"backend":"mock|sidecar","locale":"…"}`
  （向后兼容：老发送方只读 `text` 不受影响）；
- **`text` 现在是清洗 + locale 归一化后的文本**（例如缺省 `zh-CN` 下
  `"你好 世界"` → `"你好世界"`）——旧文档只写「清洗后」，Wave 3 已改；
- **backend 分支**：`mock`（缺省）与 `sidecar` 的 Rust 落点相同（本地说 `say`），
  响应回显 backend 以证明分支；
- **错误码**：`invalid_payload`（结构错）与 `empty_transcript`（清洗后为空）
  **刻意分开**；`text_too_long` 按清洗 + 归一化后长度判定（≤2000 字符）；
- **门禁三态**：在册且停用 → 403 `mod_disabled`；在册且启用 → 放行；
  **不在册** → 不设门禁（与 external 同口径）；
- **token 优先级**：env `VOICE_INPUT_TOKEN` → Mod config `token` → 不鉴权；
  body `token` 与 `Authorization: Bearer` 二选一；
- **locale 归一化**：`Cjk` 删 CJK 词间空格 / `Latin` 补中英边界；幂等。

## 4. Wave 3 实跑证据（可复制）

```text
$ python3 docs/examples/voice-sidecar/voice_sidecar.py --selftest
[selftest] 全部通过（70 项检查；离线，未发起任何请求）            # exit 0

$ python3 docs/examples/voice-sidecar/voice_sidecar.py \
    --audio docs/examples/voice-sidecar/fixtures/fake_zh.wav --dry-run
[voice-sidecar] dry-run（未发请求）
  POST http://127.0.0.1:18080/api/v1/voice/transcript
  body {"text": "把窗户关小一点"}                                  # exit 0

# 对 mock 端点实跑 6 类失败（每类退出码 + 退避说明都打印）：
#   ok(200)                -> exit 0
#   busy(200 + ok:false)   -> exit 5   退避（backoff）
#   mod_disabled(403)      -> exit 4   退避（fix）
#   unauthorized(401)      -> exit 4   退避（fix）
#   empty_transcript(400)  -> exit 4   退避（drop）
#   transport(连接被拒)     -> exit 4   退避（backoff）
#   timeout(slow server)   -> exit 4   退避（backoff）
```

```text
$ cargo test -p live2d-ai-mod-voice-input
test result: ok. 21 passed; 0 failed

$ cargo test -p live2d-ai-desktop --bin live2d-ai-desktop voice_routes
test result: ok. 27 passed; 0 failed
```
