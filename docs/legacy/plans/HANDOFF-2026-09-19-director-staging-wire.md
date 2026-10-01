> 历史（2026-10-01 归档，勿当现网）。

# HANDOFF 2026-09-19 —— 导演二路 LLM 真实接线（P1-4）

> 工作树：`/home/skystar/Live2D-Ai-l1`。**不 bump / 不 push / 不打 tag。**
> 范围真源：本波任务书「诚实清单第 1 条：`staging.rs` 只有 trait + DisabledStaging，
> 即使 `staging_enabled=true` 也不会发 HTTP」。
> **只做这一件事 + 重建产物 + 自测**；未做真摘要 / 未做本地 ASR / 未扩主链。
> 禁止项（幅度默认大战 / 记忆摘要 / motion3 / 挂回 wallpaper|pet）一行未动。

---

## 1. 做了什么

| # | 必做 | 结果 |
| --- | --- | --- |
| 1 | OpenAI 兼容 staging HTTP 客户端 | ✅ `staging_http.rs`：`OpenAiStagingClient`（**非流式** `POST {base_url}/chat/completions`）。独立 `staging_base_url` / `staging_model` / `staging_timeout_ms` / `staging_api_key_env`；reqwest + `rustls-tls`；密钥走 `live2d_ai_runtime::secrets::lookup`（`.env` > 进程环境）。**思考不进请求**：请求体只有 system + user 两条消息（回归抓原始请求文本断言无 `reasoning`）。 |
| 2 | Host 装配时注入真实客户端 | ✅ `DirectorFactory::create`（host 装配点）调 `staging::assemble(&config)`：`staging_enabled=false` / 缺端点 / 缺模型 / URL 非法 → `DisabledStaging`（与「默认关」逐字一致）；配齐 → `OpenAiStagingClient`。 |
| 3 | 失败/超时/坏 JSON 静默回退 + 可观察 | ✅ 一律 `None` → 规则层继续；`state_json.staging` 新增 `client/degraded/note/base_url/model/api_key_env/api_key_set` 与既有 `async_plans/async_failures`。主链与送 TTS 文本零污染（导演只读锚点）。 |
| 4 | 配置面写清要哪些键 + 面板一句 | ✅ `settings_spec` 追加 5 个二路字段；`docs/architecture/director-mod-v0.md` §9.1 给 `mods.json` + `.env` 样例；Flutter 面板顶部说明补「**二路 LLM 未配端点则仅走规则**」。 |
| 5 | 回归 | ✅ `Disabled ≡ 未注入` / mock 成功 plan → cue（priority 40 覆盖 10）/ mock 失败 ≡ 纯规则 / epoch 不匹配零副作用；另有 loopback mock HTTP 真发请求回归。 |
| 6 | 自测门禁 | ✅ 见 §3（trunk + ignite + 活服务规则层 + **真端点二路冒烟**）。 |

---

## 2. 如何配键开启二路（两种入口）

**缺省关闭**。只打开总闸**不够**——必须同时给端点与模型。

### 2.1 面板 / API

`POST /api/v1/mods/director/config`（body `{"config": {...}}`，需 loopback Origin）：

```bash
curl -s -X POST -H 'Content-Type: application/json' \
  -H 'Origin: http://127.0.0.1:18080' \
  -d '{"config":{
        "staging_enabled":true,
        "staging_base_url":"https://api.deepseek.com/v1",
        "staging_model":"deepseek-flash",
        "staging_api_key_env":"DEEPSEEK_API_KEY",
        "staging_timeout_ms":5000
      }}' \
  http://127.0.0.1:18080/api/v1/mods/director/config
```

### 2.2 `mods.json`（director 的 namespaced config）

```json
{
  "mods": {
    "director": {
      "enabled": true,
      "config": {
        "staging_enabled": true,
        "staging_base_url": "https://api.deepseek.com/v1",
        "staging_model": "deepseek-flash",
        "staging_api_key_env": "DEEPSEEK_API_KEY",
        "staging_timeout_ms": 5000
      }
    }
  }
}
```

