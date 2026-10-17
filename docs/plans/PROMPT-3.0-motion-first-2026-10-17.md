# 执行单：3.0 第一片 · 手势到期放回（2026-10-17）

> 稳定版 `v0.2.7` 已打。本片是顺序第 1 项的一部分，不是整项。
> 不升版本，不打 `0.3.x`，不推送，除非维护者点名。不开始第 6、2、4、3、7 项。
> 不改 `.env`、`mods.json`、`live2d-ai.toml`。不提高 `DOCS_BUDGET_LINES`。
> `action_tx` 继续休眠。不新增预设。不改导演选包。

## 做这一件事

手势槽时间到现在是立即清槽。改成与撤销同一条放回：从屏幕上的值收到 0，时长沿用 `ttl_ms × (1 - HOLD_END)`。表情槽、呼吸、眨眼、`BODY_LAG`、`ATTACK_END`、`HOLD_END` 不动。同一 id 重开、同槽换动作、`preset_id == "none"` 的现有放回保持原样。

## 验收

改回「到期立即清」时，新测试变红。修回后绿。`cargo fmt --all -- --check`，`cargo test -p l2d-wasm-demo`，`cargo clippy -p l2d-wasm-demo --all-targets -- -D warnings`。渲染面改了就要重建 wasm dist，并写明浏览器没看。不点火。

做完本地 commit。不要自己打 `0.3.x`。
