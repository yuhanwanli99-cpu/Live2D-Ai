# 阶段4 收口报告（Worker W4e：会话 baseline + host 接线 + 收口，2026-09-26）

> **工作树**：`/home/skystar/Live2D-Ai-l1`（分支 `mod/l1-product`，实际 HEAD `f703d0e5`）。
> **波次**：4e（**串行**，前置 4b/4c/4d 已 settle）。
> **契约真源**：[performance-protocol-v1.md](../architecture/performance-protocol-v1.md)（v1）
> · [session-scope-l1.md](../architecture/session-scope-l1.md)
> · [STAGE4-plan-2026-09-26.md](STAGE4-plan-2026-09-26.md) §2(4e)/§3/§4.5/§4.6/§4.7。
> **本报告只记录原始输出与诚实未决**；自述不算数。

---

## 0. 一句话

会话 baseline 落进 `session_scope` 的**兄弟字段**（同表、同键、同闸），宿主 API 是唯一写入方；
停止 / 新消息时 host 把该会话 baseline 作为**既有 `action_cue` 帧**广播、并在**同一个 HTTP 响应**里回给
调用方，**不排队、不补帧**；director 的二路产出面能表达 v1 三字段且**仍只产 cues**；
`action_tx` / `apply_settings` 零调用红线未动。

---

## 1. 改动文件（全部 ⊆ 4e 授权表）

| 文件 | 性质 | 关键 diff 摘要 |
| --- | --- | --- |
| `crates/live2d-ai-desktop/src/session_scope.rs` | 改 | `Inner` 增兄弟字段 `baselines: BTreeMap<String,String>`；新增 `set_baseline` / `baseline_for` / `clear_baseline` / `baseline_count` / `baseline_sessions`；`clear_all` 两半一起清；`MAX_SESSION_BASELINE_CHARS = 4_000`；7 条单测 |
| `crates/live2d-ai-desktop/src/web_api/chat_routes.rs` | 改 | session 路由读写 baseline（`baseline` 键缺席≠null）；`BaselineRevocation` + `return_to_session_baseline` / `cancel_and_return_to_session_baseline`；chat/stop 响应加 `session_id`+`baseline`；5 条新回归 |
| `crates/live2d-ai-desktop/src/web_api/dispatch.rs` | 改 | `ChatPost`/`ChatStop` 把 `ctx.broadcaster` 传给 handler（5 行） |
| `crates/live2d-ai-mod-director/src/plan.rs` | 改 | `Cue` 增 v1 可选键（`field/x/y/z/at/hold/seq`）+ `Cue::legacy(...)`；`to_json` 只增不改；`parse_plan` 接受 v1 三字段形态（`parse_field_cue`） |
| `crates/live2d-ai-mod-director/src/staging.rs` | 改 | `STAGING_SYSTEM` 同时给出 legacy / v1 两条合法形态，明写「不输出任何台词文本」 |
| `crates/live2d-ai-mod-director/src/tests_staging.rs` | 改 | 3 处 `Cue` 字面量改用 `Cue::legacy(..)`；新增 `staging_surface_can_express_the_three_v1_fields_without_text` |

**未创建/未触碰**：`web_api/settings.rs`（授权表点名的这个路径在树里**不存在**——见 §6.1）；
`topics.rs` / `mod_registry.rs` / `supervisor.rs` / `desktop/src/main.rs` 一行未动；
`shell/flutter/**` 一行未动；4b/4c/4d 的独占文件一行未动。

### 1.1 关键 diff（节选，逐字）

```rust
// session_scope.rs：兄弟字段 + 同一道闸（与 prompt_for 同构）
pub fn set_baseline(&self, session: &str, baseline: &str) -> bool {
    let Some(id) = sanitize_session_id(session) else { return false; };
    if baseline.chars().count() > MAX_SESSION_BASELINE_CHARS { return false; }
    let mut inner = self.guard();
    if baseline.is_empty() { inner.baselines.remove(&id); return true; }
    if !inner.baselines.contains_key(&id) && inner.baselines.len() >= MAX_SESSION_SCOPES {
        return false;
    }
    inner.baselines.insert(id, baseline.to_string());
    true
}
pub fn baseline_for(&self, session: Option<&str>) -> Option<String> {
    let id = session.and_then(sanitize_session_id)?;   // 同一道闸
    self.guard().baselines.get(&id).cloned()
}
```