密钥值写 `.env`（唯一真源）：`DEEPSEEK_API_KEY=sk-...`。

### 2.3 怎么确认「真的接上了」

`GET /api/v1/mods/director/state` → `staging`：

| 想要 | 该看到 |
| --- | --- |
| 关闸 | `client:"disabled", enabled:false, degraded:false` |
| 开闸但没配齐 | `client:"disabled", degraded:true, note:"staging_base_url 未配置..."` |
| 真接上 | `client:"openai", enabled:true, degraded:false` |
| 发过请求 | `async_plans`（成功）/ `async_failures`（失败、超时、坏 JSON）增长 |

日志锚点：`director 二路 LLM 已接线（...）`、每句一条
`director action_cue：epoch=..., covers_upto_seq=..., cues=...`。

---

## 3. 自测证据（本机实测）

### 3.1 Disabled（默认关，规则层照常）

活服务启用 director（`staging_enabled` 未配）→ `POST /api/v1/external/chat` 发一句：

```text
state.plan      = {"covers_upto_seq":1,"cues_emitted":2,"epoch":0,
                   "rule_cues":1,"sentences_seen":2,
                   "cues":[{"preset_id":"nod","priority":10,"sentence_seq":1,"ttl_ms":2000}]}
state.staging   = {"client":"disabled","enabled":false,"degraded":false,"base_url":"","model":""}
日志            = director action_cue：epoch=0, covers_upto_seq=1, cues=1   （×2）
```

### 3.2 真客户端（真端点已冒烟，DeepSeek）

```text
配置后 state.staging = {"client":"openai","enabled":true,"degraded":false,
                        "base_url":"https://api.deepseek.com/v1",
                        "model":"deepseek-flash","api_key_set":true}
发一句后 state.plan  = {"cues":[{"preset_id":"expr_smile","priority":40,"ttl_ms":1800,
                                 "sentence_seq":1}], "rule_cues":1}
           state.staging = {"async_plans":1,"async_failures":0}
```

即：**异步 plan 真的从 DeepSeek 取回，并按 priority 40 覆盖了规则 cue（`nod`，priority 10）。**
日志：`director 二路 LLM 已接线（client=openai, base_url=https://api.deepseek.com/v1,
model=deepseek-flash, timeout_ms=5000；密钥已从 .env/环境解析）`。

### 3.3 失败回退（离线 mock，等价于 100% 失败）

- `staging_failure_is_identical_to_no_staging`：mock 恒 `None` 与
  `DisabledStaging` 的 cue **逐字段相同**，`async_failures=1`；
- `staging_http::tests::upstream_error_and_bad_json_fall_back_to_none`：
  loopback mock 返回 **500** / 非 JSON / 只有 `reasoning_content` 三种坏响应 → `complete` 全回 `None`；
- `staging_epoch_mismatch_is_rule_only_and_leaves_arbiter_clean`：plan 的 epoch 过期 →
  整份丢弃、`async_plans=0`、`async_failures=0`、cue 与纯规则逐字相同。

### 3.4 真 HTTP 确实发出了（不依赖任何外部端点）

`staging_http::tests::openai_client_posts_and_parses_content` 用一个 **loopback
一次性 HTTP 服务**接住请求，断言：

- 请求行 `POST /v1/chat/completions`；
- 头里有 `Authorization: Bearer sk-test-123`（`with_api_key` 注入）；
- body 有 `"stream":false` / `"model":"director-model"`；
- body **不含** `reasoning`（也不含 mock 上游塞的 `SECRET_THOUGHT`）；
- **只有 2 个 `"role"`**（system + user）。

---

## 4. 活服务自测抓到的缺陷（已修 + 已回归）

**现象**：活服务发一句，`sentences_seen=0`、`rule_cues=0`——规则层与二路**都没工作**。

**根因**：core 的 epoch **只在 stop 时推进**（`live2d-ai-core/src/reducer.rs` 的
`stop()`），所以**普通轮次的 `SentenceReady.epoch` 恒为 0**（日志实测
`"epoch":0,"sentence_seq":1`）。P1 的守卫 `if epoch == 0 { return; }` 于是把
**所有普通轮次**的句子都丢了。附带缺陷：epoch 不变 → `arbiter` 从不在轮间重置，
上一轮的 cue 会残留（`upsert` 同优先级保留先到的 → 旧 cue 赢）。

