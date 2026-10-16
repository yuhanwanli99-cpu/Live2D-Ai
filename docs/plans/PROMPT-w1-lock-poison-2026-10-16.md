# 执行单：W1 共享锁毒化（2026-10-16）

> 上一片 D1–D8 已本地提交。Cargo 版本保持 `0.2.6-rc.1`。不发布 `0.3.0`，不升版本。
> 不移动 tag `v0.2.6-rc.1`。不改 `.env`、`mods.json`、`live2d-ai.toml`。
> 不提高 `DOCS_BUDGET_LINES`。不点火，不刷新页面。不复活 `action_tx` 与已删 Mod。
> 计划书里「不打新 tag」从本单起改口，以本单的 git 规则为准。

## 0. 做完之后的 git

一个本地 commit，再打附注 tag `debt/w1-lock-poison`。不推送。
tag 说明写「债务片，不是发布」。不要打 `v0.2.6`、`v0.2.7`、`0.3.0`。
以后每轮执行单都按这条：做完就本地 commit + 附注 tag，不推远端，除非维护者当轮点名推送。

## 1. 上一轮未做完的，本单带上

1. `shell/flutter/lib/api/settings_models_result.dart` 的 `queued` 注释仍写「实际不会产生」。服务端在一轮还在飞时会回 `queued`（`SupervisorHandle::reload_reported` 超时）。改成这句事实。枚举值不动。
2. `crates/live2d-ai-desktop/src/web_api/tests_p0c.rs` 的 `apply_status_serializes_to_snake_case` 补上 `ApplyStatus::ReloadFailed` → `"reload_failed"`。
3. D1 的 Nightly 仍没有 run URL。本单不推送，不要跑 `gh workflow run`。回报里写「D1 仍无 run URL」。

棘轮注释已按实测行数改过。`RATCHET_SRC_RS_500` 保持 42。本单不要再改那段，除非下面的改动把某个文件顶过 500：先拆到每份 ≤500，再把常量收紧到新的个数。

## 2. W1

清点真源 `docs/audit/2026-10-16-unwrap-inventory.md` §3。只改这 7 处生产锁：

- `crates/live2d-ai-desktop/src/web_api/ws/broadcaster.rs` 的 92、108、133（`expect("broadcaster mutex")`）
- `crates/live2d-ai-desktop/src/mod_registry/registry.rs` 启动写入（约 147）、`disable` 取走（约 273）、放回（约 276）
- `crates/live2d-ai-desktop/src/mod_registry/events.rs` 的 worker `shutdown`（约 79）

换成 `lock().unwrap_or_else(|e| e.into_inner())`。先例在 `web_api/mods_routes.rs` 的 `lock_registry`，以及 `events.rs` 里已有的 `sink.lock().unwrap_or_else(...)`。

`broadcaster.rs` 里 `#[cfg(test)]` 的 `clear`（约 155）不动。A–D 类 `unwrap` / `expect` 不动。不新增 clippy lint。不改 `FINDINGS.md` 正文。

回归两条，缺一条就不算做完：

- 广播器那把锁毒化之后，`try_subscribe` 与 `broadcast` 不 panic。
- Mod 槽位锁毒化之后，`disable` 不 panic，句柄还在（停不掉的那条路径仍能再试）。

## 3. 不做

不修 `shared/persona.yaml` 引起的 `pytest tests/`。不同步 `docs/design/web-ui-spec-v3.md`。
不给「没有 supervisor 时手改 toml」新做错误帧。不搬调试面板。不动 wasm 体积。

## 4. 门禁

先清 `http_proxy` `https_proxy` `HTTP_PROXY` `HTTPS_PROXY` `ALL_PROXY`。不要 `cargo clean`。

`cargo fmt --all -- --check`，`cargo test -p live2d-ai-desktop`，`cargo clippy -p live2d-ai-desktop --all-targets -- -D warnings`，`cargo run -q -p xtask -- code-stats --check`。
Dart 只改了注释时跑 `cd shell/flutter && flutter analyze`。
回报写每条命令的退出码。不点火。