```rust
// chat_routes.rs：停止 / 新消息的第四步（不排队、不补帧）
pub fn return_to_session_baseline(
    handle: &SupervisorHandle,
    broadcaster: Option<&crate::web_api::ws::Broadcaster>,
    reason: &str,
) -> BaselineRevocation {
    let store = handle.session_scopes();
    let session = store.active();
    let baseline = store.baseline_for(session.as_deref());
    let cues = baseline_cues(baseline.as_deref());
    let frame = serde_json::to_string(&serde_json::json!({
        "type": "action_cue",                       // 既有帧型（V11：不新增帧）
        "data": { "epoch": handle.current_epoch(), "covers_upto_seq": 0,
                  "cues": cues, "baseline": true, "reason": reason }
    })).ok();
    if let (Some(bc), Some(raw)) = (broadcaster, frame.as_deref()) { bc.broadcast(raw); }
    BaselineRevocation { session, baseline, frame, backfilled: false }  // 恒不补帧
}
pub fn cancel_and_return_to_session_baseline(..) -> BaselineRevocation {
    handle.stop();                                   // 清三样 = supervisor 的 stop 事务
    return_to_session_baseline(handle, broadcaster, reason)
}
```

```rust
// mod-director/plan.rs：三字段 + legacy 键逐字保留（只增不改）
pub fn to_json(&self) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    object.insert("sentence_seq".into(), json!(self.sentence_seq));   // legacy 永远在
    object.insert("preset_id".into(), json!(self.preset_id));
    object.insert("intensity".into(), json!(self.intensity));
    object.insert("ttl_ms".into(), json!(self.ttl_ms));
    object.insert("priority".into(), json!(self.priority));
    let Some(field) = self.field else { return Value::Object(object); };
    object.insert("field".into(), json!(field.as_str()));
    if field == CueField::Expression { object.insert("id".into(), json!(self.preset_id)); }
    for (k, v) in [("x", self.x), ("y", self.y), ("z", self.z)] { if let Some(v)=v { object.insert(k.into(), json!(v)); } }
    if let Some(at) = &self.at { object.insert("at".into(), json!(at)); }
    if let Some(hold) = self.hold { object.insert("hold".into(), json!(hold)); }
    if let Some(seq) = self.seq { object.insert("seq".into(), json!(seq)); }
    Value::Object(object)
}
```

### 1.2 host 接线（V10 §9.3 / O7 / O8）

| 触发 | host 落点 | 做了什么 |
| --- | --- | --- |
| **停止键** `POST /api/v1/chat/stop` | `handle_stop_with_supervisor` | `cancel_and_return_to_session_baseline`：`handle.stop()`（supervisor 的 epoch 推进 / 取消在飞轮 / 清 pending PCM / StopPlayback）→ 广播一条 `action_cue`（该会话 baseline；待机 → `cues:[]`）→ 响应带 `session_id`+`baseline` |
| **新用户消息** `POST /api/v1/chat` | `handle_chat_with_supervisor` | 先把本条消息的会话**归一化并落活动游标**（否则回的是上一条消息的会话）→ `return_to_session_baseline(.., "new-message")` → `say_scoped`。**不注入 server 侧 Stop**（理由见 §6.3） |
| **baseline 写入** `POST /api/v1/chat/session` | `handle_chat_session` | `{"session_id":"S","baseline":<json|null>}`；键**缺席=不动**，`null`=撤销（缺省为空=待机），对象/数组=写入；键存在但无合法 session_id → 400（宿主 API 写入方错误） |
| **baseline 读取** `GET /api/v1/chat/session` | 同上 | 响应加 `active_baseline` / `baselines` / `baseline_sessions` |

---

## 2. 回归名与原始数字

### 2.1 必须落地的三条（4e 判据 E1/E2/E3）

命令：`cargo test --workspace --all-targets --no-fail-fast`（完整 log：`/tmp/w4e-gate-test.log`）

```text
test session_scope::tests::baseline_is_bound_per_session_and_not_global ... ok
test web_api::chat_routes::tests::stop_returns_to_the_session_baseline ... ok
test web_api::chat_routes::tests::new_message_cancels_orchestration_without_backfill ... ok
```

| 判据 | 回归 | 语义断言要点 |
| --- | --- | --- |
| E1 | `baseline_is_bound_per_session_and_not_global` | A/B 各持 baseline；写 B 不动 A；清 B 不动 A；`None` 桶与未写入会话都回 `None`（待机，**绝不回落别人的 baseline**）；非法 id 被同一道闸拒绝；baseline 不影响 `len()`（兄弟字段，不伪造 prompt 会话） |
| E2 | `stop_returns_to_the_session_baseline` | 停止 → HTTP 响应带**该会话** baseline + WS 上**恰好一条** `action_cue`（`data.baseline=true`/`reason="stop"`）；切会话后回另一条；待机会话回 `baseline:null` + `cues:[]`；**一帧之后没有第二帧**（不补帧） |
| E3 | `new_message_cancels_orchestration_without_backfill` | 新消息 → **唯一一条**回基准帧（`reason="new-message"`）整表替换旧计划，且之后**不再有任何帧**（旧 cue 不许重放）；待机会话同样清计划 |