**修法**（都在 director 内，未碰主链）：
1. `handle_sentence_ready` 不再拒绝 epoch 0（epoch 是不透明代号，0 合法）；
2. **换轮信号 = `TurnPrompt`**：到达时 `arbiter.reset(current_epoch)` + 重置节流。

**回归**：`epoch_zero_is_accepted_and_turn_prompt_resets_the_arbiter`。

---

## 5. 门禁（本机实测）

| 检查 | 结果 |
| --- | --- |
| `cargo test --workspace --all-targets` | **1295 passed / 0 failed** |
| `cargo test --doc --workspace` | **3 passed / 0 failed** |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0 warning** |
| `cargo run -p xtask -- rust-ratio` | **96.8925% PASS** |
| `flutter analyze` | No issues found |
| `flutter test` | **983 passed** |
| `env -u NO_COLOR trunk build` | ✅ success（`crates/l2d-wasm-demo/dist` 已重建） |
| `./scripts/ignite.sh` + `--check` | 四项全 `[ok]`（`/ → 302 /app/`、`/app/ → 200`、两份产物不引用 gstatic CanvasKit） |

> 一次全量 `cargo test` 中 `web_api::tests_ws::ws_and_http_concurrent` 偶发红
> （WS 首帧撞 heartbeat，与本次改动无关，单跑 3/3 绿）——如实记录，不粉饰。

---

## 6. 诚实标注

1. **wasm 无新逻辑**：本波改动全在 host 侧的 director crate / Flutter 文案，
   `l2d-wasm-demo` 一行未改；`trunk build` 按要求跑了并重建了 `dist/`，
   但「wasm 含新逻辑」这件事**不成立**，只能说「wasm 产物已同步重建且可编译」。
2. **TTS 端点未起**（本机 `127.0.0.1:8080` 无监听）：主链在 TTS 阶段
   `tts_transport fatal`，但这发生在 `SentenceReady` **之后**——导演锚点与二路
   `complete` 都已被触发，故不影响本波证据；「送 TTS 的文本未变」由引擎契约 +
   离线回归保证，未在活服务上验证到出声。
3. **二路同步阻塞**：`complete` 是同步调用，会占用 Mod 事件 worker 至多
   `staging_timeout_ms`（supervisor 侧 `try_send` 非阻塞，主链不等）。这是 P1-3
   同步契约的既有取舍，本波**未**改成异步（不扩主链）。
4. **epoch 语义**：普通轮次 epoch 恒 0 是 core 的现状，本波只让 director 适配它
   （不碰 core）。若将来 core 改为「每轮推进 epoch」，`TurnPrompt` 的重置仍然正确。

---

## 7. 改了哪些文件

- `crates/live2d-ai-mod-director/Cargo.toml`（+reqwest blocking / +live2d-ai-runtime）
- `crates/live2d-ai-mod-director/src/staging_http.rs`（**新增**：真实客户端 + mock HTTP 回归）
- `crates/live2d-ai-mod-director/src/staging.rs`（trait 扩展 kind/has_api_key、StagingSetup、assemble）
- `crates/live2d-ai-mod-director/src/lib.rs`（配置字段、settings_spec、state_json.staging、
  factory 装配、epoch 0 修复、TurnPrompt 换轮重置、日志）
- `crates/live2d-ai-mod-director/src/tests.rs`（P1 段拆出 → 788 行，回到 ≤800）
- `crates/live2d-ai-mod-director/src/tests_staging.rs`（**新增**：P1-3/P1-4 回归 + 6 条新回归）
- `shell/flutter/lib/settings/mods/director_panel.dart`（面板一句「未配端点则仅规则」）
- `docs/architecture/director-mod-v0.md`（§0.1 / §4 / §9.1 / §14）
- `docs/legacy/plans/HANDOFF-2026-09-19-director-staging-wire.md`（本文）