### 2.2 辅助回归（同一波次新增）

```text
test session_scope::tests::baseline_write_gate_rejects_overlong_and_empty_revokes ... ok
test session_scope::tests::clear_all_also_clears_baselines ... ok
test session_scope::tests::baseline_and_prompt_do_not_overwrite_each_other ... ok
test web_api::chat_routes::tests::session_route_writes_and_reads_the_baseline_per_session ... ok
test web_api::chat_routes::tests::baseline_write_rejects_bad_session_and_overlong_payload ... ok
test tests_staging::staging_surface_can_express_the_three_v1_fields_without_text ... ok
```

### 2.3 E4：既有零调用红线（仍绿）

```text
test tests::action_tx_and_apply_settings_are_never_called ... ok          (live2d-ai-mod-director)
test tests_staging::director_never_writes_config_or_actions ... ok        (live2d-ai-mod-director)
test tests::command_paths_never_touch_action_or_settings ... ok           (live2d-ai-mod-director)
test mod_registry::tests::action_request_is_dormant_not_delivered ... ok  (live2d-ai-desktop)
```

director 的 v1 三字段扩展**没有**引入任何下行通道：`Cue` 只多带可选键，`emit_cues` 仍只走
`ModServices.cues` → host 广播 `action_cue`；`action_tx` / `apply_settings` 调用次数仍为 0。

---

## 3. E2 负对照（D20，四段缺一不可）

**声称**：去掉 baseline 的**会话绑定** → 退回全局而非会话 baseline。

```text
① 命令原文：
   cargo test -p live2d-ai-desktop baseline --no-fail-fast

② 修改点（crates/live2d-ai-desktop/src/session_scope.rs::SessionScopeStore::baseline_for）：
   把「按会话键取」改成「不看会话键、取表里任意一条」：
   -   pub fn baseline_for(&self, session: Option<&str>) -> Option<String> {
   -       let id = session.and_then(sanitize_session_id)?;
   -       self.guard().baselines.get(&id).cloned()
   -   }
   +   pub fn baseline_for(&self, _session: Option<&str>) -> Option<String> {
   +       // 负对照（临时）：退回「全局」——不看会话键，取表里任意一条。
   +       self.guard().baselines.values().next().cloned()
   +   }

③ 红输出（完整 log：/tmp/w4e-e2-red.log）：
   test session_scope::tests::baseline_is_bound_per_session_and_not_global ... FAILED
   test web_api::chat_routes::tests::session_route_writes_and_reads_the_baseline_per_session ... FAILED
   test web_api::chat_routes::tests::stop_returns_to_the_session_baseline ... FAILED
   thread 'session_scope::tests::baseline_is_bound_per_session_and_not_global' panicked at
     crates/live2d-ai-desktop/src/session_scope.rs:581:9:
     assertion `left == right` failed
       left: Some("{\"field\":\"expression\",\"id\":\"smile\",\"intensity\":1,\"at\":\"now\",\"hold\":true}")
      right: Some("{\"field\":\"body\",\"x\":-0.2,\"y\":0.1,\"intensity\":2,\"at\":\"now\",\"hold\":true}")
   thread 'web_api::chat_routes::tests::stop_returns_to_the_session_baseline' panicked at
     crates/live2d-ai-desktop/src/web_api/chat_routes.rs:887:9:
     assertion `left == right` failed
       left: String("expression")
      right: String("head")
   thread 'web_api::chat_routes::tests::session_route_writes_and_reads_the_baseline_per_session'
     panicked at crates/live2d-ai-desktop/src/web_api/chat_routes.rs:1030:9:
     assertion `left == right` failed
       left: Some("{\"at\":\"now\",\"field\":\"expression\",\"hold\":true,\"id\":\"smile\",\"intensity\":1}")
      right: None
   test result: FAILED. 4 passed; 3 failed; 0 ignored; 0 measured; 597 filtered out; finished in 0.00s

④ 还原后（同命令；完整 log：/tmp/w4e-e2-green.log）：
   test session_scope::tests::baseline_is_bound_per_session_and_not_global ... ok
   test session_scope::tests::session_route... ... ok
   test web_api::chat_routes::tests::session_route_writes_and_reads_the_baseline_per_session ... ok
   test web_api::chat_routes::tests::stop_returns_to_the_session_baseline ... ok
   test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 597 filtered out; finished in 0.00s
   （还原后 `cargo fmt --all -- --check` = FMT_OK；`grep -n '负对照（临时）' session_scope.rs` = NO_RESIDUE）
```

---

## 4. 门禁（双来源）

### 4.1 来源①：W4e 本轨原始输出（本次 tee）

| # | 命令原文 | tee 路径 | 原始读数 |
| --- | --- | --- | --- |
| G1 | `cargo test --workspace --all-targets --no-fail-fast` | `/tmp/w4e-gate-test.log` | **26 个 test target，1444 passed / 0 failed** |
| G2 | `cargo test --doc --workspace` | `/tmp/w4e-gate-doc.log` | **3 passed / 0 failed** |
| G3 | `cargo fmt --all -- --check` | `/tmp/w4e-gate-fmt.log` | exit **0**（无 diff） |
| G4 | `cargo clippy --workspace --all-targets -- -D warnings` | `/tmp/w4e-gate-clippy.log` | exit **0**（`Finished dev profile`，0 warning） |
| G5 | `cargo run -p xtask -- rust-ratio` | `/tmp/w4e-gate-ratio.log` | **97.3353%**（95996 / 98624 物理行）<br>门槛 95% → **PASS**；dart 豁免 178 文件 / 46078 行 |
| G6 | `cd shell/flutter && flutter analyze` | `/tmp/w4e-flutter-analyze.log` | `No issues found! (ran in 3.4s)`，exit **0** |
| G7 | `cd shell/flutter && flutter test` | `/tmp/w4e-flutter-test.log` | **+1063: All tests passed!**，exit **0** |

> `flutter` 不在 PATH；用绝对路径 `/home/skystar/flutter/bin/flutter`。

Cargo 全程 `CARGO_BUILD_JOBS=4`、**串行**执行（D21：swap 接近用满）。

### 4.2 来源②：编排者独立复跑（scoped）

编排者派单前置注 0 原文（**引用**）：
> W4b / W4c / W4d **三轨已 settle**；编排者已独立复跑三轨 scoped 门禁：
> fmt exit 0 / clippy(3 crates --all-targets -D warnings) exit 0 / tests exit 0。

4e 的 scoped 复跑由编排者在 C3 执行；本轨提供的 scoped 读数为：
`cargo test -p live2d-ai-desktop -p live2d-ai-mod-director --no-fail-fast` =
**desktop 604 passed / 0 failed，director 60 passed / 0 failed**
（log：`/tmp/w4e-scoped-test.log`）。

### 4.3 C5 / C6（端到端 / 驱动实测）——**未由本轨执行**

按编排者派单附注 5：`verify_core_chain` / HUD `preset:` 行 / WS 帧序列 / ack 序列 /
两种配置驱动实测由编排者在收口阶段跑；本轨**未起 live2d-ai 服务**（开工时 18080 无监听）。
因此**跳数不由本轨给出**，不伪造。收尾时观察到编排者已起 `live2d-ai-desktop --web --http-port 18080`
（PID 59888）；TTS 实例仍为**唯一一个**（PID 21058 / 127.0.0.1:8080），未起第二个（D21 合规）。

---

## 5. O7 / O8 / O15 裁决原文（协议 §12.1，原样引用）

> **O7** | 停止 / 新消息 → **立即回基准（无过渡动画）** | ⚠ **产品口味**：过渡动画进 backlog，用户验收后可回改
>
> **O8** | baseline 存 `session_scope` **兄弟字段**；**写入方 = 宿主 API**（随会话设置写）；**缺省为空 = 待机**；本轮**不做** per-character profile 文件 | 不新造第二套会话表
>
> **O15** | D11 不对称 **v1 不统一**（明文保留） | ⚠ **产品口味**，可回改；进 backlog

对应落地：

- **O8**：`Inner.baselines` 兄弟字段（同表 / 同键 / 同 `sanitize_session_id` 闸）；
  唯一写入方 = `POST /api/v1/chat/session`；未写入 = `None` = 待机；无 per-character profile 文件。
- **O7**：host 侧「立即」= **同步**返回 + 同步广播，**不排队、不补帧**（`backfilled` 恒 `false`）。
  真正被渲染面应用的过渡策略（无过渡）属前端渲染面，见 §6.2。
- **O15**：本轨**未改** D11 不对称（performance 开的中性轮 noop / 关时 `none` 撤销哨兵）——
  v1 明文保留；没有为「统一」动 director 规则层的 `none` 语义。

---

## 6. 残留 / 未决（诚实标注）

### 6.1 授权表点名 `web_api/settings.rs`，但树里不存在

`crates/live2d-ai-desktop/src/web_api/settings.rs` **不存在**（实际是目录
`web_api/settings_routes/` 与 `web_api/mod.rs` 的 `mod settings_routes;`）。
本轨**未创建**该文件（新建模块要动 `mod.rs`，而 `mod.rs` 不在授权表）。
baseline 的宿主 API 因此住在**既有的** `/api/v1/chat/session`（`chat_routes.rs`，授权文件内，
且该文件本来就承载会话路由）。**请编排者确认授权表是否应改指 `settings_routes/`**。

### 6.2 前端「回 baseline」的最终应用点仍未接线（Flutter 不在本轨授权）

4d 在 `shell/flutter/lib/main.dart` 保留 `_requestSessionBaseline(reason)`**空实现**，
注释原文：「host 侧接口未冻结……端点冻结后在此接线」。本轨已把 host 侧冻结为**两路交付**：

1. **同一个 HTTP 响应**：`POST /api/v1/chat` 与 `/api/v1/chat/stop` 的响应体都带
   `session_id` + `baseline`（`null` = 待机）——前端**不另发请求**（与 4d 注释一致）；
2. **WS**：同一次收口广播一条既有 `action_cue` 帧（`data.baseline=true` / `data.reason`），
   前端 `ActionCueEvent` 已经在消费该帧型。

**未决**：渲染面**立即**应用该 baseline（O7 无过渡）需要前端把这两路之一接到
`applyPreset`/`returnToBaseline`。本轨**未改** `shell/flutter/**`（授权表不含），按编排者附注 4 上报。

### 6.3 「新消息 → 清三样」的 server 侧 Stop 未注入（需 supervisor.rs 授权）

`handle_chat_with_supervisor` 在新消息路径上**只**回 baseline，**不**注入 `handle.stop()`。
原因是硬事实：`crates/live2d-ai-desktop/src/supervisor/turn.rs:155` 的 `do_stop!` 会
`while say_rx.try_recv().is_ok() {}` 清 pending say——若在 `say_scoped` **之前**注入 Stop，
轮内 select 可能先吃掉 Say、再吃掉 Stop，**把这条新消息自己吞掉**（消息丢失）。
正确解法是 supervisor 内的「先取消、后入队」原子路径，但 `supervisor.rs` 属**全波次禁区**，
本轨按纪律**停手上报**，未改。当前语义：新消息路径的「清三样」由上一轮自身的取消事务 +
前端 `StageCancellation`（4d 本地四步）承担；host 保证**不补帧**与**回该会话 baseline**。

### 6.4 baseline 的内容形状不由 host 解释

host 只保证「存、取、按会话回」；`baseline` 是不透明 JSON（对象 = 单条 cue；数组 = cue 列表；
其它 / 坏 JSON → 广播退化为 `cues:[]` = 待机）。「field 是否 body/head/expression」由写入方
（宿主 API 的调用方）与渲染面负责——与 session_scope 对 prompts 的「宿主只存字符串」同一条纪律。

### 6.5 阶段3 遗留（不在本波范围）

真表演层端点验收（D19）与出厂 `[performance]` degraded 的 UI 提示仍在 backlog；
`verify_core_chain` / HUD / ack / 两配置驱动实测由编排者执行（§4.3）。

---

## 7. 复现命令清单（本报告所有数字）

```bash
cd /home/skystar/Live2D-Ai-l1
CARGO_BUILD_JOBS=4 cargo test --workspace --all-targets --no-fail-fast | tee /tmp/w4e-gate-test.log
CARGO_BUILD_JOBS=4 cargo test --doc --workspace | tee /tmp/w4e-gate-doc.log
cargo fmt --all -- --check | tee /tmp/w4e-gate-fmt.log
CARGO_BUILD_JOBS=4 cargo clippy --workspace --all-targets -- -D warnings | tee /tmp/w4e-gate-clippy.log
CARGO_BUILD_JOBS=4 cargo run -p xtask -- rust-ratio | tee /tmp/w4e-gate-ratio.log
cd shell/flutter && /home/skystar/flutter/bin/flutter analyze | tee /tmp/w4e-flutter-analyze.log
cd shell/flutter && /home/skystar/flutter/bin/flutter test | tee /tmp/w4e-flutter-test.log
# E2 负对照
CARGO_BUILD_JOBS=4 cargo test -p live2d-ai-desktop baseline --no-fail-fast | tee /tmp/w4e-e2-{red,green}.log
```
